use std::{collections::BTreeMap, fmt, ops::Deref};

use crate::value::{KeyString, ObjectMap, Value};
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
    /// Pre-computed value if this object is a *structural* literal (nested
    /// literals only), cloned wholesale on resolve. Not
    /// `Expression::resolve_constant`: that can fold variables from
    /// compile-time state, which is unsound (vectordotdev/vrl#1799).
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

    /// Compute and cache the value if this object is structurally literal.
    /// Idempotent; call once the object is fully built.
    #[must_use]
    pub fn with_constant(mut self) -> Self {
        if self.constant.is_some() {
            return self;
        }
        let value = self.structural_literal_value();
        if value.is_some() {
            // The parent cache holds every descendant value; store subtrees once.
            self.drop_descendant_constants();
        }
        self.constant = value;
        self
    }

    /// Build the constant value from structurally literal expressions only.
    ///
    /// Eligibility is checked before materializing the map, so an object
    /// with a dynamic entry does not pay for partial map construction.
    fn structural_literal_value(&self) -> Option<Value> {
        if !self.inner.values().all(is_structural_literal) {
            return None;
        }
        self.inner
            .iter()
            .map(|(key, expr)| literal_expr_value(expr).map(|v| (key.clone(), v)))
            .collect::<Option<ObjectMap>>()
            .map(Value::Object)
    }

    /// Drop descendant caches once the parent cache is built from them.
    fn drop_descendant_constants(&mut self) {
        for expr in self.inner.values_mut() {
            drop_expr_constant(expr);
        }
    }
}

/// Whether `literal_expr_value` returns `Some` for this expression; keep
/// in sync with it. Cheap tree predicate, no values materialized.
fn is_structural_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(_) => true,
        Expr::Container(container) => match &container.variant {
            // A nested object qualifies iff its cache was built.
            container::Variant::Object(object) => object.constant.is_some(),
            container::Variant::Array(array) => array.iter().all(is_structural_literal),
            container::Variant::Group(group) => is_structural_literal(group.expr()),
            container::Variant::Block(_) => false,
        },
        _ => false,
    }
}

/// Resolve the value of an expression if it is a structural literal
/// (literals and nested object/array/group containers thereof).
fn literal_expr_value(expr: &Expr) -> Option<Value> {
    match expr {
        Expr::Literal(literal) => Some(literal.to_value()),
        Expr::Container(container) => match &container.variant {
            // Nested objects computed their cache in `compile_object`; reuse it.
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

/// Recursively drop descendant caches through objects, arrays and groups.
fn drop_expr_constant(expr: &mut Expr) {
    if let Expr::Container(container) = expr {
        match &mut container.variant {
            container::Variant::Object(object) => {
                object.constant = None;
                for expr in object.inner.values_mut() {
                    drop_expr_constant(expr);
                }
            }
            container::Variant::Array(array) => {
                for expr in array.iter_mut() {
                    drop_expr_constant(expr);
                }
            }
            container::Variant::Group(group) => drop_expr_constant(group.expr_mut()),
            // Blocks never appear inside a structurally literal subtree.
            container::Variant::Block(_) => {}
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
            // Keep cooperative interruption: the uncached path checkpoints
            // per entry; an empty object must not gain an extra one.
            if !self.inner.is_empty() {
                ctx.checkpoint()?;
            }
            return Ok(value.clone());
        }

        self.inner
            .iter()
            .map(|(key, expr)| expr.resolve(ctx).map(|v| (key.clone(), v)))
            .collect::<Result<ObjectMap, _>>()
            .map(Value::Object)
    }

    fn resolve_constant(&self, state: &TypeState) -> Option<Value> {
        self.inner
            .iter()
            .map(|(key, expr)| expr.resolve_constant(state).map(|v| (key.clone(), v)))
            .collect::<Option<ObjectMap>>()
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
