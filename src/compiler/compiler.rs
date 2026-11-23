use std::collections::HashMap;

use crate::compiler::expression::ExpressionError;
use crate::compiler::expression::function_call::FunctionCallError;
use crate::compiler::{
    CompileConfig, Function, Program, TypeDef,
    expression::{
        Abort, Array, Assignment, Block, Break, Container, Continue, Expr, Expression, For,
        FunctionArgument, FunctionCall, Group, IfStatement, Literal, Noop, Not, Object, Op,
        Predicate, Query, Return, Target, Unary, Variable, assignment, break_, function_call,
        literal, predicate, query,
    },
    parser::ast::RootExpr,
    program::ProgramInfo,
};
use crate::diagnostic::{DiagnosticList, DiagnosticMessage};
use crate::parser::ast::{self, Node, QueryTarget};
use crate::path::PathPrefix;
use crate::path::{OwnedTargetPath, OwnedValuePath};
use crate::prelude::{ArgumentList, expression};
use crate::value::Value;

use super::state::TypeState;

pub(crate) type DiagnosticsMessages = Vec<Box<dyn DiagnosticMessage>>;

pub struct CompilationResult {
    pub program: Program,
    pub warnings: DiagnosticList,
    pub config: CompileConfig,
}

/// The compiler has many `compile_*` functions. These all accept a `state` param which
/// should contain the type state of the program immediately before the expression
/// that is being compiled would execute. The state should be modified to reflect the
/// state after the compiled expression executes. This logic lives in `Expression::type_info`.
pub struct Compiler<'a> {
    fns: &'a [Box<dyn Function>],
    diagnostics: DiagnosticsMessages,
    fallible: bool,
    abortable: bool,
    external_queries: Vec<OwnedTargetPath>,
    external_assignments: Vec<OwnedTargetPath>,

    /// A list of variables that are missing, because the rhs expression of the
    /// assignment failed to compile.
    ///
    /// This list allows us to avoid printing "undefined variable" compilation
    /// errors when the reason for it being undefined is another compiler error.
    skip_missing_query_target: Vec<(QueryTarget, OwnedValuePath)>,

    /// Stack of unhandled fallibility errors collected while compiling the
    /// current root expression.
    ///
    /// Each entry is the error produced by a fallible expression whose
    /// fallibility has not (yet) been consumed by a parent (assignment,
    /// error-coalesce `??`, etc.). Helpers track their own boundary by
    /// snapshotting `len()` on entry and either leaving sub-expression
    /// entries pending, or truncating back to the snapshot to consume them.
    /// At the root expression boundary any entries still pending are
    /// flushed as diagnostics. Using a stack instead of a single slot lets
    /// multiple unhandled errors surface in one compilation pass — e.g.
    /// when both a discarded fallible call and a later fallible assignment
    /// exist in the same block.
    pending_fallibilities: Vec<CompilerError>,

    loop_depth: usize,
    speculative_loop: SpeculativeLoopState,
    loop_eval_ctx: expression::for_loop::LoopEvalContext,
    for_loop_cache: HashMap<crate::diagnostic::Span, Vec<CachedCompiledFor>>,

    config: CompileConfig,
    in_breakable_context: bool,
}

#[derive(Clone, Copy, Default)]
struct SpeculativeLoopState {
    depth: usize,
    had_deferred_error: bool,
}

#[derive(PartialEq)]
struct ForCacheKey {
    input: expression::for_loop::ProjectedLoopState,
    in_loop: bool,
    in_breakable: bool,
}

struct CachedCompiledFor {
    key: ForCacheKey,
    for_expr: For,
    output_patch: expression::for_loop::LoopStatePatch,
    external_queries: Vec<OwnedTargetPath>,
    external_assignments: Vec<OwnedTargetPath>,
    fallible: bool,
    abortable: bool,
}

struct CompilerSnapshot {
    diagnostics_len: usize,
    pending_fallibilities_len: usize,
    external_queries_len: usize,
    external_assignments_len: usize,
    skip_missing_query_target_len: usize,
    fallible: bool,
    abortable: bool,
    speculative_deferred_error: bool,
    config: CompileConfig,
}

// TODO: The diagnostic related code is in dire need of refactoring.
// This is a workaround to avoid doing this work upfront.
#[derive(Debug)]
pub(crate) enum CompilerError {
    FunctionCallError(FunctionCallError),
    ExpressionError(ExpressionError),
}

impl CompilerError {
    fn to_diagnostic(&self) -> &dyn DiagnosticMessage {
        match self {
            CompilerError::FunctionCallError(e) => e,
            CompilerError::ExpressionError(e) => e,
        }
    }

    fn into_diagnostic_boxed(self) -> Box<dyn DiagnosticMessage> {
        match self {
            CompilerError::FunctionCallError(e) => Box::new(e),
            CompilerError::ExpressionError(e) => Box::new(e),
        }
    }
}

impl<'a> Compiler<'a> {
    fn snapshot(&self) -> CompilerSnapshot {
        CompilerSnapshot {
            diagnostics_len: self.diagnostics.len(),
            pending_fallibilities_len: self.pending_fallibilities.len(),
            external_queries_len: self.external_queries.len(),
            external_assignments_len: self.external_assignments.len(),
            skip_missing_query_target_len: self.skip_missing_query_target.len(),
            fallible: self.fallible,
            abortable: self.abortable,
            speculative_deferred_error: self.speculative_loop.had_deferred_error,
            config: self.config.clone(),
        }
    }

    fn restore_snapshot(&mut self, snapshot: &CompilerSnapshot) {
        self.diagnostics.truncate(snapshot.diagnostics_len);
        self.pending_fallibilities
            .truncate(snapshot.pending_fallibilities_len);
        self.external_queries
            .truncate(snapshot.external_queries_len);
        self.external_assignments
            .truncate(snapshot.external_assignments_len);
        self.skip_missing_query_target
            .truncate(snapshot.skip_missing_query_target_len);
        self.fallible = snapshot.fallible;
        self.abortable = snapshot.abortable;
        self.speculative_loop.had_deferred_error = snapshot.speculative_deferred_error;
        self.config = snapshot.config.clone();
    }

    /// Run `f` and drop any pending fallibilities it pushed onto the stack
    /// on exit, regardless of whether `f` returned `Ok`/`Some` or pushed a
    /// hard diagnostic. Use this in helpers where the helper's *own* error
    /// (or expression type) already accounts for the inner fallibility —
    /// `abort`, `return`, predicate construction, function-argument
    /// validation. Entries pushed before the call are untouched.
    fn consuming_fallibility<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let snapshot = self.pending_fallibilities.len();
        let result = f(self);
        self.pending_fallibilities.truncate(snapshot);
        result
    }

    /// Compiles a given source into the final [`Program`].
    ///
    /// # Arguments
    ///
    /// * `source` - A string slice that holds the source code to be compiled.
    /// * `fns` - A slice of boxed functions to be used during compilation.
    ///
    /// # Returns
    ///
    /// A `Result` containing the `CompilationResult` if successful, or a `DiagnosticList` if there are errors.
    ///
    /// # Errors
    /// Any compilation error.
    pub fn compile(
        fns: &'a [Box<dyn Function>],
        ast: crate::parser::Program,
        state: &TypeState,
        config: CompileConfig,
    ) -> Result<CompilationResult, DiagnosticList> {
        let initial_state = state.clone();
        let mut state = state.clone();

        let mut compiler = Self {
            fns,
            diagnostics: vec![],
            fallible: false,
            abortable: false,
            external_queries: vec![],
            external_assignments: vec![],
            skip_missing_query_target: vec![],
            pending_fallibilities: vec![],
            loop_depth: 0,
            speculative_loop: SpeculativeLoopState::default(),
            loop_eval_ctx: expression::for_loop::LoopEvalContext::new(),
            for_loop_cache: HashMap::new(),
            config,
            in_breakable_context: false,
        };
        let expressions = compiler.compile_root_exprs(ast, &mut state);

        let (errors, warnings): (Vec<_>, Vec<_>) =
            compiler.diagnostics.into_iter().partition(|diagnostic| {
                matches!(
                    diagnostic.severity(),
                    crate::diagnostic::Severity::Bug | crate::diagnostic::Severity::Error
                )
            });

        if !errors.is_empty() {
            return Err(errors.into());
        }

        let result = CompilationResult {
            program: Program {
                expressions: Block::new_inline(expressions),
                info: ProgramInfo {
                    fallible: compiler.fallible,
                    abortable: compiler.abortable,
                    target_queries: compiler.external_queries,
                    target_assignments: compiler.external_assignments,
                },
                initial_state,
            },
            warnings: warnings.into(),
            config: compiler.config,
        };
        Ok(result)
    }

    fn compile_exprs(
        &mut self,
        nodes: impl IntoIterator<Item = Node<ast::Expr>>,
        state: &mut TypeState,
    ) -> Option<Vec<Expr>> {
        let mut exprs = vec![];
        for node in nodes {
            let expr = self.compile_expr(node, state)?;
            exprs.push(expr);
        }
        Some(exprs)
    }

    fn compile_block_exprs(
        &mut self,
        nodes: impl IntoIterator<Item = Node<ast::Expr>>,
        state: &mut TypeState,
    ) -> Option<Vec<Expr>> {
        if self.speculative_loop.depth == 0 {
            return self.compile_exprs(nodes, state);
        }

        let mut exprs = vec![];
        for node in nodes {
            let stmt_state = state.clone();
            let pre_pending = self.pending_fallibilities.len();
            let pre_skip = self.skip_missing_query_target.len();
            if let Some(expr) = self.compile_expr(node, state) {
                exprs.push(expr);
            } else {
                // Recover during speculative loop passes so subsequent widening statements
                // (e.g. `prev = v`) still run and converge the loop header.
                *state = stmt_state;
                self.pending_fallibilities.truncate(pre_pending);
                self.skip_missing_query_target.truncate(pre_skip);
                self.speculative_loop.had_deferred_error = true;
                exprs.push(Expr::Noop(Noop));
            }
        }
        Some(exprs)
    }

    fn compile_expr(&mut self, node: Node<ast::Expr>, state: &mut TypeState) -> Option<Expr> {
        use ast::Expr::{
            Abort, Assignment, Break, Container, Continue, For, FunctionCall, IfStatement, Literal,
            Op, Query, Return, Unary, Variable,
        };
        let original_state = state.clone();
        let pre_compile_pending = self.pending_fallibilities.len();

        let span = node.span();
        let expr = match node.into_inner() {
            Literal(node) => self.compile_literal(node, state),
            Container(node) => self.compile_container(node, state).map(Into::into),
            IfStatement(node) => self.compile_if_statement(node, state).map(Into::into),
            Op(node) => self.compile_op(node, state).map(Into::into),
            Assignment(node) => self.compile_assignment(node, state).map(Into::into),
            Query(node) => self.compile_query(node, state).map(Into::into),
            FunctionCall(node) => self
                .compile_function_call(node, state)
                .map(|function_call| {
                    let v = function_call
                        .warnings
                        .iter()
                        .cloned()
                        .map(|w| Box::new(w) as Box<dyn DiagnosticMessage>)
                        .collect::<Vec<_>>();

                    self.diagnostics.extend(v);
                    function_call.into()
                }),
            Variable(node) => self.compile_variable(node, state).map(Into::into),
            Unary(node) => self.compile_unary(node, state).map(Into::into),
            Abort(node) => self.compile_abort(node, state).map(Into::into),
            Return(node) => self.compile_return(node, state).map(Into::into),
            For(node) => self.compile_for_statement(node, state).map(Into::into),
            Break(node) => self.compile_break(node, state).map(Into::into),
            Continue(node) => self.compile_continue(node, state).map(Into::into),
        }?;

        // If the compiled expression is fallible and no sub-expression has
        // already pushed an entry for it, record this expression as the
        // outer-most fallible point in the current chain. Avoiding double
        // counting (e.g. `a + b()` where b is fallible already pushed) means
        // checking that the pending stack didn't grow during compilation.
        let type_def = self.expr_type_info(&expr, &original_state).result;
        if type_def.is_fallible() && self.pending_fallibilities.len() == pre_compile_pending {
            self.pending_fallibilities
                .push(CompilerError::ExpressionError(
                    expression::ExpressionError::Fallible { span },
                ));
        }

        Some(expr)
    }

    fn compile_literal(&mut self, node: Node<ast::Literal>, state: &mut TypeState) -> Option<Expr> {
        use ast::Literal::{Boolean, Float, Integer, Null, RawString, Regex, String, Timestamp};

        let (span, lit) = node.take();

        let literal = match lit {
            String(template) => {
                if let Some(v) = template.as_literal_string() {
                    Ok(Literal::from(v.to_string()))
                } else {
                    // Rewrite the template into an expression and compile that block.
                    return self.compile_expr(
                        Node::new(span, template.rewrite_to_concatenated_strings()),
                        state,
                    );
                }
            }
            RawString(v) => Ok(Literal::from(v)),
            Integer(v) => Ok(Literal::Integer(v)),
            Float(v) => Ok(Literal::Float(v)),
            Boolean(v) => Ok(Literal::Boolean(v)),
            Regex(v) => regex::Regex::new(&v)
                .map_err(|err| literal::Error::from((span, err)))
                .map(|r| Literal::Regex(r.into())),
            // TODO: support more formats (similar to Vector's `Convert` logic)
            Timestamp(v) => v
                .parse()
                .map(Literal::Timestamp)
                .map_err(|err| literal::Error::from((span, err))),
            Null => Ok(Literal::Null),
        };

        literal
            .map(Into::into)
            .map_err(|err| self.diagnostics.push(Box::new(err)))
            .ok()
    }

    fn compile_container(
        &mut self,
        node: Node<ast::Container>,
        state: &mut TypeState,
    ) -> Option<Container> {
        use ast::Container::{Array, Block, Group, Object};

        let variant = match node.into_inner() {
            Group(node) => self.compile_group(*node, state)?.into(),
            Block(node) => self.compile_block(node, state)?.into(),
            Array(node) => self.compile_array(node, state)?.into(),
            Object(node) => self.compile_object(node, state)?.into(),
        };

        Some(Container::new(variant))
    }

    fn compile_group(&mut self, node: Node<ast::Group>, state: &mut TypeState) -> Option<Group> {
        let expr = self.compile_expr(node.into_inner().into_inner(), state)?;

        Some(Group::new(expr))
    }

    fn compile_root_exprs(
        &mut self,
        nodes: impl IntoIterator<Item = Node<ast::RootExpr>>,
        state: &mut TypeState,
    ) -> Vec<Expr> {
        let mut node_exprs = vec![];

        for root_expr in nodes {
            match root_expr.into_inner() {
                RootExpr::Expr(node_expr) => {
                    self.pending_fallibilities.clear();

                    let compiled = self.compile_expr(node_expr, state);

                    // Flush whatever fallibilities are still pending,
                    // regardless of whether the root expression itself
                    // compiled. Otherwise a hard error mid-compile would
                    // discard prior unhandled fallibilities and the user
                    // would only discover them on the next compile.
                    for error in self.pending_fallibilities.drain(..) {
                        self.diagnostics.push(error.into_diagnostic_boxed());
                    }

                    if let Some(expr) = compiled {
                        node_exprs.push(expr);
                    }
                }
                RootExpr::Error(err) => self.handle_parser_error(err),
            }
        }

        if node_exprs.is_empty() {
            node_exprs.push(Expr::Noop(Noop));
        }
        node_exprs
    }

    fn compile_block(&mut self, node: Node<ast::Block>, state: &mut TypeState) -> Option<Block> {
        self.compile_block_with_type(node, state)
            .map(|(block, _type_def)| block)
    }

    fn expr_type_info(
        &mut self,
        expr: &Expr,
        state: &TypeState,
    ) -> crate::compiler::state::TypeInfo {
        if (self.loop_depth > 0 && expression::for_loop::expr_contains_loop_jump(expr))
            || expression::for_loop::expr_contains_for(expr)
        {
            let flow =
                expression::for_loop::eval_loop_expr_with_ctx(expr, state, &mut self.loop_eval_ctx);
            let next_state = flow.normal.unwrap_or_else(|| expr.type_info(state).state);
            crate::compiler::state::TypeInfo::new(next_state, flow.result)
        } else {
            expr.type_info(state)
        }
    }

    fn compile_block_with_type(
        &mut self,
        node: Node<ast::Block>,
        state: &mut TypeState,
    ) -> Option<(Block, TypeDef)> {
        let original_state = state.clone();
        let exprs = self.compile_block_exprs(node.into_inner(), state)?;
        let block = Block::new_scoped(exprs);

        // The type information from `compile_exprs` doesn't applying the "scoping" from the block.
        // This is recalculated using the block.
        let block_expr = Expr::Container(Container::new(expression::container::Variant::Block(
            block.clone(),
        )));
        let info = self.expr_type_info(&block_expr, &original_state);
        *state = info.state;
        Some((block, info.result))
    }

    fn compile_array(&mut self, node: Node<ast::Array>, state: &mut TypeState) -> Option<Array> {
        let exprs = self.compile_exprs(node.into_inner(), state)?;

        Some(Array::new(exprs))
    }

    fn compile_object(&mut self, node: Node<ast::Object>, state: &mut TypeState) -> Option<Object> {
        let (keys, exprs): (Vec<String>, Vec<Option<Expr>>) = node
            .into_inner()
            .into_iter()
            .map(|(k, expr)| (k.into_inner(), self.compile_expr(expr, state)))
            .unzip();

        let exprs = exprs.into_iter().collect::<Option<Vec<_>>>()?;

        Some(Object::new(
            keys.into_iter()
                .zip(exprs)
                .map(|(key, value)| (key.into(), value))
                .collect(),
        ))
    }

    fn compile_if_statement(
        &mut self,
        node: Node<ast::IfStatement>,
        state: &mut TypeState,
    ) -> Option<IfStatement> {
        let ast::IfStatement {
            predicate,
            if_node,
            else_node,
        } = node.into_inner();

        let original_state = state.clone();

        let predicate = self
            .compile_predicate(predicate, state)?
            .map_err(|err| self.diagnostics.push(Box::new(err)))
            .ok()?;

        let after_predicate_state = state.clone();

        let if_block = self.compile_block(if_node, state)?;

        let else_block = if let Some(else_node) = else_node {
            *state = after_predicate_state;
            Some(self.compile_block(else_node, state)?)
        } else {
            None
        };

        let if_statement = IfStatement {
            predicate,
            if_block,
            else_block,
        };

        // The current state is from one of the branches. Restore it and calculate
        // the type state from the full "if statement" expression.
        let info = self.expr_type_info(&Expr::IfStatement(if_statement.clone()), &original_state);
        *state = info.state;
        Some(if_statement)
    }

    fn compile_predicate(
        &mut self,
        node: Node<ast::Predicate>,
        state: &mut TypeState,
    ) -> Option<predicate::Result> {
        use ast::Predicate::{Many, One};

        let original_state = state.clone();
        let (span, predicate) = node.take();
        let pre_pending = self.pending_fallibilities.len();

        self.consuming_fallibility(|c| {
            let exprs = match predicate {
                One(node) => vec![c.compile_expr(*node, state)?],
                Many(nodes) => c.compile_exprs(nodes, state)?,
            };

            let override_type_def = (c.loop_depth > 0
                && exprs
                    .iter()
                    .any(expression::for_loop::expr_contains_loop_jump))
            .then(|| {
                let pred_block = Expr::Container(Container::new(
                    expression::container::Variant::Block(Block::new_inline(exprs.clone())),
                ));
                c.expr_type_info(&pred_block, &original_state).result
            });

            // The predicate's own fallibility is anything pushed during its
            // own compilation — never a stale prior entry from before it.
            let predicate_fallibility = c
                .pending_fallibilities
                .get(pre_pending)
                .map(CompilerError::to_diagnostic);
            Some(Predicate::new(
                Node::new(span, exprs),
                state,
                predicate_fallibility,
                override_type_def,
            ))
        })
    }

    fn compile_op(&mut self, node: Node<ast::Op>, state: &mut TypeState) -> Option<Op> {
        use crate::parser::ast::Opcode;

        let original_state = state.clone();

        let op = node.into_inner();
        let ast::Op(lhs, opcode, rhs) = op;

        // Snapshot the pending stack on entry so we can scope any consumes
        // performed by this op (`??`, infallible-typed result) to entries
        // produced by lhs/rhs and not touch entries from prior expressions.
        let pre_op_pending = self.pending_fallibilities.len();

        let lhs_span = lhs.span();
        let lhs = Node::new(lhs_span, self.compile_expr(*lhs, state)?);

        // `??` consumes any fallibility produced by the lhs.
        if opcode.inner() == &Opcode::Err {
            self.pending_fallibilities.truncate(pre_op_pending);
        }

        let rhs_span = rhs.span();
        let state_after_lhs = state.clone();
        let rhs = Node::new(rhs_span, self.compile_expr(*rhs, state)?);

        let override_operand_types = (self.loop_depth > 0
            && (expression::for_loop::expr_contains_loop_jump(lhs.inner())
                || expression::for_loop::expr_contains_loop_jump(rhs.inner())))
        .then(|| {
            (
                self.expr_type_info(lhs.inner(), &original_state).result,
                self.expr_type_info(rhs.inner(), &state_after_lhs).result,
            )
        });

        let op = match Op::new_speculative(
            lhs,
            opcode,
            rhs,
            state,
            self.speculative_loop.depth > 0,
            override_operand_types,
        ) {
            Ok((op, deferred_err)) => {
                if let Some(err) = deferred_err {
                    self.diagnostics.push(Box::new(err));
                    self.speculative_loop.had_deferred_error = true;
                }
                op
            }
            Err(err) => {
                // The op itself failed (e.g. `1 ?? x` is rejected as
                // unnecessary error coalescing). Sub-expression fallibilities
                // are subsumed by this hard error — don't double-report.
                self.pending_fallibilities.truncate(pre_op_pending);
                self.diagnostics.push(Box::new(err));
                return None;
            }
        };

        let type_info = self.expr_type_info(&Expr::Op(op.clone()), &original_state);

        // If the op as a whole is infallible (e.g. `?? default` or a
        // short-circuit boolean made it so), drop fallibility produced by
        // any of its sub-expressions.
        if type_info.result.is_infallible() {
            self.pending_fallibilities.truncate(pre_op_pending);
        }

        // Both "lhs" and "rhs" are compiled above, but "rhs" isn't always executed.
        // The expression can provide a more accurate type state.
        *state = type_info.state;
        Some(op)
    }

    /// Rewrites the ast for `a |= b` to be `a = a | b`.
    fn rewrite_to_merge(
        &mut self,
        span: crate::diagnostic::Span,
        target: &Node<ast::AssignmentTarget>,
        expr: Box<Node<ast::Expr>>,
        state: &mut TypeState,
    ) -> Option<Box<Node<Expr>>> {
        Some(Box::new(Node::new(
            span,
            Expr::Op(self.compile_op(
                Node::new(
                    span,
                    ast::Op(
                        Box::new(Node::new(target.span(), target.inner().to_expr(span))),
                        Node::new(span, ast::Opcode::Merge),
                        expr,
                    ),
                ),
                state,
            )?),
        )))
    }

    fn jump_aware_infallible_rhs_type(
        &mut self,
        variant: &assignment::Variant<Node<ast::AssignmentTarget>, Node<Expr>>,
        original_state: &TypeState,
    ) -> Option<TypeDef> {
        match variant {
            assignment::Variant::Infallible { expr, .. } => {
                self.jump_aware_override_type(expr.inner(), original_state)
            }
            assignment::Variant::Single { .. } => None,
        }
    }

    fn compile_assignment(
        &mut self,
        node: Node<ast::Assignment>,
        state: &mut TypeState,
    ) -> Option<Assignment> {
        use assignment::Variant;
        use ast::{
            Assignment::{Infallible, Single},
            AssignmentOp,
        };

        let original_state = state.clone();

        // Snapshot the pending stack on entry so any fallibility produced by
        // this assignment's RHS is scoped to entries beyond this point.
        // Entries pending from before this assignment stay on the stack and
        // are flushed at the next root boundary, even if `Assignment::new`
        // itself fails — so e.g. a discarded fallible call in a block
        // followed by a fallible-RHS assignment surfaces both errors in one
        // pass instead of one error blocking the discovery of the other.
        let pre_assignment_pending = self.pending_fallibilities.len();

        let assignment = node.into_inner();

        let node = match assignment {
            Single { target, op, expr } => {
                let span = expr.span();

                match op {
                    AssignmentOp::Assign => {
                        let expr = self
                            .compile_expr(*expr, state)
                            .map(|expr| Box::new(Node::new(span, expr)))
                            .or_else(|| {
                                self.skip_missing_assignment_target(&target.clone().into_inner());
                                None
                            })?;

                        Node::new(span, Variant::Single { target, expr })
                    }
                    AssignmentOp::Merge => {
                        let expr = self.rewrite_to_merge(span, &target, expr, state)?;
                        Node::new(span, Variant::Single { target, expr })
                    }
                }
            }
            Infallible { ok, err, op, expr } => {
                let span = expr.span();

                let node = match op {
                    AssignmentOp::Assign => {
                        let expr = self
                            .compile_expr(*expr, state)
                            .map(|expr| Box::new(Node::new(span, expr)))
                            .or_else(|| {
                                self.skip_missing_assignment_target(&ok.clone().into_inner());
                                self.skip_missing_assignment_target(&err.clone().into_inner());
                                None
                            })?;

                        let node = Variant::Infallible {
                            ok,
                            err,
                            expr,
                            default: Value::Null,
                        };
                        Node::new(span, node)
                    }
                    AssignmentOp::Merge => {
                        let expr = self.rewrite_to_merge(span, &ok, expr, state)?;
                        let node = Variant::Infallible {
                            ok,
                            err,
                            expr,
                            default: Value::Null,
                        };

                        Node::new(span, node)
                    }
                };

                // The infallible-form (`x, err = ...`) consumes any fallibility
                // produced by the RHS. Drop only RHS-produced entries; prior
                // pending errors stay on the stack.
                self.pending_fallibilities.truncate(pre_assignment_pending);

                node
            }
        };

        let override_rhs_type = self.jump_aware_infallible_rhs_type(node.inner(), &original_state);

        // The fallibility relevant to `Assignment::new`'s check is whatever
        // was produced by *this* assignment's RHS — i.e. the first entry past
        // the snapshot. Anything before that belongs to an earlier expression.
        let rhs_fallibility = self.pending_fallibilities.get(pre_assignment_pending);
        let assignment_result = Assignment::new_speculative(
            node,
            state,
            rhs_fallibility,
            &self.config,
            self.speculative_loop.depth > 0,
            override_rhs_type,
        );

        let assignment = match assignment_result {
            Ok((a, deferred_err)) => {
                if let Some(err) = deferred_err {
                    self.diagnostics.push(Box::new(err));
                    self.speculative_loop.had_deferred_error = true;
                }
                a
            }
            Err(err) => {
                // Drop only RHS-produced entries: the hard error subsumes
                // them. Prior pending entries stay on the stack so they're
                // still subject to outer consumer scopes (e.g. an enclosing
                // `abort`/`return`/predicate that would suppress them) or
                // get flushed at the root boundary if no consumer claims
                // them. This means at root the assignment error appears
                // before the priors in the diagnostic list — source order
                // is sacrificed for correct cross-scope semantics.
                self.pending_fallibilities.truncate(pre_assignment_pending);
                self.diagnostics.push(Box::new(err));
                return None;
            }
        };

        // Successful assignment consumes its own RHS fallibility (it's
        // expressed by the assignment expression itself now). Prior entries
        // remain pending.
        self.pending_fallibilities.truncate(pre_assignment_pending);

        // Track any potential external target assignments within the program.
        //
        // This data is exposed to the caller of the compiler, to allow any
        // potential external optimizations.
        for target in assignment.targets() {
            if let assignment::Target::External(path) = target {
                self.external_assignments.push(path);
            }
        }

        // The state hasn't been updated from the actual assignment yet. Recalculate the type
        // from the new assignment expression.
        let info = self.expr_type_info(&Expr::Assignment(assignment.clone()), &original_state);
        *state = info.state;

        Some(assignment)
    }

    fn compile_query(&mut self, node: Node<ast::Query>, state: &mut TypeState) -> Option<Query> {
        let ast::Query { target, path } = node.into_inner();

        if self
            .skip_missing_query_target
            .contains(&(target.clone().into_inner(), path.clone().into_inner()))
        {
            return None;
        }

        let path = path.into_inner();
        let target = self.compile_query_target(target, state)?;

        // Track any potential external target queries within the program.
        //
        // This data is exposed to the caller of the compiler, to allow any
        // potential external optimizations.
        if let Target::External(prefix) = target {
            let target_path = OwnedTargetPath {
                prefix,
                path: path.clone(),
            };
            self.external_queries.push(target_path);
        }

        Some(Query::new(target, path))
    }

    fn compile_query_target(
        &mut self,
        node: Node<ast::QueryTarget>,
        state: &mut TypeState,
    ) -> Option<query::Target> {
        use ast::QueryTarget::{Container, External, FunctionCall, Internal};

        let span = node.span();

        let target = match node.into_inner() {
            External(prefix) => Target::External(prefix),
            Internal(ident) => {
                let variable = self.compile_variable(Node::new(span, ident), state)?;
                Target::Internal(variable)
            }
            Container(container) => {
                let container = self.compile_container(Node::new(span, container), state)?;
                Target::Container(container)
            }
            FunctionCall(call) => {
                let call = self.compile_function_call(Node::new(span, call), state)?;
                Target::FunctionCall(call)
            }
        };

        Some(target)
    }

    #[allow(clippy::unused_self)]
    pub(crate) fn check_function_deprecations(
        &mut self,
        _func: &FunctionCall,
        _args: &ArgumentList,
    ) {
    }

    fn speculative_call_placeholder(
        &self,
        span: crate::diagnostic::Span,
        ident: &str,
        abort_on_error: bool,
        arguments: Vec<Node<FunctionArgument>>,
    ) -> Option<FunctionCall> {
        let func = self.fns.iter().find(|f| f.identifier() == ident)?;
        Some(FunctionCall::new_speculative_placeholder(
            span,
            func.identifier(),
            abort_on_error,
            arguments,
            func.return_kind(),
        ))
    }

    fn compile_function_arguments(
        &mut self,
        arguments: Vec<Node<ast::FunctionArgument>>,
        state: &mut TypeState,
    ) -> Option<Vec<Node<FunctionArgument>>> {
        let saved_loop_depth = self.loop_depth;
        self.loop_depth = 0;
        let compiled = arguments
            .into_iter()
            .map(|node| {
                Some(Node::new(
                    node.span(),
                    self.compile_function_argument(node, state)?,
                ))
            })
            .collect::<Option<Vec<_>>>();
        self.loop_depth = saved_loop_depth;
        compiled
    }

    fn compile_function_call(
        &mut self,
        node: Node<ast::FunctionCall>,
        state: &mut TypeState,
    ) -> Option<FunctionCall> {
        let call_span = node.span();
        let ast::FunctionCall {
            ident,
            abort_on_error,
            arguments,
            closure,
        } = node.into_inner();

        let original_state = state.clone();

        if abort_on_error {
            self.fallible = true;
        }

        let (closure_variables, closure_block) = match closure {
            Some(closure) => {
                let span = closure.span();
                let ast::FunctionClosure { variables, block } = closure.into_inner();
                (Some(Node::new(span, variables)), Some(block))
            }
            None => (None, None),
        };

        // Keep track of the known scope *before* we compile the closure.
        //
        // This allows us to revert to any known state that the closure
        // arguments might overwrite.
        let local_snapshot = state.local.clone();

        // TODO: The state passed into functions should be after function arguments
        //    have resolved, but this will break many functions relying on calling `type_def`
        //    on it's own args.
        // see: https://github.com/vectordotdev/vector/issues/13752
        let state_before_function = original_state.clone();

        let mut speculative_placeholder = None;

        // Compile arguments and run validation under a consuming scope so
        // any inner fallibility produced during arg compilation is dropped
        // when validation either subsumes it (E630/FallibleArgument) or
        // accepts the call (in which case the function's own fallibility
        // is re-pushed below via `result.error`).
        let function_info = self
            .consuming_fallibility(|c| {
                let arguments = c.compile_function_arguments(arguments, state)?;
                let speculative_args = (c.speculative_loop.depth > 0).then(|| arguments.clone());
                let ident_name = ident.as_ref().to_owned();

                function_call::Builder::new(
                    call_span,
                    ident,
                    abort_on_error,
                    arguments,
                    c.fns,
                    &state_before_function,
                    state,
                    closure_variables,
                )
                .map_err(|err| {
                    if matches!(err, FunctionCallError::InvalidArgumentKind(_))
                        && let Some(args) = speculative_args
                    {
                        c.speculative_loop.had_deferred_error = true;
                        speculative_placeholder = c.speculative_call_placeholder(
                            call_span,
                            &ident_name,
                            abort_on_error,
                            args,
                        );
                    }
                    c.diagnostics.push(Box::new(err));
                })
                .ok()
            })
            .and_then(|builder| {
                let block = match closure_block {
                    None => None,
                    Some(block) => {
                        let span = block.span();
                        let saved_loop_depth = self.loop_depth;
                        let prev_breakable = self.in_breakable_context;
                        self.loop_depth = 0;
                        self.in_breakable_context = builder.supports_break();
                        let block_result = self.compile_block_with_type(block, state);
                        self.in_breakable_context = prev_breakable;
                        self.loop_depth = saved_loop_depth;
                        match block_result {
                            Some(block_with_type) => Some(Node::new(span, block_with_type)),
                            None => return None,
                        }
                    }
                };

                let arg_list = builder.get_arg_list().clone();

                builder
                    .compile(
                        &state_before_function,
                        state,
                        block,
                        local_snapshot,
                        &mut self.config,
                    )
                    .map_err(|err| self.diagnostics.push(Box::new(err)))
                    .ok()
                    .map(|result| {
                        self.external_queries.extend(result.target_queries);
                        if let Some(e) = result.error {
                            self.pending_fallibilities
                                .push(CompilerError::FunctionCallError(e));
                        }
                        (arg_list, result.function_call)
                    })
            });

        if let Some((args, function)) = function_info {
            self.check_function_deprecations(&function, &args);
            // Update the final state using the function expression to make sure it's accurate.
            *state = function.type_info(&original_state).state;
            Some(function)
        } else {
            speculative_placeholder
        }
    }

    fn compile_function_argument(
        &mut self,
        node: Node<ast::FunctionArgument>,
        state: &mut TypeState,
    ) -> Option<FunctionArgument> {
        let ast::FunctionArgument {
            ident,
            expr: ast_expr,
        } = node.into_inner();
        let span = ast_expr.span();
        let expr = self.compile_expr(ast_expr, state)?;
        let node = Node::new(span, expr);

        Some(FunctionArgument::new(ident, node))
    }

    fn compile_variable(
        &mut self,
        node: Node<ast::Ident>,
        state: &mut TypeState,
    ) -> Option<Variable> {
        let (span, ident) = node.take();

        if self
            .skip_missing_query_target
            .contains(&(QueryTarget::Internal(ident.clone()), OwnedValuePath::root()))
        {
            return None;
        }

        Variable::new(span, ident, &state.local)
            .map_err(|err| self.diagnostics.push(Box::new(err)))
            .ok()
    }

    fn compile_unary(&mut self, node: Node<ast::Unary>, state: &mut TypeState) -> Option<Unary> {
        use ast::Unary::Not;

        let variant = match node.into_inner() {
            Not(node) => self.compile_not(node, state)?.into(),
        };

        Some(Unary::new(variant))
    }

    fn jump_aware_override_type(
        &mut self,
        expr: &Expr,
        original_state: &TypeState,
    ) -> Option<TypeDef> {
        (self.loop_depth > 0 && expression::for_loop::expr_contains_loop_jump(expr))
            .then(|| self.expr_type_info(expr, original_state).result)
    }

    fn compile_not(&mut self, node: Node<ast::Not>, state: &mut TypeState) -> Option<Not> {
        let original_state = state.clone();
        let (not, expr) = node.into_inner().take();

        let node = Node::new(expr.span(), self.compile_expr(*expr, state)?);
        let override_type_def = self.jump_aware_override_type(node.inner(), &original_state);

        Not::new_with_type_def(node, not.span(), state, override_type_def)
            .map_err(|err| self.diagnostics.push(Box::new(err)))
            .ok()
    }

    fn compile_abort(&mut self, node: Node<ast::Abort>, state: &mut TypeState) -> Option<Abort> {
        self.abortable = true;
        let original_state = state.clone();
        let (span, abort) = node.take();
        self.consuming_fallibility(|c| {
            let message = match abort.message {
                Some(node) => {
                    Some((*node).map_option(|expr| c.compile_expr(Node::new(span, expr), state))?)
                }
                None => None,
            };
            let override_type_def = message
                .as_ref()
                .and_then(|msg| c.jump_aware_override_type(msg.inner(), &original_state));

            Abort::new_with_type_def(span, message, state, override_type_def)
                .map_err(|err| c.diagnostics.push(Box::new(err)))
                .ok()
        })
    }

    fn compile_return(&mut self, node: Node<ast::Return>, state: &mut TypeState) -> Option<Return> {
        let original_state = state.clone();
        let (span, r#return) = node.take();
        self.consuming_fallibility(|c| {
            let expr = c.compile_expr(*r#return.expr, state)?;
            let override_type_def = c.jump_aware_override_type(&expr, &original_state);
            let node = Node::new(span, expr);

            Return::new_with_type_def(span, node, state, override_type_def)
                .map_err(|err| c.diagnostics.push(Box::new(err)))
                .ok()
        })
    }

    fn compile_break(&mut self, node: Node<ast::Break>, _state: &mut TypeState) -> Option<Break> {
        let (span, _break) = node.take();
        if self.loop_depth == 0 && !self.in_breakable_context {
            self.diagnostics.push(Box::new(break_::Error::new(span)));
            return None;
        }
        Some(Break::new(span))
    }

    fn compile_continue(
        &mut self,
        node: Node<ast::Continue>,
        _state: &mut TypeState,
    ) -> Option<Continue> {
        let (span, _continue) = node.take();
        if self.loop_depth == 0 {
            self.diagnostics
                .push(Box::new(expression::continue_::Error::new(span)));
            return None;
        }
        Some(Continue::new(span))
    }

    fn compile_for_body_pass(
        &mut self,
        pattern: &ast::ForPattern,
        iterable_kind: &crate::value::Kind,
        block: Node<ast::Block>,
        header_state: TypeState,
        state: &mut TypeState,
        speculative: bool,
    ) -> Option<Block> {
        *state = header_state;
        expression::for_loop::bind_for_pattern(pattern, iterable_kind, state);

        let prev_breakable = self.in_breakable_context;
        self.in_breakable_context = false;
        self.loop_depth += 1;
        if speculative {
            self.speculative_loop.depth += 1;
        }
        let compiled_block = self.compile_block(block, state);
        if speculative {
            self.speculative_loop.depth -= 1;
        }
        self.loop_depth -= 1;
        self.in_breakable_context = prev_breakable;
        compiled_block
    }

    fn validate_for_header(
        &mut self,
        pattern: &ast::ForPattern,
        iterable_span: crate::diagnostic::Span,
        iterable_kind: &crate::value::Kind,
    ) -> bool {
        // Reject if iterable contains any non-collection type or contains neither array nor object.
        // Exempt the bottom type (`never`), e.g. iterating over an element of a statically empty collection.
        if !iterable_kind.is_never()
            && (expression::for_loop::contains_non_collection_kind(iterable_kind)
                || (!iterable_kind.contains_array() && !iterable_kind.contains_object()))
        {
            self.diagnostics
                .push(Box::new(expression::for_loop::Error::non_collection(
                    iterable_span,
                    iterable_kind.clone(),
                )));
            return false;
        }

        // Single-variable pattern is only valid if object iteration is impossible.
        // If iterable can be an object, require key-value pattern: `for key, value in ...`
        if let ast::ForPattern::Single(val_node) = pattern
            && !iterable_kind.is_never()
            && iterable_kind.contains_object()
        {
            self.diagnostics
                .push(Box::new(expression::for_loop::Error::single_var_object(
                    val_node.span(),
                )));
            return false;
        }

        // For KeyValue pattern: reject duplicate variable names.
        if let ast::ForPattern::KeyValue(key_node, val_node) = pattern {
            let k = key_node.inner();
            let v = val_node.inner();
            if k.as_ref() == v.as_ref() && k.as_ref() != "_" && !k.as_ref().is_empty() {
                self.diagnostics.push(Box::new(
                    expression::for_loop::Error::duplicate_pattern_ident(
                        val_node.span(),
                        k.to_string(),
                    ),
                ));
                return false;
            }
        }

        true
    }

    fn lookup_cached_for(
        &mut self,
        for_span: crate::diagnostic::Span,
        key: &ForCacheKey,
        original_state: &TypeState,
        state: &mut TypeState,
    ) -> Option<For> {
        let cached = self
            .for_loop_cache
            .get(&for_span)?
            .iter()
            .find(|entry| &entry.key == key)?;
        self.external_queries
            .extend(cached.external_queries.iter().cloned());
        self.external_assignments
            .extend(cached.external_assignments.iter().cloned());
        self.fallible |= cached.fallible;
        self.abortable |= cached.abortable;
        *state = cached.output_patch.apply(original_state);
        Some(cached.for_expr.clone())
    }

    #[allow(clippy::too_many_arguments)]
    fn cache_compiled_for(
        &mut self,
        for_span: crate::diagnostic::Span,
        key: ForCacheKey,
        footprint: &expression::for_loop::LoopFootprint,
        entry_snapshot: &CompilerSnapshot,
        for_expr: &For,
        output_state: &TypeState,
    ) {
        let bucket = self.for_loop_cache.entry(for_span).or_default();
        if bucket.len() >= expression::for_loop::MAX_CACHE_ENTRIES_PER_LOOP
            || bucket.iter().any(|entry| entry.key == key)
        {
            return;
        }
        bucket.push(CachedCompiledFor {
            key,
            for_expr: for_expr.clone(),
            output_patch: footprint.patch_from(output_state),
            external_queries: self.external_queries[entry_snapshot.external_queries_len..].to_vec(),
            external_assignments: self.external_assignments
                [entry_snapshot.external_assignments_len..]
                .to_vec(),
            fallible: !entry_snapshot.fallible && self.fallible,
            abortable: !entry_snapshot.abortable && self.abortable,
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn compile_for_fixed_point(
        &mut self,
        for_span: crate::diagnostic::Span,
        pattern: &ast::ForPattern,
        iterable_kind: &crate::value::Kind,
        compiled_iterable: &Expr,
        block: Node<ast::Block>,
        original_state: &TypeState,
        pre_loop_state: TypeState,
        state: &mut TypeState,
    ) -> Option<(For, TypeState, bool, bool)> {
        let body_footprint =
            expression::for_loop::LoopFootprint::for_ast_body(pattern, block.inner());
        let snapshot = self.snapshot();
        let mut current_pre_loop_state = pre_loop_state;
        let mut budget_widened = self.loop_eval_ctx.is_exhausted();
        if budget_widened {
            body_footprint.widen_written_state(&mut current_pre_loop_state);
        }
        let mut pass_iterations = 0;

        loop {
            self.restore_snapshot(&snapshot);
            self.speculative_loop.had_deferred_error = false;
            let compiled_block = self.compile_for_body_pass(
                pattern,
                iterable_kind,
                block.clone(),
                current_pre_loop_state.clone(),
                state,
                true,
            )?;
            let pass_had_speculative_error = self.speculative_loop.had_deferred_error;

            let for_expr = For::new(
                for_span,
                pattern.clone(),
                Box::new(compiled_iterable.clone()),
                compiled_block,
            );

            // Compute the multi-iteration header and post-loop states via `For::resolve_loop_type_info_with_ctx`,
            // sharing our single `LoopEvalContext` cache and repass budget across nested loops.
            // Invalidate any cached summary for `for_span` since `compiled_block` may have changed.
            self.loop_eval_ctx.invalidate(for_span);
            let (header_state, loop_type_info) =
                for_expr.resolve_loop_type_info_with_ctx(original_state, &mut self.loop_eval_ctx);
            let mut next_pre_loop_state = current_pre_loop_state.clone().merge(header_state);
            expression::for_loop::canonicalize_type_state(&mut next_pre_loop_state);

            if next_pre_loop_state == current_pre_loop_state || budget_widened {
                self.speculative_loop.had_deferred_error |= snapshot.speculative_deferred_error;
                if pass_had_speculative_error && self.speculative_loop.depth == 0 {
                    // Re-compile non-speculatively at the converged header state so diagnostics
                    // reflect the real errors without cascading from speculative placeholders.
                    self.restore_snapshot(&snapshot);
                    let _ = self.compile_for_body_pass(
                        pattern,
                        iterable_kind,
                        block,
                        current_pre_loop_state,
                        state,
                        false,
                    );
                    return None;
                }
                return Some((
                    for_expr,
                    loop_type_info.state,
                    pass_had_speculative_error,
                    budget_widened,
                ));
            }

            if !self.loop_eval_ctx.consume_repass() {
                body_footprint.widen_written_state(&mut next_pre_loop_state);
                budget_widened = true;
                current_pre_loop_state = next_pre_loop_state;
                continue;
            }

            pass_iterations += 1;
            if pass_iterations >= expression::for_loop::WIDEN_AFTER_ITERATIONS {
                expression::for_loop::widen_differing_states(
                    &current_pre_loop_state,
                    &mut next_pre_loop_state,
                    pass_iterations >= expression::for_loop::FALLBACK_WIDEN_ITERATIONS,
                );
            }
            current_pre_loop_state = next_pre_loop_state;
        }
    }

    fn compile_for_statement(
        &mut self,
        node: Node<ast::ForStatement>,
        state: &mut TypeState,
    ) -> Option<For> {
        let original_state = state.clone();
        let entry_snapshot = self.snapshot();
        let (for_span, for_statement) = node.take();
        let ast_footprint = expression::for_loop::LoopFootprint::for_ast(&for_statement);
        let can_cache = !self.config.has_custom()
            && !ast_footprint.opaque
            && self.skip_missing_query_target.is_empty();
        let cache_key = can_cache.then(|| ForCacheKey {
            input: ast_footprint.project(&original_state),
            in_loop: self.loop_depth > 0,
            in_breakable: self.in_breakable_context,
        });

        if let Some(ref key) = cache_key
            && let Some(cached) = self.lookup_cached_for(for_span, key, &original_state, state)
        {
            return Some(cached);
        }

        if self.speculative_loop.depth == 0 {
            self.loop_eval_ctx.reset();
        }

        let ast::ForStatement {
            pattern,
            iterable,
            block,
        } = for_statement;

        let iterable_span = iterable.span();
        let compiled_iterable = self.compile_expr(*iterable, state)?;
        let crate::compiler::state::TypeInfo {
            state: mut pre_loop_state,
            result: iterable_type,
        } = self.expr_type_info(&compiled_iterable, &original_state);
        let iterable_kind = iterable_type.kind();

        if !self.validate_for_header(&pattern, iterable_span, iterable_kind) {
            return None;
        }

        expression::for_loop::canonicalize_type_state(&mut pre_loop_state);
        let (for_expr, final_state, pass_had_speculative_error, budget_widened) = self
            .compile_for_fixed_point(
                for_span,
                &pattern,
                iterable_kind,
                &compiled_iterable,
                block,
                &original_state,
                pre_loop_state,
                state,
            )?;

        *state = if self.loop_depth > 0
            && expression::for_loop::expr_contains_loop_jump(&Expr::For(for_expr.clone()))
        {
            self.expr_type_info(&Expr::For(for_expr.clone()), &original_state)
                .state
        } else {
            final_state
        };

        if let Some(key) = cache_key
            && !pass_had_speculative_error
            && !budget_widened
            && self.diagnostics.len() == entry_snapshot.diagnostics_len
            && self.pending_fallibilities.len() == entry_snapshot.pending_fallibilities_len
            && self.skip_missing_query_target.len() == entry_snapshot.skip_missing_query_target_len
        {
            self.cache_compiled_for(
                for_span,
                key,
                &ast_footprint,
                &entry_snapshot,
                &for_expr,
                state,
            );
        }

        Some(for_expr)
    }

    fn handle_parser_error(&mut self, error: crate::parser::Error) {
        self.diagnostics.push(Box::new(error));
    }

    fn skip_missing_assignment_target(&mut self, target: &ast::AssignmentTarget) {
        let query = match &target {
            ast::AssignmentTarget::Noop => return,
            ast::AssignmentTarget::Query(ast::Query { target, path }) => {
                (target.clone().into_inner(), path.clone().into_inner())
            }
            ast::AssignmentTarget::Internal(ident, path) => (
                QueryTarget::Internal(ident.clone()),
                path.clone().unwrap_or_else(OwnedValuePath::root),
            ),
            ast::AssignmentTarget::External(path) => {
                let prefix = path.as_ref().map_or(PathPrefix::Event, |x| x.prefix);
                let path = path.clone().map_or_else(OwnedValuePath::root, |x| x.path);
                (QueryTarget::External(prefix), path)
            }
        };

        self.skip_missing_query_target.push(query);
    }
}

#[cfg(all(test, feature = "stdlib"))]
mod tests {
    use crate::{owned_value_path, path::OwnedTargetPath};

    fn target_queries(source: &str) -> Vec<OwnedTargetPath> {
        let fns = crate::stdlib::all();
        crate::compiler::compile(source, &fns)
            .unwrap()
            .program
            .info()
            .target_queries
            .clone()
    }

    #[test]
    fn get_records_only_the_paths_it_reads() {
        let tags = OwnedTargetPath::event(owned_value_path!("tags"));
        for source in [
            r#"get!(.tags, ["region"])"#,
            r#"get!(value: .tags, path: ["region"])"#,
            r#"get!(path: ["region"], value: .tags)"#,
            r#"tags = .tags; get!(tags, ["region"])"#,
        ] {
            assert_eq!(target_queries(source), vec![tags.clone()], "{source}");
        }

        assert_eq!(
            target_queries(r#"get!(., ["tags", "region"])"#),
            vec![OwnedTargetPath::event_root()]
        );
        assert_eq!(
            target_queries(r#"get!(%custom, ["a"])"#),
            vec![OwnedTargetPath::metadata(owned_value_path!("custom"))]
        );
    }

    #[test]
    fn unnest_records_the_root_of_its_target() {
        // `unnest` keeps the other fields of the target around each element.
        for source in ["unnest!(.tags.region)", "unnest!(path: .tags.region)"] {
            let queries = target_queries(source);
            assert!(
                queries.contains(&OwnedTargetPath::event_root()),
                "{source}: {queries:?}"
            );
        }

        let queries = target_queries("unnest!(%items)");
        assert!(
            queries.contains(&OwnedTargetPath::metadata_root()),
            "{queries:?}"
        );

        // A variable doesn't read the target.
        assert_eq!(
            target_queries(r#"obj = {"items": [1, 2]}; unnest!(obj.items)"#),
            Vec::<OwnedTargetPath>::new()
        );
    }

    /// A host function that happens to be called `unnest`, but only returns its argument.
    #[derive(Debug, Clone)]
    struct HostUnnest;

    impl crate::compiler::prelude::Function for HostUnnest {
        fn identifier(&self) -> &'static str {
            "unnest"
        }

        fn description(&self) -> &'static str {
            "test"
        }

        fn category(&self) -> &'static str {
            "test"
        }

        fn return_kind(&self) -> u16 {
            crate::compiler::prelude::kind::ANY
        }

        fn examples(&self) -> &'static [crate::compiler::prelude::Example] {
            &[]
        }

        fn parameters(&self) -> &'static [crate::compiler::prelude::Parameter] {
            use crate::compiler::prelude::{Parameter, kind};
            const PARAMETERS: &[Parameter] = &[Parameter::required("value", kind::ANY, "value")];
            PARAMETERS
        }

        fn compile(
            &self,
            _state: &crate::compiler::prelude::TypeState,
            _ctx: &mut crate::compiler::prelude::FunctionCompileContext,
            arguments: crate::compiler::prelude::ArgumentList,
        ) -> crate::compiler::prelude::Compiled {
            Ok(arguments.required("value"))
        }
    }

    #[test]
    fn host_function_named_unnest_records_only_its_arguments() {
        let fns: Vec<Box<dyn crate::compiler::prelude::Function>> = vec![Box::new(HostUnnest)];
        let tags = OwnedTargetPath::event(owned_value_path!("tags"));
        for source in ["unnest(.tags)", "unnest(value: .tags)"] {
            let queries = crate::compiler::compile(source, &fns)
                .unwrap()
                .program
                .info()
                .target_queries
                .clone();
            assert_eq!(queries, vec![tags.clone()], "{source}");
        }
    }
}
