use std::{error::Error, fmt};

use crate::path::OwnedTargetPath;
use crate::value::Value;

use super::ExpressionError;
use super::TimeZone;
use super::{state, Context, Program, Target};

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
                ))
            }
            Err(err) => {
                return Err(Terminate::Error(
                    format!("error querying target object: {err}").into(),
                ))
            }
        };

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
