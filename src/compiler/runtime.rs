use std::{error::Error, fmt, ops::ControlFlow};

use crate::path::OwnedTargetPath;
use crate::value::Value;

use super::ExpressionError;
use super::TimeZone;
use super::{Context, Program, Target, state};

#[allow(clippy::module_name_repetitions)]
pub type RuntimeResult = Result<Value, Terminate>;

/// Allows an embedder to cooperatively interrupt VRL execution.
///
/// VRL invokes [`ExecutionControl::checkpoint`] between expressions. A
/// controller returns [`ControlFlow::Break`] to stop execution or
/// [`ControlFlow::Continue`] to allow it to proceed. The controller owns the
/// interruption policy, such as a deadline, operation budget, or shared
/// cancellation token.
pub trait ExecutionControl {
    fn checkpoint(&mut self) -> ControlFlow<()>;
}

impl<F> ExecutionControl for F
where
    F: FnMut() -> ControlFlow<()>,
{
    fn checkpoint(&mut self) -> ControlFlow<()> {
        self()
    }
}

/// Limits the number of cooperative execution checkpoints.
///
/// Hosts evaluating untrusted or user-authored VRL should use
/// `Runtime::resolve_with_limit` or provide their own `ExecutionControl`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepBudget {
    remaining: usize,
}

impl StepBudget {
    #[must_use]
    pub const fn new(limit: usize) -> Self {
        Self { remaining: limit }
    }

    #[must_use]
    pub const fn remaining(&self) -> usize {
        self.remaining
    }
}

impl ExecutionControl for StepBudget {
    fn checkpoint(&mut self) -> ControlFlow<()> {
        if self.remaining == 0 {
            ControlFlow::Break(())
        } else {
            self.remaining -= 1;
            ControlFlow::Continue(())
        }
    }
}

#[derive(Debug, Default)]
pub struct Runtime {
    state: state::RuntimeState,
}

/// The error raised if the runtime is terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminate {
    /// Execution was interrupted by an embedder-provided execution control.
    Interrupted,

    /// A manual `abort` call.
    ///
    /// This is an intentional termination that does not result in an
    /// `Ok(Value)` result, but should neither be interpreted as an unexpected
    /// outcome.
    Abort(ExpressionError),

    /// An unexpected program termination.
    Error(ExpressionError),
}

impl Terminate {
    #[must_use]
    pub fn get_expression_error(self) -> ExpressionError {
        match self {
            Terminate::Interrupted => ExpressionError::Interrupted,
            Terminate::Error(error) | Terminate::Abort(error) => error,
        }
    }
}

impl fmt::Display for Terminate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Terminate::Interrupted => f.write_str("execution interrupted"),
            Terminate::Error(error) | Terminate::Abort(error) => error.fmt(f),
        }
    }
}

impl Error for Terminate {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

#[allow(clippy::module_name_repetitions)]
pub type RuntimeError = Terminate;

impl Runtime {
    #[must_use]
    pub fn new(state: state::RuntimeState) -> Self {
        Self { state }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.state.is_empty()
    }

    pub fn clear(&mut self) {
        self.state.clear();
    }

    /// Resolves the provided [`Program`] to completion using the given [`Target`].
    ///
    /// This function ensures that the target contains a valid root object before proceeding.
    /// If the target is invalid or missing, an error is returned. The resolution process
    /// is performed using a [`Context`] that maintains execution state and timezone information.
    ///
    /// # Arguments
    ///
    /// * `target` - A mutable reference to an object implementing the [`Target`] trait. This
    ///   serves as the execution environment for resolving the program.
    /// * `program` - A reference to the [`Program`] that needs to be resolved.
    /// * `timezone` - A reference to the [`TimeZone`] used for resolving time-dependent expressions.
    ///
    /// # Returns
    ///
    /// Returns a [`RuntimeResult`], which is either:
    /// - `Ok(value)`: The program resolved successfully, producing a value.
    /// - `Err(Terminate::Error)`: A fatal error occurred during resolution.
    /// - `Err(Terminate::Abort)`: The resolution was aborted due to a non-fatal expression error.
    ///
    /// # Errors
    ///
    /// The function may return an error in the following cases:
    /// - If the target does not contain a valid root object, an error is returned.
    /// - If the resolution process encounters an [`ExpressionError::Error`].
    /// - If the program execution results in an [`ExpressionError::Abort`], [`ExpressionError::Fallible`], or [`ExpressionError::Missing`], the function aborts with `Terminate::Abort`.
    pub fn resolve(
        &mut self,
        target: &mut dyn Target,
        program: &Program,
        timezone: &TimeZone,
    ) -> RuntimeResult {
        self.resolve_inner(target, program, *timezone, None)
    }

    /// Resolves the provided [`Program`] with an embedder-provided execution
    /// control.
    ///
    /// The control is scoped to this invocation and is not stored in the
    /// [`Runtime`] or its [`state::RuntimeState`]. Returning
    /// [`ControlFlow::Break`] from [`ExecutionControl::checkpoint`] terminates
    /// the invocation with [`Terminate::Interrupted`].
    ///
    /// This is cooperative interruption: VRL checks between expressions and
    /// wherever a function explicitly calls [`Context::checkpoint`]. It does
    /// not preempt a single blocking or long-running function call.
    ///
    /// # Errors
    ///
    /// Returns [`Terminate::Interrupted`] when the control requests
    /// interruption. Other termination conditions are the same as
    /// [`Runtime::resolve`].
    pub fn resolve_with_control(
        &mut self,
        target: &mut dyn Target,
        program: &Program,
        timezone: &TimeZone,
        control: &mut dyn ExecutionControl,
    ) -> RuntimeResult {
        self.resolve_inner(target, program, *timezone, Some(control))
    }

    /// Resolves a program with a hard cooperative execution step budget.
    ///
    /// When the number of checkpoints exceeds `limit`, execution halts
    /// cooperatively and returns `Err(Terminate::Interrupted)`.
    ///
    /// # Errors
    ///
    /// Returns [`Terminate::Interrupted`] when the step budget is exhausted.
    /// Other termination conditions are the same as [`Runtime::resolve`].
    pub fn resolve_with_limit(
        &mut self,
        target: &mut dyn Target,
        program: &Program,
        timezone: &TimeZone,
        limit: usize,
    ) -> RuntimeResult {
        let mut budget = StepBudget::new(limit);
        self.resolve_with_control(target, program, timezone, &mut budget)
    }

    fn resolve_inner(
        &mut self,
        target: &mut dyn Target,
        program: &Program,
        timezone: TimeZone,
        control: Option<&mut dyn ExecutionControl>,
    ) -> RuntimeResult {
        // Validate that the path is a value.
        match target.target_get(&OwnedTargetPath::event_root()) {
            Ok(Some(_)) => {}
            Ok(None) => {
                return Err(Terminate::Error(
                    "expected target object, got nothing".to_owned().into(),
                ));
            }
            Err(err) => {
                return Err(Terminate::Error(
                    format!("error querying target object: {err}").into(),
                ));
            }
        }

        let mut ctx = match control {
            Some(control) => Context::new_with_control(target, &mut self.state, &timezone, control),
            None => Context::new(target, &mut self.state, &timezone),
        };

        match program.resolve(&mut ctx) {
            Err(ExpressionError::Interrupted) => Err(Terminate::Interrupted),
            Ok(value) | Err(ExpressionError::Return { value, .. }) => Ok(value),
            Err(
                err @ (ExpressionError::Abort { .. }
                | ExpressionError::Fallible { .. }
                | ExpressionError::Missing { .. }),
            ) => Err(Terminate::Abort(err)),
            Err(
                err @ (ExpressionError::Error { .. }
                | ExpressionError::Break { .. }
                | ExpressionError::Continue { .. }),
            ) => Err(Terminate::Error(err)),
        }
    }
}

#[cfg(all(test, feature = "stdlib"))]
mod execution_control_tests {
    use std::collections::BTreeMap;
    use std::ops::ControlFlow;

    use super::{ExecutionControl, Runtime, Terminate, TimeZone};
    use crate::compiler::Program;
    use crate::compiler::state::RuntimeState;
    use crate::parser::ast::Ident;
    use crate::value::Value;

    struct BreakAt {
        checkpoint: usize,
        break_at: usize,
    }

    impl BreakAt {
        fn new(break_at: usize) -> Self {
            Self {
                checkpoint: 0,
                break_at,
            }
        }
    }

    impl ExecutionControl for BreakAt {
        fn checkpoint(&mut self) -> ControlFlow<()> {
            self.checkpoint += 1;

            if self.checkpoint >= self.break_at {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        }
    }

    fn compile(source: &str) -> Program {
        crate::compiler::compile(source, &crate::stdlib::all())
            .expect("program should compile")
            .program
    }

    fn target() -> Value {
        BTreeMap::from([
            ("items".into(), Value::Array(vec![Value::from(1); 100])),
            ("value".into(), Value::from("1")),
        ])
        .into()
    }

    #[test]
    fn resolve_without_control_preserves_existing_api() {
        let program = compile("1 + 2");
        let mut target = target();
        let mut runtime = Runtime::new(RuntimeState::default());

        assert_eq!(
            runtime.resolve(&mut target, &program, &TimeZone::default()),
            Ok(Value::from(3)),
        );
    }

    #[test]
    fn for_each_loop_returns_interrupted() {
        let source = r"
            count = 0
            for_each(array!(.items)) -> |_index, _value| {
                count = count + 1
            }
            count
        ";

        let program = compile(source);

        let mut target: Value =
            BTreeMap::from([("items".into(), Value::Array(vec![Value::from(1); 5_000]))]).into();

        let mut runtime = Runtime::new(RuntimeState::default());
        let mut control = || ControlFlow::Break(());

        assert_eq!(
            runtime
                .resolve_with_control(&mut target, &program, &TimeZone::default(), &mut control,),
            Err(Terminate::Interrupted),
        );
    }

    #[test]
    fn interruption_is_not_caught_by_error_coalescing() {
        let program = compile("to_int(.value) ?? 2");
        let mut target = target();
        let mut runtime = Runtime::new(RuntimeState::default());
        let mut control = BreakAt::new(2);

        assert_eq!(
            runtime
                .resolve_with_control(&mut target, &program, &TimeZone::default(), &mut control,),
            Err(Terminate::Interrupted),
        );
    }

    #[test]
    fn interruption_is_not_caught_by_infallible_assignment() {
        let program = compile("value, error = to_int(.value)\nvalue");
        let mut target = target();
        let mut runtime = Runtime::new(RuntimeState::default());
        let mut control = BreakAt::new(2);

        assert_eq!(
            runtime
                .resolve_with_control(&mut target, &program, &TimeZone::default(), &mut control,),
            Err(Terminate::Interrupted),
        );
    }

    #[test]
    fn interruption_is_not_wrapped_by_boolean_or() {
        let program = compile("false || true");
        let mut target = target();
        let mut runtime = Runtime::new(RuntimeState::default());
        let mut control = BreakAt::new(3);

        assert_eq!(
            runtime
                .resolve_with_control(&mut target, &program, &TimeZone::default(), &mut control,),
            Err(Terminate::Interrupted),
        );
    }

    #[test]
    fn control_is_scoped_to_one_resolve_call() {
        let program = compile("1");
        let mut target = target();
        let mut runtime = Runtime::new(RuntimeState::default());
        let mut control = || ControlFlow::Break(());

        assert_eq!(
            runtime
                .resolve_with_control(&mut target, &program, &TimeZone::default(), &mut control,),
            Err(Terminate::Interrupted),
        );
        assert_eq!(
            runtime.resolve(&mut target, &program, &TimeZone::default()),
            Ok(Value::from(1)),
        );
    }

    #[test]
    fn interruption_restores_closure_variables() {
        let source = r#"
            item = "outer"
            for_each(array!(.items)) -> |_index, item| {
                item
            }
            item
        "#;
        let program = compile(source);
        let mut target = target();
        let mut runtime = Runtime::new(RuntimeState::default());
        let mut control = BreakAt::new(10);

        assert_eq!(
            runtime
                .resolve_with_control(&mut target, &program, &TimeZone::default(), &mut control,),
            Err(Terminate::Interrupted),
        );
        assert_eq!(
            runtime.state.variable(&Ident::new("item")),
            Some(&Value::from("outer")),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::compiler::Program;
    use crate::compiler::TargetValue;
    use crate::compiler::TypeDef;
    use crate::compiler::expression::{self, Block, Expr, Return, Variable};
    use crate::compiler::parser::ast::{ForPattern, Ident, Node};
    use crate::compiler::program::ProgramInfo;
    use crate::compiler::state::{LocalEnv, RuntimeState, TypeState};
    use crate::compiler::type_def::Details;
    use crate::diagnostic::Span;
    use crate::value::{ObjectMap, Secrets, Value};
    use indoc::indoc;

    fn make_program(expressions: Vec<Expr>) -> Program {
        Program {
            initial_state: TypeState::default(),
            expressions: Block::new_inline(expressions),
            info: ProgramInfo {
                fallible: false,
                abortable: false,
                target_queries: vec![],
                target_assignments: vec![],
            },
        }
    }

    fn test_variable_expr(ident: &str) -> Expr {
        let mut local = LocalEnv::default();
        local.insert_variable(
            Ident::new(ident),
            Details {
                type_def: TypeDef::any(),
                value: None,
            },
        );
        Expr::Variable(Variable::new(Span::new(0, 0), Ident::new(ident), &local).unwrap())
    }

    #[test]
    fn test_runtime_for_loop_array() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let iterable = Box::new(Expr::from(Value::Array(vec![
            Value::from(1),
            Value::from(2),
            Value::from(3),
        ])));
        let body_expr = test_variable_expr("x");
        let block = Block::new_scoped(vec![body_expr]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert_eq!(res, Ok(Value::Null));

        // Loop variable x should have been dropped
        assert!(runtime.state.variable(&Ident::new("x")).is_none());
    }

    #[test]
    fn test_runtime_for_loop_break() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let iterable = Box::new(Expr::from(Value::Array(vec![
            Value::from(1),
            Value::from(2),
            Value::from(3),
            Value::from(4),
        ])));
        let block = Block::new_scoped(vec![Expr::Break(expression::Break::new(span))]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert_eq!(res, Ok(Value::Null));
    }

    #[test]
    fn test_runtime_for_loop_continue() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let iterable = Box::new(Expr::from(Value::Array(vec![
            Value::from(1),
            Value::from(2),
            Value::from(3),
        ])));
        let block = Block::new_scoped(vec![Expr::Continue(expression::Continue::new(span))]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert_eq!(res, Ok(Value::Null));
    }

    #[test]
    fn test_runtime_for_loop_return() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let iterable = Box::new(Expr::from(Value::Array(vec![
            Value::from(1),
            Value::from(2),
            Value::from(3),
        ])));
        let ret_expr = Expr::Return(
            Return::new(
                span,
                Node::new(span, Expr::from(Value::from(42))),
                &TypeState::default(),
            )
            .unwrap(),
        );
        let block = Block::new_scoped(vec![ret_expr]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert_eq!(res, Ok(Value::from(42)));
    }

    #[test]
    fn test_runtime_for_loop_shadowing_outer_preserved() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let iterable = Box::new(Expr::from(Value::Array(vec![
            Value::from(1),
            Value::from(2),
        ])));
        let block = Block::new_scoped(vec![test_variable_expr("x")]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        runtime
            .state
            .insert_variable(Ident::new("x"), Value::from("outer_x"));

        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert_eq!(res, Ok(Value::Null));
        assert_eq!(
            runtime.state.variable(&Ident::new("x")),
            Some(&Value::from("outer_x"))
        );
    }

    #[test]
    fn test_runtime_for_loop_shadowing_on_break() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let iterable = Box::new(Expr::from(Value::Array(vec![
            Value::from(1),
            Value::from(2),
        ])));
        let block = Block::new_scoped(vec![Expr::Break(expression::Break::new(span))]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        runtime
            .state
            .insert_variable(Ident::new("x"), Value::from("outer_x"));

        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert_eq!(res, Ok(Value::Null));
        assert_eq!(
            runtime.state.variable(&Ident::new("x")),
            Some(&Value::from("outer_x"))
        );
    }

    #[test]
    fn test_runtime_for_loop_shadowing_on_return() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let iterable = Box::new(Expr::from(Value::Array(vec![
            Value::from(1),
            Value::from(2),
        ])));
        let ret_expr = Expr::Return(
            Return::new(
                span,
                Node::new(span, Expr::from(Value::from("early"))),
                &TypeState::default(),
            )
            .unwrap(),
        );
        let block = Block::new_scoped(vec![ret_expr]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        runtime
            .state
            .insert_variable(Ident::new("x"), Value::from("outer_x"));

        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert_eq!(res, Ok(Value::from("early")));
        assert_eq!(
            runtime.state.variable(&Ident::new("x")),
            Some(&Value::from("outer_x"))
        );
    }

    #[test]
    fn test_runtime_for_loop_array_key_value() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::KeyValue(
            Node::new(span, Ident::new("i")),
            Node::new(span, Ident::new("v")),
        );
        let iterable = Box::new(Expr::from(Value::Array(vec![
            Value::from("first"),
            Value::from("second"),
        ])));
        let block = Block::new_scoped(vec![test_variable_expr("v")]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert_eq!(res, Ok(Value::Null));
        assert!(runtime.state.variable(&Ident::new("i")).is_none());
        assert!(runtime.state.variable(&Ident::new("v")).is_none());
    }

    #[test]
    fn test_runtime_for_loop_object_key_value() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::KeyValue(
            Node::new(span, Ident::new("k")),
            Node::new(span, Ident::new("v")),
        );
        let mut map = ObjectMap::new();
        map.insert("a".into(), Value::Integer(10));
        map.insert("b".into(), Value::Integer(20));
        let iterable = Box::new(Expr::from(Value::Object(map)));
        let block = Block::new_scoped(vec![test_variable_expr("v")]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert_eq!(res, Ok(Value::Null));
        assert!(runtime.state.variable(&Ident::new("k")).is_none());
        assert!(runtime.state.variable(&Ident::new("v")).is_none());
    }

    #[test]
    fn test_runtime_for_loop_non_collection_error() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let iterable = Box::new(Expr::from(Value::Integer(42)));
        let block = Block::new_scoped(vec![test_variable_expr("x")]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert!(matches!(res, Err(Terminate::Error(_))));
    }

    #[test]
    fn test_runtime_for_loop_single_var_object_error() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let mut map = ObjectMap::new();
        map.insert("a".into(), Value::Integer(10));
        let iterable = Box::new(Expr::from(Value::Object(map)));
        let block = Block::new_scoped(vec![test_variable_expr("x")]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert!(matches!(res, Err(Terminate::Error(_))));
    }

    #[test]
    fn test_runtime_for_loop_empty_collections() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::Single(Node::new(span, Ident::new("x")));
        let iterable = Box::new(Expr::from(Value::Array(vec![])));
        let block = Block::new_scoped(vec![Expr::Break(expression::Break::new(span))]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert_eq!(res, Ok(Value::Null));
    }

    #[test]
    fn test_runtime_for_loop_wildcard() {
        let span = Span::new(0, 0);
        let pattern = ForPattern::KeyValue(
            Node::new(span, Ident::new("_")),
            Node::new(span, Ident::new("v")),
        );
        let iterable = Box::new(Expr::from(Value::Array(vec![Value::from(10)])));
        let block = Block::new_scoped(vec![test_variable_expr("v")]);
        let for_expr = Expr::For(expression::For::new(span, pattern, iterable, block));

        let program = make_program(vec![for_expr]);
        let mut target = Value::Object(BTreeMap::new());
        let mut runtime = Runtime::new(RuntimeState::default());
        let res = runtime.resolve(&mut target, &program, &TimeZone::default());
        assert_eq!(res, Ok(Value::Null));
        assert!(runtime.state.variable(&Ident::new("_")).is_none());
        assert!(runtime.state.variable(&Ident::new("v")).is_none());
    }

    #[test]
    fn break_is_not_caught_by_infallible_assignment_in_for_loop() {
        let source = indoc! {r#"
            count = 0
            for val in [1, 2, 3] {
                count = count + 1
                _, err = if val == 2 {
                    break
                } else {
                    parse_int("not_a_number")
                }
            }
            count
        "#};

        let fns = crate::stdlib::all();
        let program = crate::compiler::compile(source, &fns)
            .expect("program compiles")
            .program;

        let mut target = TargetValue {
            value: Value::Null,
            metadata: Value::Null,
            secrets: Secrets::new(),
        };
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let result = program.resolve(&mut ctx);
        assert_eq!(result, Ok(Value::from(2)));
    }

    #[test]
    fn continue_is_not_caught_by_infallible_assignment_in_for_loop() {
        let source = indoc! {r#"
            sum = 0
            for val in [1, 2, 3] {
                _, err = if val == 2 {
                    continue
                } else {
                    parse_int("not_a_number")
                }
                sum = sum + val
            }
            sum
        "#};

        let fns = crate::stdlib::all();
        let program = crate::compiler::compile(source, &fns)
            .expect("program compiles")
            .program;

        let mut target = TargetValue {
            value: Value::Null,
            metadata: Value::Null,
            secrets: Secrets::new(),
        };
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let result = program.resolve(&mut ctx);
        assert_eq!(result, Ok(Value::from(4))); // 1 + 3 (2 skipped)
    }

    #[test]
    fn break_is_not_caught_by_error_coalescing_in_for_loop() {
        let source = indoc! {r#"
            count = 0
            for val in [1, 2, 3] {
                count = count + 1
                res = { if val == 2 { break } else { parse_int("not_a_number") } } ?? 999
            }
            count
        "#};

        let fns = crate::stdlib::all();
        let program = crate::compiler::compile(source, &fns)
            .expect("program compiles")
            .program;

        let mut target = TargetValue {
            value: Value::Null,
            metadata: Value::Null,
            secrets: Secrets::new(),
        };
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let result = program.resolve(&mut ctx);
        assert_eq!(result, Ok(Value::from(2)));
    }

    #[test]
    fn continue_is_not_caught_by_error_coalescing_in_for_loop() {
        let source = indoc! {r#"
            sum = 0
            for val in [1, 2, 3] {
                res = { if val == 2 { continue } else { parse_int("not_a_number") } } ?? 999
                sum = sum + val
            }
            sum
        "#};

        let fns = crate::stdlib::all();
        let program = crate::compiler::compile(source, &fns)
            .expect("program compiles")
            .program;

        let mut target = TargetValue {
            value: Value::Null,
            metadata: Value::Null,
            secrets: Secrets::new(),
        };
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let result = program.resolve(&mut ctx);
        assert_eq!(result, Ok(Value::from(4))); // 1 + 3 (2 skipped)
    }

    #[test]
    fn break_is_not_wrapped_by_boolean_or_in_for_loop() {
        let source = indoc! {r"
            count = 0
            for val in [1, 2, 3] {
                count = count + 1
                if false || { if val == 2 { break } else { true } } {
                    res = val
                }
            }
            count
        "};

        let fns = crate::stdlib::all();
        let program = crate::compiler::compile(source, &fns)
            .expect("program compiles")
            .program;

        let mut target = TargetValue {
            value: Value::Null,
            metadata: Value::Null,
            secrets: Secrets::new(),
        };
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let result = program.resolve(&mut ctx);
        assert_eq!(result, Ok(Value::from(2)));
    }

    #[test]
    fn continue_is_not_wrapped_by_boolean_or_in_for_loop() {
        let source = indoc! {r"
            sum = 0
            for val in [1, 2, 3] {
                if false || { if val == 2 { continue } else { true } } {
                    sum = sum + val
                }
            }
            sum
        "};

        let fns = crate::stdlib::all();
        let program = crate::compiler::compile(source, &fns)
            .expect("program compiles")
            .program;

        let mut target = TargetValue {
            value: Value::Null,
            metadata: Value::Null,
            secrets: Secrets::new(),
        };
        let mut state = RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let result = program.resolve(&mut ctx);
        assert_eq!(result, Ok(Value::from(4))); // 1 + 3 (2 skipped)
    }

    fn compile_test_program(source: &str) -> Program {
        // VRL grammar requires blocks to contain at least one expression.
        // Normalize empty or comment-only blocks to include `null`.
        let normalized = source.replace("{ }", "{ null }").replace(
            "{\n  # interrupts here\n}",
            "{\n  # interrupts here\n  null\n}",
        );
        let external = crate::compiler::state::ExternalEnv::new_with_kind(
            crate::value::Kind::array(crate::compiler::value::Collection::any()),
            crate::value::Kind::object(crate::compiler::value::Collection::any()),
        );
        let fns = crate::stdlib::all();
        crate::compiler::compile_with_external(
            &normalized,
            &fns,
            &external,
            crate::compiler::CompileConfig::default(),
        )
        .expect("test program compiles")
        .program
    }

    #[test]
    fn resolve_with_limit_zero_budget_interrupts_immediately() {
        let mut runtime = Runtime::default();
        let mut target = TargetValue::new(Value::from(vec![1, 2, 3]));
        let program = compile_test_program("for x in . { }");
        let res = runtime.resolve_with_limit(&mut target, &program, &TimeZone::default(), 0);
        assert!(matches!(res, Err(RuntimeError::Interrupted)));
    }

    #[test]
    fn resolve_with_limit_exhausts_during_direct_loop() {
        let mut runtime = Runtime::default();
        let mut target =
            TargetValue::new(Value::from((0..100).map(Value::from).collect::<Vec<_>>()));
        let program = compile_test_program("count = 0\nfor x in . { count = count + 1 }");
        // Limit to 10 steps on a 100-element array
        let res = runtime.resolve_with_limit(&mut target, &program, &TimeZone::default(), 10);
        assert!(matches!(res, Err(RuntimeError::Interrupted)));
    }

    #[test]
    fn resolve_with_limit_exhausts_in_nested_loops() {
        let mut runtime = Runtime::default();
        let mut target =
            TargetValue::new(Value::from((0..10).map(Value::from).collect::<Vec<_>>()));
        let program = compile_test_program(
            "count = 0\nfor x in . {\n  for y in . {\n    count = count + 1\n  }\n}",
        );
        // 10x10 = 100 iterations. Budget of 15 should interrupt during the second outer iteration.
        let res = runtime.resolve_with_limit(&mut target, &program, &TimeZone::default(), 15);
        assert!(matches!(res, Err(RuntimeError::Interrupted)));
    }

    #[test]
    fn resolve_with_limit_restores_shadowed_variable_on_interruption() {
        let mut runtime = Runtime::default();
        let mut target = TargetValue::new(Value::from(vec![10, 20, 30]));
        let program = compile_test_program("x = 999\nfor x in . {\n  # interrupts here\n}");
        let res = runtime.resolve_with_limit(&mut target, &program, &TimeZone::default(), 2);
        assert!(matches!(res, Err(RuntimeError::Interrupted)));
        // Verify outer x is restored in runtime state
        let ident = crate::parser::ast::Ident::from("x");
        assert_eq!(runtime.state.variable(&ident), Some(&Value::from(999)));
    }

    #[test]
    fn step_budget_counts_down_and_breaks_at_zero() {
        let mut budget = StepBudget::new(2);
        assert_eq!(budget.remaining(), 2);
        assert_eq!(budget.checkpoint(), ControlFlow::Continue(()));
        assert_eq!(budget.remaining(), 1);
        assert_eq!(budget.checkpoint(), ControlFlow::Continue(()));
        assert_eq!(budget.remaining(), 0);
        assert_eq!(budget.checkpoint(), ControlFlow::Break(()));
        assert_eq!(budget.remaining(), 0);
        assert_eq!(budget.checkpoint(), ControlFlow::Break(()));

        let copy = budget;
        assert_eq!(budget, copy);
        assert_eq!(format!("{budget:?}"), "StepBudget { remaining: 0 }");
    }

    #[test]
    fn resolve_with_limit_sufficient_budget_succeeds() {
        let mut runtime = Runtime::default();
        let mut target = TargetValue::new(Value::from(vec![1, 2, 3]));
        let program = compile_test_program("count = 0\nfor x in . { count = count + 1 }\ncount");
        let res = runtime.resolve_with_limit(&mut target, &program, &TimeZone::default(), 100);
        assert_eq!(res, Ok(Value::from(3)));
    }
}
