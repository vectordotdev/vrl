use super::TimeZone;

use super::{Target, state::RuntimeState};

pub struct Context<'a> {
    target: &'a mut dyn Target,
    state: &'a mut RuntimeState,
    timezone: &'a TimeZone,
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

    /// Checks whether the program has been cancelled via a caller-supplied
    /// flag, panicking if so. Called on every expression resolution — it is
    /// used for cancellation only, hence the name.
    ///
    /// A no-op unless the `execution_cancellation` feature is enabled and a
    /// flag has been registered via
    /// [`RuntimeState::set_cancellation_flag`](super::state::RuntimeState::set_cancellation_flag).
    #[cfg(feature = "execution_cancellation")]
    #[inline]
    pub(crate) fn cancel_breakpoint(&mut self) {
        self.state.check_cancellation();
    }

    #[cfg(not(feature = "execution_cancellation"))]
    #[inline(always)]
    #[allow(clippy::unused_self)]
    pub(crate) fn cancel_breakpoint(&mut self) {}
}
