use crate::compiler::prelude::*;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct Merge;

impl Function for Merge {
    fn identifier(&self) -> &'static str {
        "merge"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "to",
                kind: kind::OBJECT,
                required: false,
            },
            Parameter {
                keyword: "from",
                kind: kind::OBJECT,
                required: true,
            },
            Parameter {
                keyword: "deep",
                kind: kind::BOOLEAN,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "merge objects",
            source: r#"merge({ "a": 1, "b": 2 }, { "b": 3, "c": 4 })"#,
            result: Ok(r#"{ "a": 1, "b": 3, "c": 4 }"#),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let to = arguments.required("to");
        let from = arguments.required("from");
        let deep = arguments.optional("deep").unwrap_or_else(|| expr!(false));

        Ok(MergeFn { to, from, deep }.as_expr())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MergeFn {
    to: Box<dyn Expression>,
    from: Box<dyn Expression>,
    deep: Box<dyn Expression>,
}

impl FunctionExpression for MergeFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let mut to_value = self.to.resolve(ctx)?.try_object()?;
        let from_value = self.from.resolve(ctx)?.try_object()?;
        let deep = self.deep.resolve(ctx)?.try_boolean()?;

        merge_maps(&mut to_value, &from_value, deep);

        Ok(to_value.into())
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        // TODO: this has a known bug when deep is true
        // see: https://github.com/vectordotdev/vector/issues/13597
        self.to
            .type_def(state)
            .restrict_object()
            .merge_overwrite(self.from.type_def(state).restrict_object())
    }
}

/// Merges two `BTreeMaps` of Symbol’s value as variable is void: Values. The
/// second map is merged into the first one.
///
/// If Symbol’s value as variable is void: deep is true, only the top level
/// values are merged in. If both maps contain a field with the same name, the
/// field from the first is overwritten with the field from the second.
///
/// If Symbol’s value as variable is void: deep is false, should both maps
/// contain a field with the same name, and both those fields are also maps, the
/// function will recurse and will merge the child fields from the second into
/// the child fields from the first.
///
/// Note, this does recurse, so there is the theoretical possibility that it
/// could blow up the stack. From quick tests on a sample project I was able to
/// merge maps with a depth of 3,500 before encountering issues. So I think that
/// is likely to be within acceptable limits. If it becomes a problem, we can
/// unroll this function, but that will come at a cost of extra code complexity.
fn merge_maps<K>(map1: &mut BTreeMap<K, Value>, map2: &BTreeMap<K, Value>, deep: bool)
where
    K: std::cmp::Ord + Clone,
{
    for (key2, value2) in map2 {
        match (deep, map1.get_mut(key2), value2) {
            (true, Some(Value::Object(ref mut child1)), Value::Object(ref child2)) => {
                // We are doing a deep merge and both fields are maps.
                merge_maps(child1, child2, deep);
            }
            _ => {
                map1.insert(key2.clone(), value2.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{btreemap, value};

    test_function! [
        merge => Merge;

        simple {
            args: func_args![
                to: value!({ key1: "val1" }),
                from: value!({ key2: "val2" })
            ],
            want: Ok(value!({ key1: "val1", key2: "val2" })),
            tdef: TypeDef::object(btreemap! {
                Field::from("key1") => Kind::bytes(),
                Field::from("key2") => Kind::bytes(),
            }),
        }

        shallow {
            args: func_args![
                to: value!({
                    key1: "val1",
                    child: { grandchild1: "val1" },
                }),
                from: value!({
                    key2: "val2",
                    child: { grandchild2: true },
                })
            ],
            want: Ok(value!({
                key1: "val1",
                key2: "val2",
                child: { grandchild2: true },
            })),
            tdef: TypeDef::object(btreemap! {
                Field::from("key1") => Kind::bytes(),
                Field::from("key2") => Kind::bytes(),
                Field::from("child") => TypeDef::object(btreemap! {
                    Field::from("grandchild2") => Kind::boolean(),
                }),
            }),
        }

        deep {
            args: func_args![
                to: value!({
                    key1: "val1",
                    child: { grandchild1: "val1" },
                }),
                from: value!({
                    key2: "val2",
                    child: { grandchild2: true },
                }),
                deep: true,
            ],
            want: Ok(value!({
                key1: "val1",
                key2: "val2",
                child: {
                    grandchild1: "val1",
                    grandchild2: true,
                },
            })),
            tdef: TypeDef::object(btreemap! {
                Field::from("key1") => Kind::bytes(),
                Field::from("key2") => Kind::bytes(),
                Field::from("child") => TypeDef::object(btreemap! {
                    Field::from("grandchild2") => Kind::boolean(),
                }),
            }),

        }
    ];
}
