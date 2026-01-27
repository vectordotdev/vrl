use crate::compiler::prelude::*;
use std::collections::BTreeMap;
use std::sync::LazyLock;

static DEFAULT_DEEP: LazyLock<Value> = LazyLock::new(|| Value::Boolean(false));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "to",
            kind: kind::OBJECT,
            required: true,
            description: "The object to merge into.",
            default: None,
        },
        Parameter {
            keyword: "from",
            kind: kind::OBJECT,
            required: true,
            description: "The object to merge from.",
            default: None,
        },
        Parameter {
            keyword: "deep",
            kind: kind::BOOLEAN,
            required: false,
            description: "A deep merge is performed if `true`, otherwise only top-level fields are merged.",
            default: Some(&DEFAULT_DEEP),
        },
    ]
});

#[derive(Clone, Copy, Debug)]
pub struct Merge;

impl Function for Merge {
    fn identifier(&self) -> &'static str {
        "merge"
    }

    fn usage(&self) -> &'static str {
        "Merges the `from` object into the `to` object."
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Object merge (shallow)",
                source: indoc! {r#"
                    merge(
                        {
                            "parent1": {
                                "child1": 1,
                                "child2": 2
                            },
                            "parent2": {
                                "child3": 3
                            }
                        },
                        {
                            "parent1": {
                                "child2": 4,
                                "child5": 5
                            }
                        }
                    )
                "#},
                result: Ok(r#"{ "parent1": { "child2": 4, "child5": 5 }, "parent2": { "child3": 3 } }"#),
            },
            example! {
                title: "Object merge (deep)",
                source: indoc! {r#"
                    merge(
                        {
                            "parent1": {
                                "child1": 1,
                                "child2": 2
                            },
                            "parent2": {
                                "child3": 3
                            }
                        },
                        {
                            "parent1": {
                                "child2": 4,
                                "child5": 5
                            }
                        },
                        deep: true
                    )
                "#},
                result: Ok(r#"{ "parent1": { "child1": 1, "child2": 4, "child5": 5 }, "parent2": { "child3": 3 } }"#),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let to = arguments.required("to");
        let from = arguments.required("from");
        let deep = arguments.optional("deep");

        Ok(MergeFn { to, from, deep }.as_expr())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct MergeFn {
    to: Box<dyn Expression>,
    from: Box<dyn Expression>,
    deep: Option<Box<dyn Expression>>,
}

impl FunctionExpression for MergeFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let mut to_value = self.to.resolve(ctx)?.try_object()?;
        let from_value = self.from.resolve(ctx)?.try_object()?;
        let deep = self
            .deep
            .map_resolve_with_default(ctx, || DEFAULT_DEEP.clone())?
            .try_boolean()?;

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
            (true, Some(Value::Object(child1)), Value::Object(child2)) => {
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
