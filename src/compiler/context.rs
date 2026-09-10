use std::ops::ControlFlow;

use super::{ExpressionError, Target, TimeZone, runtime::ExecutionControl, state::RuntimeState};

pub struct Context<'a> {
    target: &'a mut dyn Target,
    state: &'a mut RuntimeState,
    timezone: &'a TimeZone,
    execution_control: Option<&'a mut dyn ExecutionControl>,
}

impl<'a> Context<'a> {
    /// Create a new [`Context`].
    pub fn new(
        target: &'a mut dyn Target,
        state: &'a mut RuntimeState,
        timezone: &'a TimeZone,
    ) -> Self {
        Self {
            target,
            state,
            timezone,
            execution_control: None,
        }
    }

    pub(crate) fn new_with_control(
        target: &'a mut dyn Target,
        state: &'a mut RuntimeState,
        timezone: &'a TimeZone,
        execution_control: &'a mut dyn ExecutionControl,
    ) -> Self {
        Self {
            target,
            state,
            timezone,
            execution_control: Some(execution_control),
        }
    }

    /// Get a reference to the [`Target`].
    #[must_use]
    pub fn target(&self) -> &dyn Target {
        self.target
    }

    /// Get a mutable reference to the [`Target`].
    pub fn target_mut(&mut self) -> &mut dyn Target {
        self.target
    }

    /// Get a reference to the [`runtime state`](RuntimeState).
    #[must_use]
    pub fn state(&self) -> &RuntimeState {
        self.state
    }

    /// Get a mutable reference to the [`runtime state`](RuntimeState).
    pub fn state_mut(&mut self) -> &mut RuntimeState {
        self.state
    }

    /// Get a reference to the [`TimeZone`]
    #[must_use]
    pub fn timezone(&self) -> &TimeZone {
        self.timezone
    }

    /// Checks whether the embedder has requested that execution stop.
    ///
    /// VRL calls this between expressions. Functions that perform long-running
    /// work can call it at additional safe points.
    ///
    /// # Errors
    ///
    /// Returns [`ExpressionError::Interrupted`] when the configured
    /// [`ExecutionControl`] requests interruption. With no execution control,
    /// this always succeeds.
    #[inline]
    pub fn checkpoint(&mut self) -> Result<(), ExpressionError> {
        match self.execution_control.as_deref_mut() {
            Some(control) => match control.checkpoint() {
                ControlFlow::Break(()) => Err(ExpressionError::Interrupted),
                ControlFlow::Continue(()) => Ok(()),
            },
            None => Ok(()),
        }
    }
}
