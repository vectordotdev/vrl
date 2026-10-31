use std::{collections::BTreeMap, fmt, ops::Deref};

use crate::value::{KeyString, Value};
use crate::{
    compiler::{
        Context, Expression, TypeDef,
        expression::{Expr, Resolved, container},
        state::{TypeInfo, TypeState},
    },
    value::Kind,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    inner: BTreeMap<KeyString, Expr>,
    /// Pre-computed value if this object is a *structural* literal (every
    /// value is a literal, or a nested object/array/group of literals).
    ///
    /// `resolve` can then bulk-clone this value instead of re-assembling the
    /// map (resolving each entry, cloning keys, re-balancing tree nodes) on
    /// every evaluation.
    ///
    /// This intentionally does NOT use `Expression::resolve_constant`:
    /// that API can fold `Variable`/`Query` expressions based on
    /// compile-time state, which is unsound as a source of runtime values
    /// when an earlier object entry runs side effects that mutate the
    /// variable (see vectordotdev/vrl#1799). Only structurally
    /// side-effect-free expressions qualify.
    constant: Option<Value>,
}

impl Object {
    #[must_use]
    pub fn new(inner: BTreeMap<KeyString, Expr>) -> Self {
        Self {
            inner,
            constant: None,
        }
    }

    /// Compute and cache the value of this object if it is structurally
    /// literal (see the `constant` field). Callers must invoke this after the
    /// object is fully built, before the program is executed.
    #[must_use]
    pub fn with_constant(mut self) -> Self {
        let value = self.structural_literal_value();
        if value.is_some() {
            // The parent cache now holds a copy of every descendant's value;
            // drop the descendant caches so nested literals are stored once.
            self.drop_descendant_constants();
        }
        self.constant = value;
        self
    }

    /// Build the constant value from structurally literal expressions only.
    fn structural_literal_value(&self) -> Option<Value> {
        let mut map = BTreeMap::new();
        for (key, expr) in &self.inner {
            map.insert(key.clone(), literal_expr_value(expr)?);
        }
        Some(Value::Object(map))
    }

    /// Drop the caches of structurally literal descendant objects. Only used
    /// after this object's own cache was built from them, so that nested
    /// literal subtrees are stored exactly once.
    fn drop_descendant_constants(&mut self) {
        for expr in self.inner.values_mut() {
            drop_expr_constant(expr);
        }
    }
}

/// Resolve the value of an expression only if it is structurally
/// side-effect-free and context-independent: literals, and nested
/// objects/arrays/groups thereof.
///
/// Everything else (variables, queries, ops, function calls, blocks, ...)
/// yields `None`, falling back to the regular runtime resolve path.
fn literal_expr_value(expr: &Expr) -> Option<Value> {
    match expr {
        Expr::Literal(literal) => Some(literal.to_value()),
        Expr::Container(container) => match &container.variant {
            // Nested objects already computed their cache in `compile_object`;
            // reuse it instead of re-materializing the subtree.
            container::Variant::Object(object) => object.constant.clone(),
            container::Variant::Array(array) => array
                .iter()
                .map(literal_expr_value)
                .collect::<Option<Vec<_>>>()
                .map(Value::Array),
            container::Variant::Group(group) => literal_expr_value(group.expr()),
            container::Variant::Block(_) => None,
        },
        _ => None,
    }
}

/// Recursively drop the caches of structurally literal descendant objects.
fn drop_expr_constant(expr: &mut Expr) {
    if let Expr::Container(container) = expr
        && let container::Variant::Object(object) = &mut container.variant
    {
        object.constant = None;
        for expr in object.inner.values_mut() {
            drop_expr_constant(expr);
        }
    }
}

impl Deref for Object {
    type Target = BTreeMap<KeyString, Expr>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Expression for Object {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        if let Some(value) = &self.constant {
            // Preserve cooperative interruption: the uncached path checkpoints
            // around every entry expression via `Expr::resolve`; the cached
            // path resolves no entries, so checkpoint here explicitly before
            // the (bounded, literal-only) clone.
            ctx.checkpoint()?;
            return Ok(value.clone());
        }

        self.inner
            .iter()
            .map(|(key, expr)| expr.resolve(ctx).map(|v| (key.clone(), v)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(Value::Object)
    }

    fn resolve_constant(&self, state: &TypeState) -> Option<Value> {
        self.inner
            .iter()
            .map(|(key, expr)| expr.resolve_constant(state).map(|v| (key.clone(), v)))
            .collect::<Option<BTreeMap<_, _>>>()
            .map(Value::Object)
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        let mut state = state.clone();
        let mut fallible = false;
        let mut returns = Kind::never();

        let mut type_defs = BTreeMap::new();
        for (k, expr) in &self.inner {
            let type_def = expr.apply_type_info(&mut state).upgrade_undefined();
            returns.merge_keep(type_def.returns().clone(), false);

            // If any expression is fallible, the entire object is fallible.
            fallible |= type_def.is_fallible();

            // If any expression aborts, the entire object aborts
            if type_def.is_never() {
                return TypeInfo::new(
                    state,
                    TypeDef::never()
                        .maybe_fallible(fallible)
                        .with_returns(returns),
                );
            }
            type_defs.insert(k.clone(), type_def);
        }

        let collection = type_defs
            .into_iter()
            .map(|(field, type_def)| (field.into(), type_def.into()))
            .collect::<BTreeMap<_, _>>();

        let result = TypeDef::object(collection)
            .maybe_fallible(fallible)
            .with_returns(returns);
        TypeInfo::new(state, result)
    }
}

impl fmt::Display for Object {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let exprs = self
            .inner
            .iter()
            .map(|(k, v)| format!(r#""{k}": {v}"#))
            .collect::<Vec<_>>()
            .join(", ");

        write!(f, "{{ {exprs} }}")
    }
}

impl From<BTreeMap<KeyString, Expr>> for Object {
    fn from(inner: BTreeMap<KeyString, Expr>) -> Self {
        Self {
            inner,
            constant: None,
        }
    }
}
