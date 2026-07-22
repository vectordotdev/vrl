use crate::compiler::expression::ExpressionError;
use crate::compiler::expression::function_call::FunctionCallError;
use crate::compiler::{
    CompileConfig, Function, Program, TypeDef,
    expression::{
        Abort, Array, Assignment, Block, Container, Expr, Expression, FunctionArgument,
        FunctionCall, Group, IfArm, IfStatement, Literal, Noop, Not, Object, Op, Predicate, Query, Return,
        Target, Unary, Variable, assignment, function_call, literal, predicate, query,
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
            config,
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

    fn compile_expr(&mut self, node: Node<ast::Expr>, state: &mut TypeState) -> Option<Expr> {
        use ast::Expr::{
            Abort, Assignment, Container, FunctionCall, IfStatement, Literal, Op, Query, Return,
            Unary, Variable,
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
        }?;

        // If the compiled expression is fallible and no sub-expression has
        // already pushed an entry for it, record this expression as the
        // outer-most fallible point in the current chain. Avoiding double
        // counting (e.g. `a + b()` where b is fallible already pushed) means
        // checking that the pending stack didn't grow during compilation.
        let type_def = expr.type_info(&original_state).result;
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
        use bytes::Bytes;

        let (span, lit) = node.take();

        let literal = match lit {
            String(template) => {
                if let Some(v) = template.as_literal_string() {
                    Ok(Literal::String(Bytes::from(v.to_string())))
                } else {
                    // Rewrite the template into an expression and compile that block.
                    return self.compile_expr(
                        Node::new(span, template.rewrite_to_concatenated_strings()),
                        state,
                    );
                }
            }
            RawString(v) => Ok(Literal::String(Bytes::from(v))),
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

    fn compile_block_with_type(
        &mut self,
        node: Node<ast::Block>,
        state: &mut TypeState,
    ) -> Option<(Block, TypeDef)> {
        let original_state = state.clone();
        let exprs = self.compile_exprs(node.into_inner(), state)?;
        let block = Block::new_scoped(exprs);

        // The type information from `compile_exprs` doesn't applying the "scoping" from the block.
        // This is recalculated using the block.
        *state = original_state;
        let result = block.apply_type_info(state);
        Some((block, result))
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

        // Peel parser-emitted `else { if ... }` chains into a flat arm list so
        // typecheck visits each arm once (nested form re-walked suffixes).
        let (ast_arms, final_else) = flatten_ast_else_if_chain(predicate, if_node, else_node);

        let original_state = state.clone();
        let mut pred_state = original_state.clone();
        let mut arms = Vec::with_capacity(ast_arms.len());

        for (predicate, if_node) in ast_arms {
            *state = pred_state.clone();

            let predicate = self
                .compile_predicate(predicate, state)?
                .map_err(|err| self.diagnostics.push(Box::new(err)))
                .ok()?;

            pred_state = state.clone();
            let block = self.compile_block(if_node, state)?;
            arms.push(IfArm { predicate, block });
        }

        let else_block = if let Some(else_node) = final_else {
            *state = pred_state;
            Some(self.compile_block(else_node, state)?)
        } else {
            None
        };

        let if_statement = IfStatement::new(arms, else_block);

        // Restore and calculate type state from the full flat if expression once.
        *state = original_state;
        if_statement.apply_type_info(state);
        Some(if_statement)
    }

    fn compile_predicate(
        &mut self,
        node: Node<ast::Predicate>,
        state: &mut TypeState,
    ) -> Option<predicate::Result> {
        use ast::Predicate::{Many, One};

        let (span, predicate) = node.take();
        let pre_pending = self.pending_fallibilities.len();

        self.consuming_fallibility(|c| {
            let exprs = match predicate {
                One(node) => vec![c.compile_expr(*node, state)?],
                Many(nodes) => c.compile_exprs(nodes, state)?,
            };

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
        let rhs = Node::new(rhs_span, self.compile_expr(*rhs, state)?);

        let op = match Op::new(lhs, opcode, rhs, state) {
            Ok(op) => op,
            Err(err) => {
                // The op itself failed (e.g. `1 ?? x` is rejected as
                // unnecessary error coalescing). Sub-expression fallibilities
                // are subsumed by this hard error — don't double-report.
                self.pending_fallibilities.truncate(pre_op_pending);
                self.diagnostics.push(Box::new(err));
                return None;
            }
        };

        let type_info = op.type_info(&original_state);

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

        // The fallibility relevant to `Assignment::new`'s check is whatever
        // was produced by *this* assignment's RHS — i.e. the first entry past
        // the snapshot. Anything before that belongs to an earlier expression.
        let rhs_fallibility = self.pending_fallibilities.get(pre_assignment_pending);
        let assignment_result = Assignment::new(node, state, rhs_fallibility, &self.config);

        let assignment = match assignment_result {
            Ok(a) => a,
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
        *state = original_state;
        assignment.apply_type_info(state);

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
        // TODO: Remove this (hacky) code once dynamic path syntax lands.
        //
        // See: https://github.com/vectordotdev/vector/issues/12547
        if ident.as_deref() == "get" {
            self.external_queries.push(OwnedTargetPath::event_root());
        }

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

        // Compile arguments and run validation under a consuming scope so
        // any inner fallibility produced during arg compilation is dropped
        // when validation either subsumes it (E630/FallibleArgument) or
        // accepts the call (in which case the function's own fallibility
        // is re-pushed below via `result.error`).
        let function_info = self
            .consuming_fallibility(|c| {
                let arguments: Vec<_> = arguments
                    .into_iter()
                    .map(|node| {
                        Some(Node::new(
                            node.span(),
                            c.compile_function_argument(node, state)?,
                        ))
                    })
                    .collect::<Option<_>>()?;

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
                .map_err(|err| c.diagnostics.push(Box::new(err)))
                .ok()
            })
            .and_then(|builder| {
                let block = match closure_block {
                    None => None,
                    Some(block) => {
                        let span = block.span();
                        match self.compile_block_with_type(block, state) {
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
                        if let Some(e) = result.error {
                            self.pending_fallibilities
                                .push(CompilerError::FunctionCallError(e));
                        }
                        (arg_list, result.function_call)
                    })
            });

        if let Some((args, function)) = &function_info {
            self.check_function_deprecations(function, args);
            // Update the final state using the function expression to make sure it's accurate.
            *state = function.type_info(&original_state).state;
        }

        function_info.map(|info| info.1)
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

    fn compile_not(&mut self, node: Node<ast::Not>, state: &mut TypeState) -> Option<Not> {
        let (not, expr) = node.into_inner().take();

        let node = Node::new(expr.span(), self.compile_expr(*expr, state)?);

        Not::new(node, not.span(), state)
            .map_err(|err| self.diagnostics.push(Box::new(err)))
            .ok()
    }

    fn compile_abort(&mut self, node: Node<ast::Abort>, state: &mut TypeState) -> Option<Abort> {
        self.abortable = true;
        let (span, abort) = node.take();
        self.consuming_fallibility(|c| {
            let message = match abort.message {
                Some(node) => {
                    Some((*node).map_option(|expr| c.compile_expr(Node::new(span, expr), state))?)
                }
                None => None,
            };

            Abort::new(span, message, state)
                .map_err(|err| c.diagnostics.push(Box::new(err)))
                .ok()
        })
    }

    fn compile_return(&mut self, node: Node<ast::Return>, state: &mut TypeState) -> Option<Return> {
        let (span, r#return) = node.take();
        self.consuming_fallibility(|c| {
            let expr = c.compile_expr(*r#return.expr, state)?;
            let node = Node::new(span, expr);

            Return::new(span, node, state)
                .map_err(|err| c.diagnostics.push(Box::new(err)))
                .ok()
        })
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

/// Peel parser `else { if ... }` nesting into a flat list of arms.
///
/// The VRL parser builds `else if` as `else` wrapping a block that contains a
/// single nested `if`. Walking that nest during compile/typecheck re-visits
/// suffixes and is quadratic in arm count.
fn flatten_ast_else_if_chain(
    predicate: Node<ast::Predicate>,
    if_node: Node<ast::Block>,
    mut else_node: Option<Node<ast::Block>>,
) -> (
    Vec<(Node<ast::Predicate>, Node<ast::Block>)>,
    Option<Node<ast::Block>>,
) {
    let mut arms = vec![(predicate, if_node)];

    loop {
        let Some(else_block) = else_node.take() else {
            return (arms, None);
        };

        let is_else_if = matches!(
            else_block.inner().0.as_slice(),
            [stmt] if matches!(stmt.inner(), ast::Expr::IfStatement(_))
        );

        if !is_else_if {
            return (arms, Some(else_block));
        }

        let mut stmts = else_block.into_inner().into_inner();
        let only = stmts.pop().expect("checked single stmt");
        let ast::Expr::IfStatement(inner) = only.into_inner() else {
            unreachable!("checked IfStatement");
        };
        let inner = inner.into_inner();
        arms.push((inner.predicate, inner.if_node));
        else_node = inner.else_node;
    }
}
