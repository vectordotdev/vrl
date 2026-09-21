use crate::compiler::expression::{Expr, Literal};
use crate::compiler::prelude::*;
use std::collections::{
    BTreeMap,
    btree_map::{self, Entry},
};

static DEFAULT_DEEP: Value = Value::Boolean(false);

const PARAMETERS: &[Parameter] = &[
    Parameter::required("to", kind::OBJECT, "The object to merge into."),
    Parameter::required("from", kind::OBJECT, "The object to merge from."),
    Parameter::optional(
        "deep",
        kind::BOOLEAN,
        "A deep merge is performed if `true`, otherwise only top-level fields are merged.",
    )
    .default(&DEFAULT_DEEP),
];

#[derive(Clone, Copy, Debug)]
pub struct Merge;

impl Function for Merge {
    fn identifier(&self) -> &'static str {
        "merge"
    }

    fn usage(&self) -> &'static str {
        "Merges the `from` object into the `to` object."
    }

    fn category(&self) -> &'static str {
        Category::Object.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::OBJECT
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "The field from the `from` object is chosen if a key exists in both objects.",
            "Objects are merged recursively if `deep` is specified, a key exists in both objects, and both of those
fields are also objects.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS
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
        let deep_expr = arguments.optional_expr("deep");

        let deep = match deep_expr {
            Some(Expr::Literal(Literal::Boolean(b))) => DeepMode::Const(b),
            Some(expr) => DeepMode::Dynamic(Box::new(expr)),
            None => DeepMode::Const(false),
        };

        Ok(MergeFn { to, from, deep }.as_expr())
    }
}

#[derive(Debug, Clone)]
enum DeepMode {
    Const(bool),
    Dynamic(Box<dyn Expression>),
}

#[derive(Debug, Clone)]
pub(crate) struct MergeFn {
    to: Box<dyn Expression>,
    from: Box<dyn Expression>,
    deep: DeepMode,
}

impl FunctionExpression for MergeFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let to_value = self.to.resolve(ctx)?.try_object()?;
        let from_value = self.from.resolve(ctx)?.try_object()?;
        let deep = match &self.deep {
            DeepMode::Const(b) => *b,
            DeepMode::Dynamic(expr) => expr.resolve(ctx)?.try_boolean()?,
        };

        Ok(merge_maps(to_value, from_value, deep).into())
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let to = self.to.type_def(state).restrict_object();
        let from = self.from.type_def(state).restrict_object();

        match &self.deep {
            DeepMode::Const(false) => to.merge_overwrite(from),
            DeepMode::Const(true) => deep_merge_type_def(to, from),
            DeepMode::Dynamic(_) => {
                let shallow = to.clone().merge_overwrite(from.clone());
                let deep = deep_merge_type_def(to, from);
                shallow.union(deep)
            }
        }
    }
}

fn deep_merge_type_def(to: TypeDef, from: TypeDef) -> TypeDef {
    let to_kind: Kind = to.clone().into();
    let from_kind: Kind = from.clone().into();
    let merged_kind = deep_merge_kind(&to_kind, &from_kind);
    to.merge_overwrite(from).with_kind(merged_kind)
}

fn deep_merge_kind(to: &Kind, from: &Kind) -> Kind {
    let mut result = to.clone();
    result.merge_keep(from.clone(), true);

    let (Some(to_col), Some(from_col), Some(res_col)) =
        (to.as_object(), from.as_object(), result.as_object_mut())
    else {
        return result;
    };

    for (key, from_child) in from_col.known() {
        let Some(to_child) = to_col.known().get(key) else {
            continue;
        };
        if !to_child.contains_object() || !from_child.contains_object() {
            continue;
        }

        let to_obj = Kind::object(
            to_child
                .as_object()
                .cloned()
                .unwrap_or_else(Collection::empty),
        );
        let from_obj = Kind::object(
            from_child
                .as_object()
                .cloned()
                .unwrap_or_else(Collection::empty),
        );
        let merged_obj = deep_merge_kind(&to_obj, &from_obj);
        let from_non_obj = from_child.without_object();
        let mut final_child = if from_non_obj.is_never() {
            merged_obj
        } else {
            merged_obj.union(from_non_obj)
        };

        if to_child
            .without_object()
            .without_undefined()
            .contains_any_defined()
        {
            final_child = final_child.union(from_obj);
        }
        if to_child.contains_undefined() {
            final_child = final_child.union(from_child.clone().without_undefined());
        }
        if from_child.contains_undefined() {
            final_child = final_child.union(to_child.clone().without_undefined());
        }
        if to_child.contains_undefined() && from_child.contains_undefined() {
            final_child.add_undefined();
        }

        res_col.known_mut().insert(key.clone(), final_child);
    }

    result
}

/// Merges two `ObjectMap`s of Values. The second map is merged into the first one.
///
/// If `deep` is true, objects are merged recursively if a key exists in both objects
/// and both values are also objects.
///
/// If `deep` is false, only top-level fields are merged; values from `map2` overwrite
/// values from `map1`.
fn merge_maps(map1: ObjectMap, map2: ObjectMap, deep: bool) -> ObjectMap {
    if map2.is_empty() {
        return map1;
    }
    if map1.is_empty() {
        return map2;
    }

    if deep {
        deep_merge_maps(map1, map2)
    } else if map1.len() < map2.len() {
        let mut result = map2;
        for (key1, value1) in map1 {
            result.entry(key1).or_insert(value1);
        }
        result
    } else {
        let mut result = map1;
        for (key2, value2) in map2 {
            result.insert(key2, value2);
        }
        result
    }
}

struct MergeFrame {
    parent_key: Option<KeyString>,
    target: ObjectMap,
    incoming: btree_map::IntoIter<KeyString, Value>,
}

fn deep_merge_maps(map1: ObjectMap, map2: ObjectMap) -> ObjectMap {
    let mut stack = vec![MergeFrame {
        parent_key: None,
        target: map1,
        incoming: map2.into_iter(),
    }];

    'outer: while let Some(mut frame) = stack.pop() {
        while let Some((key, value2)) = frame.incoming.next() {
            match frame.target.entry(key) {
                Entry::Occupied(occ) => match (occ.get(), &value2) {
                    (Value::Object(_), Value::Object(_)) => {
                        let (child_key, old_val) = occ.remove_entry();
                        let child1 = match old_val {
                            Value::Object(o) => o,
                            _ => BTreeMap::new(),
                        };
                        let child2 = match value2 {
                            Value::Object(o) => o,
                            _ => BTreeMap::new(),
                        };
                        stack.push(frame);
                        stack.push(MergeFrame {
                            parent_key: Some(child_key),
                            target: child1,
                            incoming: child2.into_iter(),
                        });
                        continue 'outer;
                    }
                    _ => {
                        *occ.into_mut() = value2;
                    }
                },
                Entry::Vacant(entry) => {
                    entry.insert(value2);
                }
            }
        }

        match frame.parent_key {
            Some(parent_key) => {
                if let Some(parent) = stack.last_mut() {
                    parent
                        .target
                        .insert(parent_key, Value::Object(frame.target));
                }
            }
            None => {
                return frame.target;
            }
        }
    }

    BTreeMap::new()
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
                    Field::from("grandchild1") => Kind::bytes(),
                    Field::from("grandchild2") => Kind::boolean(),
                }),
            }),
        }

        deep_constant_false {
            args: func_args![
                to: value!({ key1: { sub1: "val1" } }),
                from: value!({ key1: { sub2: "val2" } }),
                deep: false,
            ],
            want: Ok(value!({ key1: { sub2: "val2" } })),
            tdef: TypeDef::object(btreemap! {
                Field::from("key1") => TypeDef::object(btreemap! {
                    Field::from("sub2") => Kind::bytes(),
                }),
            }),
        }

        deep_constant_true {
            args: func_args![
                to: value!({ key1: { sub1: "val1" } }),
                from: value!({ key1: { sub2: "val2" } }),
                deep: true,
            ],
            want: Ok(value!({
                key1: {
                    sub1: "val1",
                    sub2: "val2",
                },
            })),
            tdef: TypeDef::object(btreemap! {
                Field::from("key1") => TypeDef::object(btreemap! {
                    Field::from("sub1") => Kind::bytes(),
                    Field::from("sub2") => Kind::bytes(),
                }),
            }),
        }

        empty_both {
            args: func_args![
                to: value!({}),
                from: value!({}),
            ],
            want: Ok(value!({})),
            tdef: TypeDef::object(btreemap! {}),
        }

        empty_to {
            args: func_args![
                to: value!({}),
                from: value!({ a: 1 }),
            ],
            want: Ok(value!({ a: 1 })),
            tdef: TypeDef::object(btreemap! {
                Field::from("a") => Kind::integer(),
            }),
        }

        empty_from {
            args: func_args![
                to: value!({ a: 1 }),
                from: value!({}),
            ],
            want: Ok(value!({ a: 1 })),
            tdef: TypeDef::object(btreemap! {
                Field::from("a") => Kind::integer(),
            }),
        }

        asymmetric_small_to_large {
            args: func_args![
                to: value!({ a: 1, b: 2 }),
                from: value!({ b: 20, c: 30, d: 40 }),
            ],
            want: Ok(value!({ a: 1, b: 20, c: 30, d: 40 })),
            tdef: TypeDef::object(btreemap! {
                Field::from("a") => Kind::integer(),
                Field::from("b") => Kind::integer(),
                Field::from("c") => Kind::integer(),
                Field::from("d") => Kind::integer(),
            }),
        }

        asymmetric_large_to_small {
            args: func_args![
                to: value!({ a: 1, b: 2, c: 3, d: 4 }),
                from: value!({ a: 10 }),
            ],
            want: Ok(value!({ a: 10, b: 2, c: 3, d: 4 })),
            tdef: TypeDef::object(btreemap! {
                Field::from("a") => Kind::integer(),
                Field::from("b") => Kind::integer(),
                Field::from("c") => Kind::integer(),
                Field::from("d") => Kind::integer(),
            }),
        }

        deep_scalar_overwrites_object {
            args: func_args![
                to: value!({ a: { nested: 1 } }),
                from: value!({ a: "replaced" }),
                deep: true,
            ],
            want: Ok(value!({ a: "replaced" })),
            tdef: TypeDef::object(btreemap! {
                Field::from("a") => Kind::bytes(),
            }),
        }

        deep_object_overwrites_scalar {
            args: func_args![
                to: value!({ a: "original" }),
                from: value!({ a: { nested: 2 } }),
                deep: true,
            ],
            want: Ok(value!({ a: { nested: 2 } })),
            tdef: TypeDef::object(btreemap! {
                Field::from("a") => Kind::object(btreemap! {
                    Field::from("nested") => Kind::integer(),
                }),
            }),
        }

        deep_three_levels {
            args: func_args![
                to: value!({
                    l1: {
                        l2: {
                            a: 1,
                            b: 2,
                        },
                        keep: "yes",
                    }
                }),
                from: value!({
                    l1: {
                        l2: {
                            b: 20,
                            c: 30,
                        },
                        new: "val",
                    }
                }),
                deep: true,
            ],
            want: Ok(value!({
                l1: {
                    l2: {
                        a: 1,
                        b: 20,
                        c: 30,
                    },
                    keep: "yes",
                    new: "val",
                }
            })),
            tdef: TypeDef::object(btreemap! {
                Field::from("l1") => TypeDef::object(btreemap! {
                    Field::from("l2") => TypeDef::object(btreemap! {
                        Field::from("a") => Kind::integer(),
                        Field::from("b") => Kind::integer(),
                        Field::from("c") => Kind::integer(),
                    }),
                    Field::from("keep") => Kind::bytes(),
                    Field::from("new") => Kind::bytes(),
                }),
            }),
        }
    ];

    #[test]
    fn dynamic_deep_resolution() {
        let fns = vec![Box::new(Merge) as Box<dyn crate::compiler::Function>];
        let src = r#"merge({"key1": {"sub1": "val1"}}, {"key1": {"sub2": "val2"}}, .flag == true)"#;
        let prog = crate::compiler::compile(src, &fns).expect("compiles");

        // dynamic true
        let mut target_true = value!({ flag: true });
        let mut state = crate::compiler::state::RuntimeState::default();
        let tz = crate::compiler::TimeZone::default();
        let mut ctx = crate::compiler::Context::new(&mut target_true, &mut state, &tz);
        let res_true = prog.program.resolve(&mut ctx);
        assert_eq!(
            res_true,
            Ok(value!({
                key1: {
                    sub1: "val1",
                    sub2: "val2",
                }
            }))
        );

        // dynamic false
        let mut target_false = value!({ flag: false });
        let mut state = crate::compiler::state::RuntimeState::default();
        let mut ctx = crate::compiler::Context::new(&mut target_false, &mut state, &tz);
        let res_false = prog.program.resolve(&mut ctx);
        assert_eq!(
            res_false,
            Ok(value!({
                key1: {
                    sub2: "val2",
                }
            }))
        );
    }

    #[test]
    fn deep_nesting_10_000_levels() {
        let depth = 10_000;
        let mut map1 = Value::from("leaf1");
        let mut map2 = Value::from("leaf2");

        for i in (0..depth).rev() {
            let k: KeyString = format!("k{i}").into();
            map1 = Value::Object(btreemap! { k.clone() => map1 });
            map2 = Value::Object(btreemap! { k => map2 });
        }

        let o1 = map1.try_object().expect("object");
        let o2 = map2.try_object().expect("object");

        let result = merge_maps(o1, o2, true);

        let mut cur = result;
        for i in 0..depth {
            let k = format!("k{i}");
            if i == depth - 1 {
                assert_eq!(cur.get(k.as_str()), Some(&Value::from("leaf2")));
            } else {
                let next = cur.remove(k.as_str()).expect("key exists");
                cur = next.try_object().expect("object");
            }
        }
    }

    #[test]
    fn deep_nesting_10_000_levels_preserves_sibling_leaf() {
        let depth = 10_000;
        let mut map1 = Value::Object(btreemap! {
            KeyString::from("leaf_target") => Value::from("leaf1"),
            KeyString::from("leaf_keep") => Value::from("preserved"),
        });
        let mut map2 = Value::Object(btreemap! {
            KeyString::from("leaf_target") => Value::from("leaf2"),
        });

        for i in (0..depth).rev() {
            let k: KeyString = format!("k{i}").into();
            map1 = Value::Object(btreemap! { k.clone() => map1 });
            map2 = Value::Object(btreemap! { k => map2 });
        }

        let o1 = map1.try_object().expect("object");
        let o2 = map2.try_object().expect("object");

        let result = merge_maps(o1, o2, true);

        let mut cur = result;
        for i in 0..depth {
            let k = format!("k{i}");
            let next = cur.remove(k.as_str()).expect("key exists");
            cur = next.try_object().expect("object");
        }
        assert_eq!(cur.get("leaf_target"), Some(&Value::from("leaf2")));
        assert_eq!(cur.get("leaf_keep"), Some(&Value::from("preserved")));
    }

    #[test]
    fn deep_merge_compiler_field_access() {
        let fns = vec![Box::new(Merge) as Box<dyn crate::compiler::Function>];
        let src = r#"
            res = merge({"key": {"left": 1}}, {"key": {"right": 2}}, deep: true)
            res.key.left
        "#;
        let prog = crate::compiler::compile(src, &fns).expect("compiles successfully");
        let mut target = value!({});
        let mut state = crate::compiler::state::RuntimeState::default();
        let tz = crate::compiler::TimeZone::default();
        let mut ctx = crate::compiler::Context::new(&mut target, &mut state, &tz);
        let val = prog.program.resolve(&mut ctx).expect("resolves");
        assert_eq!(val, Value::from(1));
    }

    #[test]
    fn deep_merge_dynamic_compiler_field_access() {
        let fns = vec![Box::new(Merge) as Box<dyn crate::compiler::Function>];
        let src = r#"
            res = merge({"key": {"left": 1}}, {"key": {"right": 2}}, .flag == true)
            res.key.left
        "#;
        let prog = crate::compiler::compile(src, &fns).expect("compiles successfully");
        let mut target = value!({ flag: true });
        let mut state = crate::compiler::state::RuntimeState::default();
        let tz = crate::compiler::TimeZone::default();
        let mut ctx = crate::compiler::Context::new(&mut target, &mut state, &tz);
        let val = prog.program.resolve(&mut ctx).expect("resolves");
        assert_eq!(val, Value::from(1));
    }

    #[test]
    fn pront_example_1_stale_deep_flag() {
        let fns = vec![Box::new(Merge) as Box<dyn crate::compiler::Function>];
        let src = indoc! {r#"
            flag = false
            . = merge(
              { flag = true; {"k": {"a": 1}} },
              {"k": {"b": 2}},
              deep: flag
            )
        "#};
        let prog = crate::compiler::compile(src, &fns).expect("compiles successfully");
        let mut target = value!({});
        let mut state = crate::compiler::state::RuntimeState::default();
        let tz = crate::compiler::TimeZone::default();
        let mut ctx = crate::compiler::Context::new(&mut target, &mut state, &tz);
        let _ = prog.program.resolve(&mut ctx).expect("resolves");
        assert_eq!(
            target,
            value!({
                k: {
                    a: 1,
                    b: 2,
                }
            })
        );
    }

    #[test]
    fn pront_example_2_guaranteed_string() {
        let fns = vec![
            Box::new(Merge) as Box<dyn crate::compiler::Function>,
            Box::new(crate::stdlib::Upcase) as Box<dyn crate::compiler::Function>,
        ];
        let src = indoc! {r#"
            res = merge({}, {"a": "hello"}, deep: true)
            .result = upcase(res.a)
        "#};
        let prog = crate::compiler::compile(src, &fns).expect("compiles successfully");
        let mut target = value!({});
        let mut state = crate::compiler::state::RuntimeState::default();
        let tz = crate::compiler::TimeZone::default();
        let mut ctx = crate::compiler::Context::new(&mut target, &mut state, &tz);
        let _ = prog.program.resolve(&mut ctx).expect("resolves");
        assert_eq!(target, value!({ result: "HELLO" }));
    }

    #[test]
    fn dynamic_deep_guaranteed_string() {
        let fns = vec![
            Box::new(Merge) as Box<dyn crate::compiler::Function>,
            Box::new(crate::stdlib::Upcase) as Box<dyn crate::compiler::Function>,
        ];
        let src = indoc! {r#"
            res = merge({}, {"a": "hello"}, deep: .flag == true)
            .result = upcase(res.a)
        "#};
        let prog = crate::compiler::compile(src, &fns).expect("compiles successfully");
        let mut target = value!({ flag: true });
        let mut state = crate::compiler::state::RuntimeState::default();
        let tz = crate::compiler::TimeZone::default();
        let mut ctx = crate::compiler::Context::new(&mut target, &mut state, &tz);
        let _ = prog.program.resolve(&mut ctx).expect("resolves");
        assert_eq!(target, value!({ flag: true, result: "HELLO" }));
    }

    #[test]
    fn deep_merge_nested_guaranteed_strings() {
        let fns = vec![
            Box::new(Merge) as Box<dyn crate::compiler::Function>,
            Box::new(crate::stdlib::Upcase) as Box<dyn crate::compiler::Function>,
        ];
        let src = indoc! {r#"
            res = merge({"k": {"a": "foo"}}, {"k": {"b": "bar"}}, deep: true)
            .res_a = upcase(res.k.a)
            .res_b = upcase(res.k.b)
        "#};
        let prog = crate::compiler::compile(src, &fns).expect("compiles successfully");
        let mut target = value!({});
        let mut state = crate::compiler::state::RuntimeState::default();
        let tz = crate::compiler::TimeZone::default();
        let mut ctx = crate::compiler::Context::new(&mut target, &mut state, &tz);
        let _ = prog.program.resolve(&mut ctx).expect("resolves");
        assert_eq!(target, value!({ res_a: "FOO", res_b: "BAR" }));
    }

    #[test]
    fn deep_merge_rejects_to_only_field_when_to_may_not_be_object() {
        let fns = vec![
            Box::new(Merge) as Box<dyn crate::compiler::Function>,
            Box::new(crate::stdlib::Upcase) as Box<dyn crate::compiler::Function>,
        ];
        let src = indoc! {r#"
            input = if .flag == true {
                {"k": {"a": "foo"}}
            } else {
                {"k": "not-object"}
            }
            res = merge(input, {"k": {"b": "bar"}}, deep: true)
            .result = upcase(res.k.a)
        "#};

        assert!(
            crate::compiler::compile(src, &fns).is_err(),
            "res.k.a is only guaranteed when input.k is an object"
        );
    }
}
