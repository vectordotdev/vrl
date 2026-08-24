#[cfg(feature = "execution_cancellation")]
use std::sync::Arc;
#[cfg(feature = "execution_cancellation")]
use std::sync::atomic::AtomicBool;
use std::{error::Error, fmt};

use crate::path::OwnedTargetPath;
use crate::value::Value;

use super::ExpressionError;
use super::TimeZone;
use super::{Context, Program, Target, state};

#[allow(clippy::module_name_repetitions)]
pub type RuntimeResult = Result<Value, Terminate>;

#[derive(Debug, Default)]
pub struct Runtime {
    state: state::RuntimeState,
}

/// The error raised if the runtime is terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminate {
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
            Terminate::Error(error) | Terminate::Abort(error) => error,
        }
    }
}

impl fmt::Display for Terminate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

        let mut ctx = Context::new(target, &mut self.state, timezone);

        match program.resolve(&mut ctx) {
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

#[cfg(feature = "execution_cancellation")]
impl Runtime {
    /// Registers a flag [`Runtime::resolve`] checks on every expression
    /// resolution. Set it to `true` from any thread to abort a running
    /// program — it panics with [`state::Cancelled`] rather than returning
    /// a [`RuntimeResult`].
    pub fn set_cancellation_flag(&mut self, flag: Arc<AtomicBool>) {
        self.state.set_cancellation_flag(flag);
    }
}

#[cfg(all(test, feature = "execution_cancellation", feature = "stdlib"))]
mod execution_cancellation_tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use super::{Runtime, TimeZone};
    use crate::compiler::state::{Cancelled, RuntimeState};
    use crate::value::Value;

    #[test]
    fn for_each_loop_panics_when_already_cancelled() {
        let source = r"
            count = 0
            for_each(array!(.items)) -> |_index, _value| {
                count = count + 1
            }
            count
        ";

        let program = crate::compiler::compile(source, &crate::stdlib::all())
            .expect("program should compile")
            .program;

        let mut target: Value =
            BTreeMap::from([("items".into(), Value::Array(vec![Value::from(1); 5_000]))]).into();

        let flag = Arc::new(AtomicBool::new(true));
        let mut runtime = Runtime::new(RuntimeState::default());
        runtime.set_cancellation_flag(flag);

        let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            runtime.resolve(&mut target, &program, &TimeZone::default())
        }))
        .expect_err("expected the for_each loop to be cancelled before it started");

        assert!(payload.downcast_ref::<Cancelled>().is_some());
    }
}
