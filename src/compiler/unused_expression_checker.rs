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
/// - **Closures**: Closure support is minimal. For example, shadowed variables are not detected.
// #[allow(clippy::print_stdout)]

use crate::compiler::codes::WARNING_UNUSED_CODE;
use crate::compiler::parser::{Ident, Node};
use crate::diagnostic::{Diagnostic, DiagnosticList, Label, Note, Severity};
use crate::parser::ast::{
    Array, Assignment, AssignmentTarget, Block, Container, Expr, FunctionCall, Object, Predicate,
    QueryTarget, RootExpr, Unary,
};
use crate::parser::{Program, Span};
use std::collections::{BTreeMap, HashMap};

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
    ident_pending_usage: BTreeMap<Ident, IdentState>,
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

    fn mark_query_target_as_pending(&mut self, query_target: &Node<QueryTarget>) {
        match &query_target.node {
            QueryTarget::Internal(ident) => {
                self.ident_pending_usage
                    .entry(ident.clone())
                    .and_modify(|state| {
                        state.pending_usage = true;
                    })
                    .or_insert(IdentState {
                        span: query_target.span,
                        pending_usage: true,
                    });
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
        for (ident, state) in self.ident_pending_usage.clone() {
            if state.pending_usage && !ident.starts_with('_') {
                self.append_diagnostic(format!("unused variable `{ident}`"), &state.span);
            }
        }
    }
}

impl AstVisitor<'_> {
    fn visit_node(&self, node: &Node<Expr>, state: &mut VisitorState) {
        let expression = node.inner();
        // println!("\n{} visit_node: {expression:#?}", state.level);
        // println!("pending_assignment {:#?}", state.expecting_result);
        // println!("ident_pending_usage {:#?}", state.ident_pending_usage);

        match expression {
            Expr::Literal(literal) => {
                if state.is_unused() {
                    state.append_diagnostic(format!("unused literal `{literal}`"), &node.span());
                }
            }
            Expr::Container(container) => {
                self.visit_container(container, state);
            }
            Expr::IfStatement(if_statement) => {
                match &if_statement.predicate.node {
                    Predicate::One(expr) => self.visit_node(expr, state),
                    Predicate::Many(exprs) => {
                        for expr in exprs {
                            self.visit_node(expr, state);
                        }
                    }
                }
                self.visit_block(&if_statement.if_node, state);
                if let Some(else_block) = &if_statement.else_node {
                    self.visit_block(else_block, state);
                }
            }
            Expr::Op(op) => {
                self.visit_node(&op.0, state);
                self.visit_node(&op.2, state);
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
                        state
                            .ident_pending_usage
                            .entry(ident.clone())
                            .and_modify(|state| state.pending_usage = false)
                            .or_insert({
                                IdentState {
                                    span: query.node.target.span,
                                    pending_usage: false,
                                }
                            });
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
                let key = variable.node.clone();
                state
                    .ident_pending_usage
                    .entry(key)
                    .and_modify(|state| state.pending_usage = false)
                    .or_insert(IdentState {
                        span: variable.span,
                        pending_usage: false,
                    });
            }
            Expr::Abort(_) => {}
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
        state.level += 1;
        let exprs = &block.node.0;
        for (i, expr) in exprs.iter().enumerate() {
            if i == exprs.len() - 1 {
                state.level -= 1;
            }
            self.visit_node(expr, state);
        }
    }

    fn visit_object(&self, object: &Node<Object>, state: &mut VisitorState) {
        if state.is_unused() {
            state.append_diagnostic(format!("unused object `{object}`"), &object.span);
        }
        for value in object.0.values() {
            state.level += 1;
            state.expecting_result.insert(state.level, true);
            self.visit_node(value, state);
            state.expecting_result.insert(state.level, false);
            state.level -= 1;
        }
    }

    fn visit_assignment(&self, assignment: &Node<Assignment>, state: &mut VisitorState) {
        state.level += 1;
        let level = state.level;
        state.expecting_result.insert(level, true);

        // All targets needs to be used later.
        let targets = match &assignment.node {
            Assignment::Single { target, .. } => vec![target],
            Assignment::Infallible { ok, err, .. } => vec![ok, err],
        };
        for target in targets {
            match &target.node {
                AssignmentTarget::Noop => {}
                AssignmentTarget::Query(query) => {
                    state.mark_query_target_as_pending(&query.target);
                }
                AssignmentTarget::Internal(ident, path) => {
                    if path.is_none() {
                        state
                            .ident_pending_usage
                            .entry(ident.clone())
                            .or_insert(IdentState {
                                span: target.span,
                                pending_usage: true,
                            });
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
        state.level -= 1;
    }

    fn visit_function_call(
        &self,
        function_call: &FunctionCall,
        span: &Span,
        state: &mut VisitorState,
    ) {
        for argument in &function_call.arguments {
            state.level += 1;
            state.expecting_result.insert(state.level, true);
            self.visit_node(&argument.node.expr, state);
            state.expecting_result.insert(state.level, true);
            state.level -= 1;
        }
        match function_call.ident.0.as_str() {
            //  All bets are off for functions with side-effects.
            "del" | "log" => (),
            _ => {
                if let Some(closure) = &function_call.closure {
                    for variable in &closure.variables {
                        state.ident_pending_usage.entry(variable.node.clone()).and_modify(|state| state.pending_usage = true)
                            .or_insert(IdentState {
                                span: *span,
                                pending_usage: true,
                            });
                    }
                    self.visit_block(&closure.block, state);
                } else if state.is_unused() {
                    state.append_diagnostic(
                        format!("unused result for function call `{function_call}`"),
                        span,
                    );
                }
            }
        }
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
                state.level += 1;
                state.expecting_result.insert(state.level, true);
            }
            match root_node.inner() {
                RootExpr::Expr(node) => self.visit_node(node, &mut state),
                RootExpr::Error(_) => {}
            }
            if is_last {
                state.level -= 1;
                state.expecting_result.insert(state.level, true);
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
    // use crate::diagnostic::Formatter;

    fn unused_test(source: &str, expected_warnings: Vec<String>) {
        let warnings = crate::compiler::compile(source, &stdlib::all())
            .unwrap()
            .warnings;
        // println!("{}", Formatter::new(source, warnings.clone()).colored());
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

                . = {{{ 42 }}}

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
            vec![r#"unused result for function call `random_bool()`"#.to_string()],
        );
    }

    #[test]
    fn unused_ident_with_path() {
        let source = indoc! {r#"
            x = {}
            .f1 = x
            y = {}
            y.a = 1
        "#};
        unused_test(source, vec!["unused variable `y`".to_string()]);
    }

    #[test]
    fn unused_coalesce_branches() {
        let source = indoc! {r#"
           parse_syslog("not syslog") ?? parse_common_log("not common") ?? "malformed"
           .res = parse_syslog("not syslog") ?? parse_common_log("not common") ?? "malformed"
        "#};
        unused_test(
            source,
            vec![
                r#"unused result for function call `parse_syslog("not syslog")`"#.to_string(),
                r#"unused result for function call `parse_common_log("not common")`"#.to_string(),
                r#"unused literal `"malformed"`"#.to_string(),
            ],
        );
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
        unused_test(source, vec![r#"unused variable `y`"#.to_string()]);
    }

    #[test]
    fn used_in_if_condition() {
        let source = indoc! {r#"
            x = 1
            .a = if (x < 1) { 0 } else { 1 }

            y = 2
            z = 3
            .b = if (y < z) { 0 } else { 1 }

            x = {}
            x.a = 1
            .c = if (x.a < 1) { 0 } else { 1 }
        "#};
        unused_test(source, vec![]);
    }

    #[test]
    fn used_in_function_arguments() {
        let source = indoc! {r#"
            x = {}
            x.foo = 1
            .r = random_int!({x.foo}, x.foo + 1)

            x.bar = 2
            exists(field: x.bar)
            del(x.bar, compact: false)
        "#};
        unused_test(
            source,
            vec![r#"unused result for function call `exists(field: xbar)`"#.to_string()],
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
        unused_test(
            source,
            vec![],
        );
    }
}
