use std::collections::BTreeMap;

use crate::compiler::{
    Context, ControlSignal, ExpressionError,
    state::RuntimeState,
    value::{Kind, VrlValueConvert},
};
use crate::parser::ast::Ident;
use crate::value::{
    KeyString, Value,
    kind::{Collection, Field, Index},
};

use super::Example;

/// The definition of a function-closure block a function expects to
/// receive.
#[derive(Debug)]
pub struct Definition {
    /// A list of input configurations valid for this closure definition.
    pub inputs: Vec<Input>,

    /// Defines whether the closure is expected to iterate over the elements of
    /// a collection.
    ///
    /// If this is `true`, the compiler will (1) reject any non-iterable types
    /// passed to this closure, and (2) use the type definition of inner
    /// collection elements to determine the eventual type definition of the
    /// closure variable(s) (see `Variable`).
    pub is_iterator: bool,

    /// Defines whether the closure supports the `break` statement for early
    /// loop exit. Only closures whose implementations consume
    /// `ControlSignal::Break` (e.g. `for_each`) should set this to `true`.
    pub supports_break: bool,
}

/// One input variant for a function-closure.
///
/// A closure can support different variable input shapes, depending on the
/// type of a given parameter of the function.
///
/// For example, the `for_each` function takes either an `Object` or an `Array`
/// for the `value` parameter, and the closure it takes either accepts `|key,
/// value|`, where "key" is always a string, or `|index, value|` where "index"
/// is always a number, depending on the parameter input type.
#[derive(Debug, Clone)]
pub struct Input {
    /// The parameter keyword upon which this closure input variant depends on.
    pub parameter_keyword: &'static str,

    /// The value kind this closure input expects from the parameter.
    pub kind: Kind,

    /// The list of variables attached to this closure input type.
    pub variables: Vec<Variable>,

    /// The return type this input variant expects the closure to have.
    pub output: Output,

    /// An example matching the given input.
    pub example: Example,
}

/// One variable input for a closure.
///
/// For example, in `{ |foo, bar| ... }`, `foo` and `bar` are each
/// a `Variable`.
#[derive(Debug, Clone)]
pub struct Variable {
    /// The value kind this variable will return when called.
    ///
    /// If set to `None`, the compiler is expected to provide this value at
    /// compile-time, or resort to `Kind::any()` if no information is known.
    pub kind: VariableKind,
}

/// The [`Value`] kind expected to be returned by a [`Variable`].
#[derive(Debug, Clone)]
pub enum VariableKind {
    /// An exact [`Kind`] means this variable is guaranteed to always contain
    /// a value that resolves to this kind.
    ///
    /// For example, in `map_keys`, it is known that the first (and only)
    /// variable the closure takes will be a `Kind::bytes()`.
    Exact(Kind),

    /// The variable [`Kind`] is inferred from the target of the closure.
    Target,

    /// The variable [`Kind`] is inferred from the inner kind of the target of
    /// the closure. This requires the closure target to be a collection type.
    TargetInnerValue,

    /// The variable [`Kind`] is inferred from the key or index type of the
    /// target. If the target is known to be exactly an object, this is always
    /// a `Value::bytes()`, if it's known to be exactly an array, it is
    /// a `Value::integer()`, otherwise it is one of the two.
    TargetInnerKey,
}

/// The output type required by the closure block.
#[derive(Debug, Clone)]
pub enum Output {
    Array {
        /// The number, and kind of elements expected.
        elements: Vec<Kind>,
    },

    Object {
        /// The field names, and value kinds expected.
        fields: BTreeMap<&'static str, Kind>,
    },

    Kind(
        /// The expected kind.
        Kind,
    ),
}

impl Output {
    #[must_use]
    pub fn into_kind(self) -> Kind {
        match self {
            Output::Array { elements } => {
                let collection: Collection<Index> = elements
                    .into_iter()
                    .enumerate()
                    .map(|(i, k)| (i.into(), k))
                    .collect::<BTreeMap<_, _>>()
                    .into();

                collection.into()
            }
            Output::Object { fields } => {
                let collection: Collection<Field> = fields
                    .into_iter()
                    .map(|(k, v)| (k.into(), v))
                    .collect::<BTreeMap<_, _>>()
                    .into();

                collection.into()
            }
            Output::Kind(kind) => kind,
        }
    }
}

pub struct Runner<'a, T> {
    pub(crate) variables: &'a [Ident],
    pub(crate) runner: T,
}

#[allow(clippy::missing_errors_doc)]
impl<'a, T> Runner<'a, T>
where
    T: Fn(&mut Context) -> Result<Value, ExpressionError>,
{
    pub fn new(variables: &'a [Ident], runner: T) -> Self {
        Self { variables, runner }
    }

    /// Run the closure to completion, given the provided key/value pair, and
    /// the runtime context.
    ///
    /// The provided values are *NOT* mutated during the run. See `map_key` or
    /// `map_value` for mutating alternatives.
    pub fn run_key_value(
        &self,
        ctx: &mut Context,
        key: &str,
        value: &Value,
    ) -> Result<Value, ExpressionError> {
        // TODO: we need to allow `LocalEnv` to take a mutable reference to
        // values, instead of owning them.
        let cloned_key = key.to_owned();
        let cloned_value = value.clone();

        let key_ident = self.ident(0);
        let value_ident = self.ident(1);

        let old_key = insert(ctx.state_mut(), key_ident, cloned_key.into());
        let old_value = insert(ctx.state_mut(), value_ident, cloned_value);

        let result = match (self.runner)(ctx) {
            Ok(value) | Err(ExpressionError::ControlFlow(ControlSignal::Return { value, .. })) => {
                Ok(value)
            }
            err @ Err(_) => err,
        };

        cleanup(ctx.state_mut(), key_ident, old_key);
        cleanup(ctx.state_mut(), value_ident, old_value);

        result
    }

    /// Run the closure to completion, given the provided index/value pair, and
    /// the runtime context.
    ///
    /// The provided values are *NOT* mutated during the run. See `map_key` or
    /// `map_value` for mutating alternatives.
    pub fn run_index_value(
        &self,
        ctx: &mut Context,
        index: usize,
        value: &Value,
    ) -> Result<Value, ExpressionError> {
        // TODO: we need to allow `LocalEnv` to take a mutable reference to
        // values, instead of owning them.
        let cloned_value = value.clone();

        let index_ident = self.ident(0);
        let value_ident = self.ident(1);

        let old_index = insert(ctx.state_mut(), index_ident, index.into());
        let old_value = insert(ctx.state_mut(), value_ident, cloned_value);

        let result = (self.runner)(ctx);

        cleanup(ctx.state_mut(), index_ident, old_index);
        cleanup(ctx.state_mut(), value_ident, old_value);

        result
    }

    /// Run the closure to completion, given the provided key, and the runtime
    /// context.
    ///
    /// The provided key is *MUTATED* by overwriting the key with the return
    /// value of the closure after completion.
    ///
    /// See `run_key_value` and `run_index_value` for immutable alternatives.
    pub fn map_key(&self, ctx: &mut Context, key: &mut KeyString) -> Result<(), ExpressionError> {
        // TODO: we need to allow `LocalEnv` to take a mutable reference to
        // values, instead of owning them.
        let cloned_key = key.clone();
        let ident = self.ident(0);
        let old_key = insert(ctx.state_mut(), ident, cloned_key.into());

        let result = (self.runner)(ctx);

        cleanup(ctx.state_mut(), ident, old_key);

        *key = result?.try_bytes_utf8_lossy()?.into();

        Ok(())
    }

    /// Run the closure to completion, given the provided value, and the runtime
    /// context.
    ///
    /// The provided value is *MUTATED* by overwriting the value with the return
    /// value of the closure after completion.
    ///
    /// See `run_key_value` and `run_index_value` for immutable alternatives.
    pub fn map_value(&self, ctx: &mut Context, value: &mut Value) -> Result<(), ExpressionError> {
        // TODO: we need to allow `LocalEnv` to take a mutable reference to
        // values, instead of owning them.
        let cloned_value = value.clone();
        let ident = self.ident(0);
        let old_value = insert(ctx.state_mut(), ident, cloned_value);

        let result = (self.runner)(ctx);

        cleanup(ctx.state_mut(), ident, old_value);

        *value = result?;

        Ok(())
    }

    /// Run the closure to completion, consuming the provided key/value pair,
    /// and the runtime context.
    ///
    /// Avoids cloning the value and skips binding parameters that are wildcards
    /// or omitted.
    pub fn run_key_value_owned(
        &self,
        ctx: &mut Context,
        key: KeyString,
        value: Value,
    ) -> Result<Value, ExpressionError> {
        match self.run_owned(ctx, Value::from(key), value) {
            Ok(val)
            | Err(ExpressionError::ControlFlow(ControlSignal::Return { value: val, .. })) => {
                Ok(val)
            }
            err @ Err(_) => err,
        }
    }

    /// Run the closure to completion, consuming the provided index and value,
    /// and the runtime context.
    ///
    /// Avoids cloning the value and skips binding parameters that are wildcards
    /// or omitted.
    pub fn run_index_value_owned(
        &self,
        ctx: &mut Context,
        index: usize,
        value: Value,
    ) -> Result<Value, ExpressionError> {
        self.run_owned(ctx, Value::from(index), value)
    }

    fn run_owned(
        &self,
        ctx: &mut Context,
        first: Value,
        second: Value,
    ) -> Result<Value, ExpressionError> {
        let first_ident = self.ident(0);
        let value_ident = self.ident(1);

        let old_first = insert(ctx.state_mut(), first_ident, first);
        let old_value = insert(ctx.state_mut(), value_ident, second);

        let result = (self.runner)(ctx);

        cleanup(ctx.state_mut(), first_ident, old_first);
        cleanup(ctx.state_mut(), value_ident, old_value);

        result
    }

    /// Create a scoped loop runner that amortizes variable scoping across loop iterations.
    ///
    /// Snapshots outer variable values before the loop begins, updates slots in-place
    /// across iterations avoiding re-allocation and ident cloning, and restores outer
    /// variables via an RAII drop guard upon normal completion, error, or early return.
    pub fn scoped_loop<'c, 'b>(
        &'a self,
        ctx: &'c mut Context<'b>,
    ) -> LoopScopeGuard<'a, 'c, 'b, T> {
        LoopScopeGuard::new(self, ctx)
    }

    fn ident(&self, index: usize) -> Option<&Ident> {
        self.variables
            .get(index)
            .and_then(|v| (!v.is_empty() && v.as_ref() != "_").then_some(v))
    }
}

/// An RAII guard that manages closure variable bindings across loop iterations.
///
/// It snapshots outer scope variable values before the loop begins,
/// updates slots in-place during the loop without allocations or clones,
/// and restores the outer state upon completion, early return, or error.
pub struct LoopScopeGuard<'a, 'c, 'b, T> {
    runner: &'a Runner<'a, T>,
    ctx: &'c mut Context<'b>,
    first_ident: Option<Ident>,
    first_old_value: Option<Value>,
    second_ident: Option<Ident>,
    second_old_value: Option<Value>,
}

pub type ScopedLoop<'a, 'c, 'b, T> = LoopScopeGuard<'a, 'c, 'b, T>;

impl<'a, 'c, 'b, T> LoopScopeGuard<'a, 'c, 'b, T>
where
    T: Fn(&mut Context) -> Result<Value, ExpressionError>,
{
    pub fn new(runner: &'a Runner<'a, T>, ctx: &'c mut Context<'b>) -> Self {
        let first_ident = runner.ident(0).cloned();
        let first_old_value = first_ident
            .as_ref()
            .and_then(|ident| ctx.state_mut().remove_variable(ident));

        let second_ident = runner.ident(1).cloned();
        let second_old_value = second_ident
            .as_ref()
            .and_then(|ident| ctx.state_mut().remove_variable(ident));

        Self {
            runner,
            ctx,
            first_ident,
            first_old_value,
            second_ident,
            second_old_value,
        }
    }

    pub fn run_key_value(
        &mut self,
        key: KeyString,
        value: Value,
    ) -> Result<Value, ExpressionError> {
        if let Some(ident) = &self.first_ident {
            self.ctx
                .state_mut()
                .set_or_insert_variable(ident, Value::from(key));
        }
        if let Some(ident) = &self.second_ident {
            self.ctx.state_mut().set_or_insert_variable(ident, value);
        }

        match (self.runner.runner)(self.ctx) {
            Ok(val)
            | Err(ExpressionError::ControlFlow(ControlSignal::Return { value: val, .. })) => {
                Ok(val)
            }
            err @ Err(_) => err,
        }
    }

    pub fn run_index_value(
        &mut self,
        index: usize,
        value: Value,
    ) -> Result<Value, ExpressionError> {
        if let Some(ident) = &self.first_ident {
            self.ctx
                .state_mut()
                .set_or_insert_variable(ident, Value::from(index));
        }
        if let Some(ident) = &self.second_ident {
            self.ctx.state_mut().set_or_insert_variable(ident, value);
        }

        (self.runner.runner)(self.ctx)
    }
}

impl<T> Drop for LoopScopeGuard<'_, '_, '_, T> {
    fn drop(&mut self) {
        if let Some(ident) = &self.first_ident {
            match self.first_old_value.take() {
                Some(val) => self.ctx.state_mut().set_or_insert_variable(ident, val),
                None => {
                    self.ctx.state_mut().remove_variable(ident);
                }
            }
        }
        if let Some(ident) = &self.second_ident {
            match self.second_old_value.take() {
                Some(val) => self.ctx.state_mut().set_or_insert_variable(ident, val),
                None => {
                    self.ctx.state_mut().remove_variable(ident);
                }
            }
        }
    }
}

fn insert(state: &mut RuntimeState, ident: Option<&Ident>, data: Value) -> Option<Value> {
    ident.and_then(|ident| state.swap_variable(ident.clone(), data))
}

fn cleanup(state: &mut RuntimeState, ident: Option<&Ident>, data: Option<Value>) {
    match (ident, data) {
        (Some(ident), Some(value)) => {
            state.insert_variable(ident.clone(), value);
        }
        (Some(ident), None) => {
            state.remove_variable(ident);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compiler::{Span, TimeZone};
    use std::collections::BTreeMap;

    fn ident(value: &str) -> Ident {
        Ident::from(value.to_owned())
    }

    fn test_context() -> (Value, RuntimeState, TimeZone) {
        (
            Value::from(BTreeMap::default()),
            RuntimeState::default(),
            TimeZone::Named(chrono_tz::Tz::UTC),
        )
    }

    #[test]
    fn run_key_value_owned_accesses_consumed_value_and_cleans_up() {
        let (mut target, mut state, tz) = test_context();
        let key_ident = ident("k");
        let val_ident = ident("v");
        state.insert_variable(val_ident.clone(), Value::from("outer_val"));
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let variables = vec![key_ident.clone(), val_ident.clone()];
        let runner = Runner::new(&variables, |ctx| {
            let k = ctx.state().variable(&ident("k")).cloned().unwrap();
            let v = ctx.state().variable(&ident("v")).cloned().unwrap();
            assert_eq!(k, Value::from("my_key"));
            assert_eq!(v, Value::from("my_val"));
            Ok(Value::from("done"))
        });

        let res =
            runner.run_key_value_owned(&mut ctx, KeyString::from("my_key"), Value::from("my_val"));
        assert_eq!(res, Ok(Value::from("done")));
        assert!(ctx.state().variable(&key_ident).is_none());
        assert_eq!(
            ctx.state().variable(&val_ident),
            Some(&Value::from("outer_val"))
        );
    }

    #[test]
    fn run_index_value_owned_accesses_consumed_value_and_cleans_up() {
        let (mut target, mut state, tz) = test_context();
        let idx_ident = ident("idx");
        let val_ident = ident("val");
        state.insert_variable(idx_ident.clone(), Value::from(999));
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let variables = vec![idx_ident.clone(), val_ident.clone()];
        let runner = Runner::new(&variables, |ctx| {
            let i = ctx.state().variable(&ident("idx")).cloned().unwrap();
            let v = ctx.state().variable(&ident("val")).cloned().unwrap();
            assert_eq!(i, Value::Integer(5));
            assert_eq!(v, Value::from("item_val"));
            Ok(Value::Null)
        });

        let res = runner.run_index_value_owned(&mut ctx, 5, Value::from("item_val"));
        assert_eq!(res, Ok(Value::Null));
        assert_eq!(ctx.state().variable(&idx_ident), Some(&Value::from(999)));
        assert!(ctx.state().variable(&val_ident).is_none());
    }

    #[test]
    fn wildcard_parameter_executes_without_binding_variable_to_state() {
        let (mut target, mut state, tz) = test_context();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        // Test with empty ident (parser representation of _) and "_"
        let variables = vec![ident(""), ident("_")];
        let runner = Runner::new(&variables, |ctx| {
            assert!(ctx.state().variable(&ident("")).is_none());
            assert!(ctx.state().variable(&ident("_")).is_none());
            Ok(Value::from("wildcard_ok"))
        });

        let key_value_result =
            runner.run_key_value_owned(&mut ctx, KeyString::from("key"), Value::from("val"));
        assert_eq!(key_value_result, Ok(Value::from("wildcard_ok")));

        let index_value_result = runner.run_index_value_owned(&mut ctx, 0, Value::from("val"));
        assert_eq!(index_value_result, Ok(Value::from("wildcard_ok")));

        assert!(ctx.state().variable(&ident("")).is_none());
        assert!(ctx.state().variable(&ident("_")).is_none());
    }

    #[test]
    fn unused_parameter_does_not_overwrite_outer_variable() {
        let (mut target, mut state, tz) = test_context();
        let outer_ident = ident("outer");
        state.insert_variable(outer_ident.clone(), Value::from("preserved"));
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        // Runner has only 1 variable (value only, key omitted or wildcard)
        let variables = vec![ident(""), ident("v")];
        let runner = Runner::new(&variables, |ctx| {
            assert_eq!(
                ctx.state().variable(&ident("outer")),
                Some(&Value::from("preserved"))
            );
            Ok(Value::Null)
        });

        let res = runner.run_key_value_owned(&mut ctx, KeyString::from("k"), Value::from("v_val"));
        assert_eq!(res, Ok(Value::Null));
        assert_eq!(
            ctx.state().variable(&outer_ident),
            Some(&Value::from("preserved"))
        );
    }

    #[test]
    fn owned_runners_handle_early_return() {
        let (mut target, mut state, tz) = test_context();
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let variables = vec![ident("i"), ident("v")];

        // Early returns are propagated unchanged.
        let runner_ret = Runner::new(&variables, |_ctx| {
            Err(ExpressionError::ControlFlow(ControlSignal::Return {
                span: Span::new(0, 0),
                value: Value::from("early_ret"),
            }))
        });

        let res_ret = runner_ret.run_index_value_owned(&mut ctx, 1, Value::from("val"));
        assert_eq!(
            res_ret,
            Err(ExpressionError::ControlFlow(ControlSignal::Return {
                span: Span::new(0, 0),
                value: Value::from("early_ret"),
            }))
        );
    }

    #[test]
    fn scoped_loop_updates_in_place_and_restores_outer_variables() {
        let (mut target, mut state, tz) = test_context();
        let key_ident = ident("k");
        let val_ident = ident("v");
        state.insert_variable(key_ident.clone(), Value::from("initial_k"));
        state.insert_variable(val_ident.clone(), Value::from("initial_v"));
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let variables = vec![key_ident.clone(), val_ident.clone()];
        let runner = Runner::new(&variables, |ctx| {
            let k = ctx.state().variable(&ident("k")).cloned().unwrap();
            let v = ctx.state().variable(&ident("v")).cloned().unwrap();
            Ok(Value::Array(vec![k, v]))
        });

        {
            let mut scoped = runner.scoped_loop(&mut ctx);
            let r1 = scoped.run_key_value(KeyString::from("a"), Value::from(1));
            assert_eq!(r1, Ok(Value::Array(vec![Value::from("a"), Value::from(1)])));
            let r2 = scoped.run_key_value(KeyString::from("b"), Value::from(2));
            assert_eq!(r2, Ok(Value::Array(vec![Value::from("b"), Value::from(2)])));
        }

        assert_eq!(
            ctx.state().variable(&key_ident),
            Some(&Value::from("initial_k"))
        );
        assert_eq!(
            ctx.state().variable(&val_ident),
            Some(&Value::from("initial_v"))
        );
    }

    #[test]
    fn scoped_loop_cleans_up_new_variables_on_drop() {
        let (mut target, mut state, tz) = test_context();
        let idx_ident = ident("idx");
        let val_ident = ident("val");
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let variables = vec![idx_ident.clone(), val_ident.clone()];
        let runner = Runner::new(&variables, |_ctx| Ok(Value::Null));

        {
            let mut scoped = runner.scoped_loop(&mut ctx);
            assert!(scoped.run_index_value(0, Value::from("x")).is_ok());
            assert!(scoped.run_index_value(1, Value::from("y")).is_ok());
            // Inside the loop, variables exist in state
            assert_eq!(
                scoped.ctx.state().variable(&idx_ident),
                Some(&Value::Integer(1))
            );
            assert_eq!(
                scoped.ctx.state().variable(&val_ident),
                Some(&Value::from("y"))
            );
        }

        // After loop drop, variables are completely removed
        assert!(ctx.state().variable(&idx_ident).is_none());
        assert!(ctx.state().variable(&val_ident).is_none());
    }

    #[test]
    fn scoped_loop_restores_outer_variables_on_early_return_and_error() {
        let (mut target, mut state, tz) = test_context();
        let idx_ident = ident("i");
        let val_ident = ident("v");
        state.insert_variable(idx_ident.clone(), Value::from(100));
        state.insert_variable(val_ident.clone(), Value::from(200));
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let variables = vec![idx_ident.clone(), val_ident.clone()];

        // Test early return
        let runner_ret = Runner::new(&variables, |_ctx| {
            Err(ExpressionError::ControlFlow(ControlSignal::Return {
                span: Span::new(0, 0),
                value: Value::from("early_val"),
            }))
        });

        {
            let mut scoped = runner_ret.scoped_loop(&mut ctx);
            let res = scoped.run_index_value(0, Value::from("temp"));
            assert_eq!(
                res,
                Err(ExpressionError::ControlFlow(ControlSignal::Return {
                    span: Span::new(0, 0),
                    value: Value::from("early_val"),
                }))
            );
        }
        assert_eq!(ctx.state().variable(&idx_ident), Some(&Value::from(100)));
        assert_eq!(ctx.state().variable(&val_ident), Some(&Value::from(200)));

        // Test error exit
        let runner_err = Runner::new(&variables, |_ctx| Err(ExpressionError::from("test error")));

        {
            let mut scoped = runner_err.scoped_loop(&mut ctx);
            let res = scoped.run_index_value(0, Value::from("temp"));
            assert!(res.is_err());
        }
        assert_eq!(ctx.state().variable(&idx_ident), Some(&Value::from(100)));
        assert_eq!(ctx.state().variable(&val_ident), Some(&Value::from(200)));
    }

    #[test]
    fn scoped_loop_handles_wildcard_parameters() {
        let (mut target, mut state, tz) = test_context();
        let outer_ident = ident("outer");
        state.insert_variable(outer_ident.clone(), Value::from("outer_saved"));
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let variables = vec![ident(""), ident("_")];
        let runner = Runner::new(&variables, |ctx| {
            assert!(ctx.state().variable(&ident("")).is_none());
            assert!(ctx.state().variable(&ident("_")).is_none());
            assert_eq!(
                ctx.state().variable(&ident("outer")),
                Some(&Value::from("outer_saved"))
            );
            Ok(Value::Null)
        });

        {
            let mut scoped = runner.scoped_loop(&mut ctx);
            let res = scoped.run_key_value(KeyString::from("k"), Value::from("v"));
            assert_eq!(res, Ok(Value::Null));
        }

        assert!(ctx.state().variable(&ident("")).is_none());
        assert!(ctx.state().variable(&ident("_")).is_none());
        assert_eq!(
            ctx.state().variable(&outer_ident),
            Some(&Value::from("outer_saved"))
        );
    }

    #[test]
    fn scoped_loop_shadows_and_restores_outer_variable_without_cloning() {
        let (mut target, mut state, tz) = test_context();
        let idx_ident = ident("i");
        let val_ident = ident("v");
        let initial_array = Value::Array(vec![Value::from(1), Value::from(2), Value::from(3)]);
        state.insert_variable(val_ident.clone(), initial_array.clone());
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let variables = vec![idx_ident.clone(), val_ident.clone()];
        let runner = Runner::new(&variables, |ctx| {
            let current_v = ctx.state().variable(&ident("v")).unwrap();
            assert_ne!(
                current_v,
                &Value::Array(vec![Value::from(1), Value::from(2), Value::from(3)])
            );
            Ok(Value::Null)
        });

        {
            let mut scoped = runner.scoped_loop(&mut ctx);
            assert!(scoped.run_index_value(0, Value::from("new_val")).is_ok());
        }

        assert_eq!(ctx.state().variable(&val_ident), Some(&initial_array));
    }

    #[test]
    fn run_index_value_propagates_break_and_cleans_up() {
        let (mut target, mut state, tz) = test_context();
        let val_ident = ident("val");
        state.insert_variable(val_ident.clone(), Value::from(42));
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let variables = vec![ident("idx"), val_ident.clone()];
        let runner = Runner::new(&variables, |_ctx| {
            Err(ExpressionError::ControlFlow(ControlSignal::Break {
                span: Span::new(0, 5),
            }))
        });

        let res = runner.run_index_value(&mut ctx, 0, &Value::from(10));
        assert!(matches!(
            res,
            Err(ExpressionError::ControlFlow(ControlSignal::Break { .. }))
        ));
        assert!(ctx.state().variable(&ident("idx")).is_none());
        assert_eq!(ctx.state().variable(&val_ident), Some(&Value::from(42)));
    }

    #[test]
    fn run_key_value_propagates_break_and_cleans_up() {
        let (mut target, mut state, tz) = test_context();
        let val_ident = ident("val");
        state.insert_variable(val_ident.clone(), Value::from(42));
        let mut ctx = Context::new(&mut target, &mut state, &tz);

        let variables = vec![ident("key"), val_ident.clone()];
        let runner = Runner::new(&variables, |_ctx| {
            Err(ExpressionError::ControlFlow(ControlSignal::Break {
                span: Span::new(0, 5),
            }))
        });

        let res = runner.run_key_value(&mut ctx, "k", &Value::from(10));
        assert!(matches!(
            res,
            Err(ExpressionError::ControlFlow(ControlSignal::Break { .. }))
        ));
        assert!(ctx.state().variable(&ident("key")).is_none());
        assert_eq!(ctx.state().variable(&val_ident), Some(&Value::from(42)));
    }
}
