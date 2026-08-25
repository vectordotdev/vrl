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
            Err(err @ ExpressionError::Error { .. }) => Err(Terminate::Error(err)),
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
