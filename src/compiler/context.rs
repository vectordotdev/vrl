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

    /// Checks whether the program has exceeded a configured execution
    /// timeout, panicking if so. Called on every expression resolution.
    ///
    /// A no-op unless the `execution_timeout` feature is enabled and a
    /// timeout has been set via
    /// [`RuntimeState::set_timeout`](super::state::RuntimeState::set_timeout).
    #[cfg(feature = "execution_timeout")]
    #[inline]
    pub(crate) fn breakpoint(&mut self) {
        self.state.check_timeout();
    }

    #[cfg(not(feature = "execution_timeout"))]
    #[inline(always)]
    #[allow(clippy::unused_self)]
    pub(crate) fn breakpoint(&mut self) {}
}
