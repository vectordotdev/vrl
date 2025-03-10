use std::collections::BTreeMap;

use crate::compiler::{
    value::{Kind, VrlValueConvert},
    Context, ExpressionError,
};
use crate::parser::ast::Ident;
use crate::value::{
    kind::{Collection, Field, Index},
    KeyString, Value,
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

    /// The variable [`Kind`] is inferred from a parameter of the closure.
    ///
    /// For example, `VariableKind::Parameter('initial')` is equivalent to `VariableKind::Target`
    /// where the [`Input`] `parameter_keyword` is "initial".
    Parameter(&'static str),

    /// The variable [`Kind`] is inferred from the closure's output. The inner
    /// value is the type used initially when determining the closure's output.
    Closure(InitialKind),

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

impl From<InitialKind> for VariableKind {
    fn from(initial_kind: InitialKind) -> Self {
        match initial_kind {
            InitialKind::Exact(kind) => VariableKind::Exact(kind),
            InitialKind::Parameter(parameter) => VariableKind::Parameter(parameter),
            InitialKind::Target => VariableKind::Target,
            InitialKind::TargetInnerValue => VariableKind::TargetInnerValue,
            InitialKind::TargetInnerKey => VariableKind::TargetInnerKey,
        }
    }
}

/// If a [`Variable`] is inferring its [`Value`] kind from a closure (see
/// [`VariableKind::Closure`]), this is the initial value used for the variable.
#[derive(Debug, Clone)]
pub enum InitialKind {
    /// Equivalent to [`VariableKind::Exact`]
    Exact(Kind),

    /// Equivalent to [`VariableKind::Parameter`]
    Parameter(&'static str),

    /// Equivalent to [`VariableKind::Target`]
    Target,

    /// Equivalent to [`VariableKind::TargetInnerValue`]
    TargetInnerValue,

    /// Equivalent to [`VariableKind::TargetInnerKey`]
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

enum SwapSpace<'a> {
    Owned(Vec<Option<Value>>),
    Borrowed(&'a mut [Option<Value>]),
}

impl SwapSpace<'_> {
    fn as_mut_slice(&mut self) -> &mut [Option<Value>] {
        match self {
            SwapSpace::Owned(v) => v.as_mut_slice(),
            SwapSpace::Borrowed(s) => s,
        }
    }
}

#[must_use]
pub struct FluentRunnerInterface<'a, 'b, T> {
    parent: &'a FluentRunner<'a, T>,
    swap_space: SwapSpace<'b>,
}

impl<'a, 'b, T> FluentRunnerInterface<'a, 'b, T>
where
    T: Fn(&mut Context) -> Result<Value, ExpressionError>,
{
    fn new(parent: &'a FluentRunner<'a, T>, swap_space: Option<&'b mut [Option<Value>]>) -> Self {
        let swap_space = if let Some(s) = swap_space {
            SwapSpace::Borrowed(s)
        } else {
            let mut swap_space = Vec::new();
            swap_space.resize_with(parent.variables.len(), Default::default);
            SwapSpace::Owned(swap_space)
        };

        Self { parent, swap_space }
    }

    /// Adds a new parameter to the runner. The `index` corresponds with the index of the supplied
    /// variables.
    pub fn parameter(mut self, ctx: &mut Context, index: usize, value: Value) -> Self {
        self.parent
            .parameter(self.swap_space.as_mut_slice(), ctx, index, value);
        self
    }

    /// Run the closure to completion, given the supplied parameters, and the runtime context.
    pub fn run(mut self, ctx: &mut Context) -> Result<Value, ExpressionError> {
        self.parent.run(self.swap_space.as_mut_slice(), ctx)
    }
}

pub struct FluentRunner<'a, T> {
    variables: &'a [Ident],
    runner: T,
}

impl<'a, T> FluentRunner<'a, T>
where
    T: Fn(&mut Context) -> Result<Value, ExpressionError>,
{
    pub fn new(variables: &'a [Ident], runner: T) -> Self {
        Self { variables, runner }
    }

    /// Creates a new [`FluentRunnerInterface`] with a temporary swap space equal in size to the
    /// number of provided variables.
    ///
    /// This is useful when a closure is expected to only run once.
    pub fn with_tmp_swap_space(&'a self) -> FluentRunnerInterface<'a, 'a, T> {
        FluentRunnerInterface::new(self, None)
    }

    /// Creates a new [`FluentRunnerInterface`] with a supplied swap space.
    ///
    /// This is useful for repeating closures that need the same sized swap space.
    pub fn with_swap_space<'b>(
        &'a self,
        swap_space: &'b mut [Option<Value>],
    ) -> FluentRunnerInterface<'a, 'b, T> {
        FluentRunnerInterface::new(self, Some(swap_space))
    }

    fn parameter(
        &self,
        swap_space: &mut [Option<Value>],
        ctx: &mut Context,
        index: usize,
        value: Value,
    ) {
        let ident = self.variables.get(index).filter(|i| !i.is_empty()).cloned();

        if let Some(swap) = swap_space.get_mut(index) {
            *swap = ident.and_then(|ident| ctx.state_mut().swap_variable(ident, value));
        }
    }

    fn run(
        &self,
        swap_space: &mut [Option<Value>],
        ctx: &mut Context,
    ) -> Result<Value, ExpressionError> {
        let value = (self.runner)(ctx)?;
        let state = ctx.state_mut();

        for (old_value, ident) in swap_space.iter().zip(self.variables) {
            match old_value {
                Some(value) => {
                    state.insert_variable(ident.clone(), value.clone());
                }
                None => state.remove_variable(ident),
            }
        }

        Ok(value)
    }
}

pub struct Runner<'a, T> {
    inner_runner: FluentRunner<'a, T>,
}

#[allow(clippy::missing_errors_doc)]
impl<'a, T> Runner<'a, T>
where
    T: Fn(&mut Context) -> Result<Value, ExpressionError>,
{
    pub fn new(variables: &'a [Ident], runner: T) -> Self {
        let inner_runner = FluentRunner::new(variables, runner);
        Self { inner_runner }
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
        let mut swap_space: [Option<Value>; 2] = [None, None];

        let result = match self
            .inner_runner
            .with_swap_space(&mut swap_space)
            .parameter(ctx, 0, key.to_owned().into())
            .parameter(ctx, 1, value.clone())
            .run(ctx)
        {
            Ok(value) | Err(ExpressionError::Return { value, .. }) => Ok(value),
            err @ Err(_) => err,
        };

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
        let mut swap_space: [Option<Value>; 2] = [None, None];

        self.inner_runner
            .with_swap_space(&mut swap_space)
            .parameter(ctx, 0, index.into())
            .parameter(ctx, 1, value.clone())
            .run(ctx)
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
        let mut swap_space: [Option<Value>; 1] = [None];

        *key = self
            .inner_runner
            .with_swap_space(&mut swap_space)
            .parameter(ctx, 0, key.clone().into())
            .run(ctx)?
            .try_bytes_utf8_lossy()?
            .into();

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
        let mut swap_space: [Option<Value>; 1] = [None];

        *value = self
            .inner_runner
            .with_swap_space(&mut swap_space)
            .parameter(ctx, 0, value.clone())
            .run(ctx)?;

        Ok(())
    }
}
