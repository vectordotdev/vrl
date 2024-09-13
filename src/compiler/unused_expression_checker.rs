/// # Unused Expression Checker
///
/// This module provides functionality for traversing VRL (Vector Remap Language) Abstract Syntax Trees (AST).
/// It's designed to detect and report unused expressions, helping users clean up and optimize their VRL scripts.
/// Initially, it will generate warnings for unused expressions. These warnings might be escalated to errors
/// in future versions, once the module has been battle-tested.
///
/// ## How it works
///
/// - **Traversal**: Recursively explores each node of the AST. This process begins after the program has
///   been successfully compiled.
/// - **Stateful Context**: Builds context on the fly to determine whether an expression is unused. The context
///   takes into account variable scopes, assignments, and the flow of the program.
/// - **Detection**: Identifies and reports expressions that do not contribute to assignments,
///   affect external events, or influence the outcome of function calls.
/// - **Ignored Variables**: Variable names prefixed with '_' are ignored.
///
/// ## Caveats
/// - **Closures**: Closure support is minimal. For now, we are only ensuring that there are no false positives.
/// - **Variable Shadowing**: Variable shadowing is not supported. Unused variables will not be detected in this case.
use crate::compiler::codes::WARNING_UNUSED_CODE;
use crate::compiler::parser::{Ident, Node};
use crate::diagnostic::{Diagnostic, DiagnosticList, Label, Note, Severity};
use crate::parser::ast::{
    Array, Assignment, AssignmentOp, AssignmentTarget, Block, Container, Expr, FunctionCall,
    IfStatement, Object, Predicate, QueryTarget, Return, RootExpr, Unary,
};
use crate::parser::template_string::StringSegment;
use crate::parser::{Literal, Program, Span};
use std::collections::{BTreeMap, HashMap};
use tracing::warn;

#[must_use]
pub fn check_for_unused_results(ast: &Program) -> DiagnosticList {
    let expression_visitor = AstVisitor { ast };
    expression_visitor.check_for_unused_results()
}

pub struct AstVisitor<'a> {
    ast: &'a Program,
}

#[derive(Default, Debug, Clone)]
struct IdentState {
    span: Span,
    pending_usage: bool,
}

#[derive(Default, Debug, Clone)]
struct VisitorState {
    level: usize,
    expecting_result: HashMap<usize, bool>,
    within_block_expression: HashMap<usize, bool>,
    ident_to_state: BTreeMap<Ident, IdentState>,
    diagnostics: DiagnosticList,
}

impl VisitorState {
    fn is_unused(&self) -> bool {
        let pending_result = self
            .expecting_result
            .get(&self.level)
            .is_some_and(|active| *active);
        !pending_result
    }

    fn is_within_block(&self) -> bool {
        self.within_block_expression
            .get(&self.level)
            .is_some_and(|within_block| *within_block)
    }

    fn increase_level(&mut self) {
        self.level += 1;
    }

    fn decrease_level(&mut self) {
        self.level -= 1;
    }

    fn enter_block(&mut self) {
        self.increase_level();
        self.within_block_expression.insert(self.level, true);
    }

    fn exiting_block(&mut self) {
        self.within_block_expression.insert(self.level, false);
        self.decrease_level();
    }

    fn mark_level_as_expecting_result(&mut self) {
        self.expecting_result.insert(self.level, true);
    }

    fn mark_level_as_not_expecting_result(&mut self) {
        self.expecting_result.insert(self.level, false);
    }

    fn mark_identifier_pending_usage(&mut self, ident: &Ident, span: &Span) {
        if ident.is_empty() || ident.starts_with('_') {
            return;
        }

        self.ident_to_state
            .entry(ident.clone())
            .and_modify(|state| {
                state.pending_usage = true;
            })
            .or_insert(IdentState {
                span: *span,
                pending_usage: true,
            });
    }

    fn mark_identifier_used(&mut self, ident: &Ident) {
        if ident.is_empty() || ident.starts_with('_') {
            return;
        }

        if let Some(entry) = self.ident_to_state.get_mut(ident) {
            entry.pending_usage = false;
        } else {
            warn!("unexpected identifier `{}` reported as used", ident);
        }
    }

    fn mark_query_target_pending_usage(&mut self, query_target: &Node<QueryTarget>) {
        match &query_target.node {
            QueryTarget::Internal(ident) => {
                self.mark_identifier_pending_usage(ident, &query_target.span);
            }
            QueryTarget::External(_) => {}
            QueryTarget::FunctionCall(_) => {}
            QueryTarget::Container(_) => {}
        }
    }

    fn append_diagnostic(&mut self, message: String, span: &Span) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: WARNING_UNUSED_CODE,
            message,
            labels: vec![Label::primary(
                "help: use the result of this expression or remove it",
                span,
            )],
            notes: vec![Note::Basic(
                "this expression has no side-effects".to_owned(),
            )],
        })
    }

    fn extend_diagnostics_for_unused_variables(&mut self) {
        for (ident, state) in self.ident_to_state.clone() {
            if state.pending_usage {
                self.append_diagnostic(format!("unused variable `{ident}`"), &state.span);
            }
        }
    }
}

fn scoped_visit(state: &mut VisitorState, f: impl FnOnce(&mut VisitorState)) {
    state.increase_level();
    state.mark_level_as_expecting_result();
    f(state);
    state.mark_level_as_not_expecting_result();
    state.decrease_level();
}

impl AstVisitor<'_> {
    fn visit_node(&self, node: &Node<Expr>, state: &mut VisitorState) {
        let expression = node.inner();

        match expression {
            Expr::Literal(literal) => {
                if let Literal::String(template) = &literal.node {
                    for segment in &template.0 {
                        if let StringSegment::Template(ident, _) = segment {
                            state.mark_identifier_used(&Ident::from(ident.clone()));
                        }
                    }
                }
                if state.is_unused() {
                    state.append_diagnostic(format!("unused literal `{literal}`"), &node.span());
                }
            }
            Expr::Container(container) => {
                self.visit_container(container, state);
            }
            Expr::IfStatement(if_statement) => {
                scoped_visit(state, |state| {
                    self.visit_if_statement(if_statement, state);
                });
            }
            Expr::Op(op) => {
                self.visit_node(&op.0, state);
                scoped_visit(state, |state| {
                    self.visit_node(&op.2, state);
                });
            }
            Expr::Unary(unary) => match &unary.node {
                Unary::Not(not) => {
                    self.visit_node(&not.1, state);
                }
            },
            Expr::Assignment(assignment) => {
                self.visit_assignment(assignment, state);
            }
            Expr::Query(query) => match &query.node.target.node {
                QueryTarget::Internal(ident) => {
                    if !state.is_unused() {
                        state.mark_identifier_used(ident);
                    }
                }
                QueryTarget::External(_) => {}
                QueryTarget::FunctionCall(function_call) => {
                    self.visit_function_call(function_call, &query.node.target.span, state)
                }
                QueryTarget::Container(_) => {}
            },
            Expr::FunctionCall(function_call) => {
                self.visit_function_call(function_call, &function_call.span, state)
            }
            Expr::Variable(variable) => {
                state.mark_identifier_used(&variable.node);
            }
            Expr::Abort(_) => {}
            Expr::Return(r#return) => self.visit_return(r#return, state),
        }
    }

    fn visit_container(&self, node: &Node<Container>, state: &mut VisitorState) {
        match &node.node {
            Container::Group(group) => self.visit_node(&group.node.0, state),
            Container::Block(block) => self.visit_block(block, state),
            Container::Array(array) => self.visit_array(array, state),
            Container::Object(object) => self.visit_object(object, state),
        }
    }

    fn visit_array(&self, array: &Node<Array>, state: &mut VisitorState) {
        for expr in &array.0 {
            self.visit_node(expr, state);
        }
    }

    fn visit_block(&self, block: &Node<Block>, state: &mut VisitorState) {
        let block_expressions = &block.node.0;
        if block_expressions.is_empty() {
            return;
        }
        state.enter_block();

        for (i, expr) in block_expressions.iter().enumerate() {
            if i == block_expressions.len() - 1 {
                state.exiting_block();
            }
            self.visit_node(expr, state);
        }
    }

    fn visit_object(&self, object: &Node<Object>, state: &mut VisitorState) {
        if state.is_unused() {
            state.append_diagnostic(format!("unused object `{object}`"), &object.span);
        }
        for value in object.0.values() {
            scoped_visit(state, |state| {
                self.visit_node(value, state);
            });
        }
    }

    fn visit_if_statement(&self, if_statement: &Node<IfStatement>, state: &mut VisitorState) {
        match &if_statement.predicate.node {
            Predicate::One(expr) => self.visit_node(expr, state),
            Predicate::Many(exprs) => {
                for expr in exprs {
                    self.visit_node(expr, state);
                }
            }
        }

        scoped_visit(state, |state| {
            self.visit_block(&if_statement.if_node, state);
        });

        if let Some(else_block) = &if_statement.else_node {
            scoped_visit(state, |state| {
                self.visit_block(else_block, state);
            });
        }
    }

    fn visit_assignment(&self, assignment: &Node<Assignment>, state: &mut VisitorState) {
        state.increase_level();
        let level = state.level;
        state.expecting_result.insert(level, true);

        // All targets needs to be used later.
        let (op, targets) = match &assignment.node {
            Assignment::Single { target, op, .. } => (op, vec![target]),
            Assignment::Infallible { ok, err, op, .. } => (op, vec![ok, err]),
        };
        for target in targets {
            match &target.node {
                AssignmentTarget::Noop => {}
                AssignmentTarget::Query(query) => {
                    state.mark_query_target_pending_usage(&query.target);
                }
                AssignmentTarget::Internal(ident, path) => {
                    if *op == AssignmentOp::Assign && path.is_none() {
                        state.mark_identifier_pending_usage(ident, &target.span);
                    } else if *op == AssignmentOp::Merge {
                        // The following example: `x |= {}` falls under shadowing and is not handled.
                        state.mark_identifier_used(ident);
                    }
                }
                AssignmentTarget::External(_path) => {}
            }
        }

        // Visit the assignment right hand side.
        match &assignment.node {
            Assignment::Single { expr, .. } => {
                self.visit_node(expr, state);
            }
            Assignment::Infallible { expr, .. } => {
                self.visit_node(expr, state);
            }
        }
        state.expecting_result.insert(level, false);
        state.decrease_level();
    }

    fn visit_function_call(
        &self,
        function_call: &FunctionCall,
        span: &Span,
        state: &mut VisitorState,
    ) {
        for argument in &function_call.arguments {
            state.increase_level();
            state.mark_level_as_expecting_result();
            self.visit_node(&argument.node.expr, state);
            state.mark_level_as_not_expecting_result();
            state.decrease_level();
        }

        // This function call might be part of fallible block.
        if !function_call.abort_on_error && state.is_within_block() {
            state.mark_level_as_expecting_result();
        }

        match function_call.ident.0.as_str() {
            //  All bets are off for functions with side-effects.
            "del" | "log" | "assert" | "assert_eq" => (),
            _ => {
                if let Some(closure) = &function_call.closure {
                    for variable in &closure.variables {
                        state.mark_identifier_pending_usage(&variable.node, &variable.span);
                    }
                    state.mark_level_as_expecting_result();
                    self.visit_block(&closure.block, state);
                    state.mark_level_as_not_expecting_result();
                } else if state.is_unused() {
                    state.append_diagnostic(
                        format!("unused result for function call `{function_call}`"),
                        span,
                    );
                }
            }
        }

        if !function_call.abort_on_error && state.is_within_block() {
            state.mark_level_as_not_expecting_result();
        }
    }

    fn visit_return(&self, r#return: &Node<Return>, state: &mut VisitorState) {
        state.increase_level();
        let level = state.level;
        state.expecting_result.insert(level, true);
        self.visit_node(&r#return.node.expr, state);
        state.expecting_result.insert(level, false);
        state.decrease_level();
    }

    /// This function traverses the VRL AST and detects unused results.
    /// An expression might have side-effects, in that case we do not except its result to be used.
    ///
    /// We want to detect the following cases:
    /// * Unused Variables: a variable which is assigned a value but never used in any expression
    /// * Unused Expressions: an expression without side-effects with an unused result
    fn check_for_unused_results(&self) -> DiagnosticList {
        let mut unused_warnings = DiagnosticList::default();
        let mut state = VisitorState::default();
        let root_expressions = &self.ast.0;
        for (i, root_node) in root_expressions.iter().enumerate() {
            let is_last = i == root_expressions.len() - 1;
            if is_last {
                state.increase_level();
                state.mark_level_as_expecting_result();
            }
            match root_node.inner() {
                RootExpr::Expr(node) => self.visit_node(node, &mut state),
                RootExpr::Error(_) => {}
            }
            if is_last {
                state.decrease_level();
                state.mark_level_as_not_expecting_result();
            }
        }
        state.extend_diagnostics_for_unused_variables();
        unused_warnings.extend(state.diagnostics);
        unused_warnings
    }
}

#[cfg(test)]
mod test {
    use crate::compiler::codes::WARNING_UNUSED_CODE;
    use crate::stdlib;
    use indoc::indoc;

    fn unused_test(source: &str, expected_warnings: Vec<String>) {
        let warnings = crate::compiler::compile(source, &stdlib::all())
            .unwrap()
            .warnings;

        assert_eq!(warnings.len(), expected_warnings.len());

        for (i, content) in expected_warnings.iter().enumerate() {
            let warning = warnings.get(i).unwrap();
            assert_eq!(warning.code, WARNING_UNUSED_CODE);
            assert!(
                warning.message.contains(content),
                "expected message `{}` to contain `{content}`",
                warning.message
            );
        }
    }

    #[test]
    fn unused_top_level_literal() {
        let source = indoc! {r#"
            "foo"
            "program result"
        "#};
        unused_test(source, vec![r#"unused literal `"foo"`"#.to_string()]);
    }

    #[test]
    fn unused_variable_in_assignment() {
        let source = indoc! {"
            foo = 5
        "};
        unused_test(source, vec!["unused variable `foo`".to_string()]);
    }

    #[test]
    fn unused_literal() {
        let source = indoc! {r#"
            . = {
                "unused"
                "a"
            }
        "#};
        unused_test(source, vec![r#"unused literal `"unused"`"#.to_string()]);
    }

    #[test]
    fn unused_top_level_variable() {
        let source = indoc! {r#"
            x = "bar"
        "#};
        unused_test(source, vec!["unused variable `x`".to_string()]);
    }

    #[test]
    fn test_nested_blocks() {
        let source = indoc! {r#"
            . = {
                "1"
                {
                    "2"
                    {
                        "3"
                    }
                }

                . = {{{ x = 42; x }}}

                "4"
                "5"
            }
        "#};

        let expected_warnings: Vec<String> = (1..5)
            .map(|i| format!("unused literal `\"{i}\"`"))
            .collect();
        unused_test(source, expected_warnings);
    }

    #[test]
    fn unused_object() {
        let source = indoc! {r#"
            .o = { "key": 1 }
            { "array": [{"a": "b"}], "b": 2}
            "program result"
        "#};
        unused_test(
            source,
            vec![r#"unused object `{ "array": [{ "a": "b" }], "b": 2 }`"#.to_string()],
        );
    }

    #[test]
    fn unused_variables() {
        let source = indoc! {r#"
            a = "1"
            b = {
                c = "2"
                "3"
            }
            d = random_bool()
            . = d
        "#};

        let expected_warnings = ('a'..'d')
            .map(|ident| format!("unused variable `{ident}`"))
            .collect();
        unused_test(source, expected_warnings);
    }

    #[test]
    fn unused_function_result() {
        let source = indoc! {r#"
            .r = random_int(0,1)
            random_bool()
            "program result"
        "#};
        unused_test(
            source,
            vec!["unused result for function call `random_bool()`".to_string()],
        );
    }

    #[test]
    fn unused_ident_with_path() {
        let source = indoc! {"
            x = {}
            .f1 = x
            y = {}
            y.a = 1
        "};
        unused_test(source, vec!["unused variable `y`".to_string()]);
    }

    #[test]
    fn used_queries() {
        let source = indoc! {r#"
            _i_am_ignored = 42
            x = {}
            x.foo = 1
            x.bar = 2
            .bar = remove!(x, ["foo"]).bar

            y = {"foo": 3}.foo
        "#};
        unused_test(source, vec!["unused variable `y`".to_string()]);
    }

    #[test]
    fn used_in_if_condition() {
        let source = indoc! {r#"
            if starts_with!(.a, "foo") {
                .a = "foo"
            } else if starts_with!(.a, "bar") {
                .a = "bar"
            }

            x = 1
            .b = if (x < 1) { 0 } else { 1 }

            y = 2
            z = 3
            if (y < 2 && random_int(0, 4) < 3 ) { 0 } else { .c = z }

            x = {}
            x.a = 1
            .d = if (x.a < 1) { 0 } else { 1 }
        "#};
        unused_test(source, vec![]);
    }

    #[test]
    fn used_in_function_arguments() {
        let source = indoc! {"
            x = {}
            x.foo = 1
            .r = random_int!({x.foo}, x.foo + 1)

            x.bar = 2
            exists(field: x.bar)
            del(x.bar, compact: false)
        "};
        unused_test(
            source,
            vec!["unused result for function call `exists(field: xbar)`".to_string()],
        );
    }

    #[test]
    fn closure_shadows_unused_variable() {
        let source = indoc! {r#"
            count = 0;
            value = 42
            for_each({ "a": 1, "b": 2 }) -> |_key, value| { count = count + value };
            count
        "#};
        // Note that the `value` outside of the closure block is unused but not detected.
        unused_test(source, vec![]);
    }

    #[test]
    fn used_closure_result() {
        let source = indoc! {"
            patterns = [r'foo', r'bar']
            matched = false
            for_each(patterns) -> |_, pattern| {
              if !matched && match!(.message, pattern) {
                matched = true
              }
            }
            matched
        "};
        // Note that the `value` outside of the closure block is unused but not detected.
        unused_test(source, vec![]);
    }

    #[test]
    fn used_function_result_in_fallible_block() {
        let source = indoc! {r#"
            {
              parse_json("invalid")
              2
            } ?? 1
        "#};
        unused_test(source, vec![]);
    }

    #[test]
    fn unused_shadow_variable_not_detected() {
        // TODO: Support variable shadowing. A potential solution is to introduce the following type:
        // type IdentState = HashMap<usize, (bool, Span)>;
        let source = indoc! {"
            x = 1
            x = 2
            {
                x = {
                    x = {
                        x = 3
                        4
                    }
                    x
                }
                x
            }
        "};
        unused_test(source, vec![]);
    }

    #[test]
    fn undetected_merge_assignment() {
        // `x` is not used after the merging operation. This case is not detected.
        let source = indoc! {r#"
            x = {}
            x |= { "a" : 1}
            .
        "#};
        unused_test(source, vec![]);
    }
}
