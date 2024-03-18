use std::{collections::BTreeMap, fmt, ops::Deref};

use crate::value::Value;
use crate::{
    compiler::{
        expression::{Expr, Resolved},
        state::{TypeInfo, TypeState},
        Context, Expression, TypeDef,
    },
    value::Kind,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Array {
    inner: Vec<Expr>,
}

impl Array {
    pub(crate) fn new(inner: Vec<Expr>) -> Self {
        Self { inner }
    }
}

impl Deref for Array {
    type Target = Vec<Expr>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl Expression for Array {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.inner
            .iter()
            .map(|expr| expr.resolve(ctx))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array)
    }

    fn resolve_constant(&self, state: &TypeState) -> Option<Value> {
        self.inner
            .iter()
            .map(|x| x.resolve_constant(state))
            .collect::<Option<Vec<_>>>()
            .map(Value::Array)
    }

    fn type_info(&self, state: &TypeState) -> TypeInfo {
        let mut state = state.clone();

        let mut type_defs = vec![];
        let mut fallible = false;

        for expr in &self.inner {
            let type_def = expr.apply_type_info(&mut state).upgrade_undefined();

            // If any expression is fallible, the entire array is fallible.
            fallible |= type_def.is_fallible();

            // If any expression aborts, the entire array aborts
            if type_def.is_never() {
                return TypeInfo::new(state, TypeDef::never().maybe_fallible(fallible));
            }
            type_defs.push(type_def);
        }

        let returns = type_defs.iter().fold(Kind::never(), |returns, type_def| {
            returns.union(type_def.returns().clone())
        });

        let collection = type_defs
            .into_iter()
            .enumerate()
            .map(|(index, type_def)| (index.into(), type_def.into()))
            .collect::<BTreeMap<_, _>>();

        TypeInfo::new(
            state,
            TypeDef::array(collection)
                .maybe_fallible(fallible)
                .with_returns(returns),
        )
    }
}

impl fmt::Display for Array {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let exprs = self
            .inner
            .iter()
            .map(Expr::to_string)
            .collect::<Vec<_>>()
            .join(", ");

        write!(f, "[{exprs}]")
    }
}

impl From<Vec<Expr>> for Array {
    fn from(inner: Vec<Expr>) -> Self {
        Self { inner }
    }
}

#[cfg(test)]
mod tests {
    use crate::value::kind::Collection;
    use crate::{expr, test_type_def, value::Kind};

    use super::*;

    test_type_def![
        empty_array {
            expr: |_| expr!([]),
            want: TypeDef::array(Collection::empty()),
        }

        scalar_array {
            expr: |_| expr!([1, "foo", true]),
            want: TypeDef::array(BTreeMap::from([
                (0.into(), Kind::integer()),
                (1.into(), Kind::bytes()),
                (2.into(), Kind::boolean()),
            ])),
        }

        mixed_array {
            expr: |_| expr!([1, [true, "foo"], { "bar": null }]),
            want: TypeDef::array(BTreeMap::from([
                (0.into(), Kind::integer()),
                (1.into(), Kind::array(BTreeMap::from([
                    (0.into(), Kind::boolean()),
                    (1.into(), Kind::bytes()),
                ]))),
                (2.into(), Kind::object(BTreeMap::from([
                    ("bar".into(), Kind::null())
                ]))),
            ])),
        }
    ];
}
