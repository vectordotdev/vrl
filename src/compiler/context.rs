use std::ops::ControlFlow;

use crate::compiler::parser::ast::Ident;
use crate::value::Value;

use crate::compiler::{
    ExpressionError, Target, TimeZone, runtime::ExecutionControl, state::RuntimeState,
};

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

/// An RAII guard that manages up to two scoped variable bindings across loop iterations.
///
/// Pre-existing outer variables matching the bound names have their values saved
/// before iteration begins and restored when the guard drops (on normal completion,
/// `break`, `return`, or error). Slots are updated in place without re-allocating
/// or cloning identifiers. Newly introduced variables are removed on drop.
pub(crate) struct BindingScopeGuard<'a, 'c, 'b> {
    ctx: &'c mut Context<'b>,
    first_ident: Option<&'a Ident>,
    first_old_value: Option<Value>,
    second_ident: Option<&'a Ident>,
    second_old_value: Option<Value>,
}

impl<'a, 'c, 'b> BindingScopeGuard<'a, 'c, 'b> {
    #[must_use]
    pub(crate) fn new(
        ctx: &'c mut Context<'b>,
        first_ident: Option<&'a Ident>,
        second_ident: Option<&'a Ident>,
    ) -> Self {
        let first_ident = first_ident.filter(|ident| Some(*ident) != second_ident);
        let first_old_value = first_ident.and_then(|ident| ctx.state_mut().remove_variable(ident));
        let second_old_value =
            second_ident.and_then(|ident| ctx.state_mut().remove_variable(ident));

        Self {
            ctx,
            first_ident,
            first_old_value,
            second_ident,
            second_old_value,
        }
    }

    #[inline]
    pub(crate) fn ctx_mut(&mut self) -> &mut Context<'b> {
        self.ctx
    }

    #[inline]
    pub(crate) fn set_first(&mut self, value: Value) {
        if let Some(ident) = self.first_ident {
            self.ctx.state_mut().set_or_insert_variable(ident, value);
        }
    }

    #[inline]
    pub(crate) fn set_second(&mut self, value: Value) {
        if let Some(ident) = self.second_ident {
            self.ctx.state_mut().set_or_insert_variable(ident, value);
        }
    }

    #[inline]
    pub(crate) fn set_single(&mut self, value: Value) {
        self.set_first(value);
    }

    #[inline]
    pub(crate) fn set_key_value(&mut self, key: Value, value: Value) {
        self.set_first(key);
        self.set_second(value);
    }
}

impl Drop for BindingScopeGuard<'_, '_, '_> {
    fn drop(&mut self) {
        if let Some(ident) = self.first_ident {
            match self.first_old_value.take() {
                Some(val) => self.ctx.state_mut().set_or_insert_variable(ident, val),
                None => {
                    self.ctx.state_mut().remove_variable(ident);
                }
            }
        }
        if let Some(ident) = self.second_ident {
            match self.second_old_value.take() {
                Some(val) => self.ctx.state_mut().set_or_insert_variable(ident, val),
                None => {
                    self.ctx.state_mut().remove_variable(ident);
                }
            }
        }
    }
}
