use crate::compiler::prelude::*;

static DEFAULT_COMPACT: Value = Value::Boolean(false);

const PARAMETERS: &[Parameter] = &[
    Parameter::required("target", kind::ANY, "The path of the field to delete"),
    Parameter::optional(
        "compact",
        kind::BOOLEAN,
        "After deletion, if `compact` is `true` and there is an empty object or array left,
the empty object or array is also removed, cascading up to the root. This only
applies to the path being deleted, and any parent paths.",
    )
    .default(&DEFAULT_COMPACT),
];

#[inline]
fn del(query: &expression::Query, compact: bool, ctx: &mut Context) -> Resolved {
    let path = query.path();

    if let Some(target_path) = query.external_path() {
        Ok(ctx
            .target_mut()
            .target_remove(target_path, compact)
            .ok()
            .flatten()
            .unwrap_or(Value::Null))
    } else if let Some(ident) = query.variable_ident() {
        Ok(ctx
            .state_mut()
            .variable_mut(ident)
            .and_then(|value| value.remove(path, compact))
            .unwrap_or(Value::Null))
    } else if let Some(expr) = query.expression_target() {
        let mut value = expr.resolve(ctx)?;

        // Move the target out of the temporary owned value without cloning.
        Ok(value.remove(path, false).unwrap_or(Value::Null))
    } else {
        Ok(Value::Null)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Del;

impl Function for Del {
    fn identifier(&self) -> &'static str {
        "del"
    }

    fn description(&self) -> &'static str {
        indoc! {"
            Removes the field specified by the static `path` from the target.

            For dynamic path deletion, see the `remove` function.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Path.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::ANY
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns the value of the field being deleted. Returns `null` if the field doesn't exist.",
        ]
    }

    fn notices(&self) -> &'static [&'static str] {
        &[
            "The `del` function _modifies the current event in place_ and returns the value of the deleted field.",
        ]
    }

    fn pure(&self) -> bool {
        false
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Delete a field",
                source: indoc! {r#"
                    . = { "foo": "bar" }
                    del(.foo)
                "#},
                result: Ok("bar"),
            },
            example! {
                title: "Rename a field",
                source: indoc! {r#"
                    . = { "old": "foo" }
                    .new = del(.old)
                    .
                "#},
                result: Ok(r#"{ "new": "foo" }"#),
            },
            example! {
                title: "Returns null for unknown field",
                source: r#"del({"foo": "bar"}.baz)"#,
                result: Ok("null"),
            },
            example! {
                title: "External target",
                source: indoc! {r#"
                    . = { "foo": true, "bar": 10 }
                    del(.foo)
                    .
                "#},
                result: Ok(r#"{ "bar": 10 }"#),
            },
            example! {
                title: "Delete field from variable",
                source: indoc! {r#"
                    var = { "foo": true, "bar": 10 }
                    del(var.foo)
                    var
                "#},
                result: Ok(r#"{ "bar": 10 }"#),
            },
            example! {
                title: "Delete object field",
                source: indoc! {r#"
                    var = { "foo": {"nested": true}, "bar": 10 }
                    del(var.foo.nested, false)
                    var
                "#},
                result: Ok(r#"{ "foo": {}, "bar": 10 }"#),
            },
            example! {
                title: "Compact object field",
                source: indoc! {r#"
                    var = { "foo": {"nested": true}, "bar": 10 }
                    del(var.foo.nested, true)
                    var
                "#},
                result: Ok(r#"{ "bar": 10 }"#),
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let query = arguments.required_query("target")?;

        if let Some(target_path) = query.external_path()
            && ctx.is_read_only_path(target_path)
        {
            return Err(function::Error::ReadOnlyMutation {
                context: format!("{query} is read-only, and cannot be deleted"),
            }
            .into());
        }

        let compact = match arguments.optional("compact") {
            None => ConstOrExpr::Const(false),
            Some(expr) => match expr.resolve_constant(state) {
                Some(Value::Boolean(b)) => ConstOrExpr::Const(b),
                _ => ConstOrExpr::Expr(expr),
            },
        };

        Ok(Box::new(DelFn { query, compact }))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DelFn {
    query: expression::Query,
    compact: ConstOrExpr<bool>,
}

impl DelFn {
    #[cfg(test)]
    fn new(path: &str) -> Self {
        use crate::path::{PathPrefix, parse_value_path};

        Self {
            query: expression::Query::new(
                expression::Target::External(PathPrefix::Event),
                parse_value_path(path).unwrap(),
            ),
            compact: ConstOrExpr::Const(false),
        }
    }
}

impl Expression for DelFn {
    // TODO: we're silencing the result of the `remove` call here, to make this
    // function infallible.
    //
    // This isn't correct though, since, while deleting Vector log fields is
    // infallible, deleting metric fields is not.
    //
    // For example, if you try to delete `.name` in a metric event, the call
    // returns an error, since this is an immutable field.
    //
    // After some debating, we've decided to _silently ignore_ deletions of
    // immutable fields for now, but we'll circle back to this in the near
    // future to potentially improve this situation.
    //
    // see tracking issue: https://github.com/vectordotdev/vector/issues/5887
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let compact = match &self.compact {
            ConstOrExpr::Const(compact) => *compact,
            ConstOrExpr::Expr(expr) => expr.resolve(ctx)?.try_boolean()?,
        };
        del(&self.query, compact, ctx)
    }

    fn type_info(&self, state: &state::TypeState) -> TypeInfo {
        let mut state = state.clone();

        let return_type = self.query.apply_type_info(&mut state).impure();

        match &self.compact {
            ConstOrExpr::Const(compact) => {
                self.query.delete_type_def(&mut state.external, *compact);
            }
            ConstOrExpr::Expr(expr) => {
                if let Some(compact) = expr
                    .resolve_constant(&state)
                    .and_then(|compact| compact.as_boolean())
                {
                    self.query.delete_type_def(&mut state.external, compact);
                } else {
                    let mut false_result = state.external.clone();
                    self.query.delete_type_def(&mut false_result, false);

                    let mut true_result = state.external.clone();
                    self.query.delete_type_def(&mut true_result, true);

                    state.external = false_result.merge(true_result);
                }
            }
        }

        TypeInfo::new(state, return_type)
    }
}

impl fmt::Display for DelFn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::btreemap;
    use crate::compiler::TargetValue;
    use crate::value;
    use crate::value::Secrets;

    #[test]
    fn del() {
        let cases = vec![
            (
                // String field exists
                btreemap! { "exists" => "value" },
                Ok(value!("value")),
                DelFn::new("exists"),
            ),
            (
                // String field doesn't exist
                btreemap! { "exists" => "value" },
                Ok(value!(null)),
                DelFn::new("does_not_exist"),
            ),
            (
                // Array field exists
                btreemap! { "exists" => value!([1, 2, 3]) },
                Ok(value!([1, 2, 3])),
                DelFn::new("exists"),
            ),
            (
                // Null field exists
                btreemap! { "exists" => value!(null) },
                Ok(value!(null)),
                DelFn::new("exists"),
            ),
            (
                // Map field exists
                btreemap! {"exists" => btreemap! { "foo" => "bar" }},
                Ok(value!(btreemap! {"foo" => "bar" })),
                DelFn::new("exists"),
            ),
            (
                // Integer field exists
                btreemap! { "exists" => 127 },
                Ok(value!(127)),
                DelFn::new("exists"),
            ),
            (
                // Array field exists
                btreemap! {"exists" => value!([1, 2, 3]) },
                Ok(value!(2)),
                DelFn::new(".exists[1]"),
            ),
        ];
        let tz = TimeZone::default();
        for (object, exp, func) in cases {
            let mut object: Value = object.into();
            let mut runtime_state = state::RuntimeState::default();
            let mut ctx = Context::new(&mut object, &mut runtime_state, &tz);
            let got = func
                .resolve(&mut ctx)
                .map_err(|e| format!("{:#}", anyhow::anyhow!(e)));
            assert_eq!(got, exp);
        }
    }

    fn eval_vrl(source: &str, event: Value) -> Result<Value, String> {
        let fns = crate::stdlib::all();
        let compiled =
            crate::compiler::compile(source, &fns).map_err(|diags| format!("{diags:?}"))?;
        let mut target = TargetValue {
            value: event,
            metadata: Value::Object(ObjectMap::new()),
            secrets: Secrets::default(),
        };
        let mut runtime_state = state::RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut runtime_state, &tz);
        compiled
            .program
            .resolve(&mut ctx)
            .map_err(|e| e.to_string())
    }

    #[test]
    fn del_variable_field_and_root() {
        use crate::compiler::expression::{Query, Target, Variable};
        use crate::compiler::parser::ast::Ident;
        use crate::path::OwnedValuePath;

        assert_eq!(
            eval_vrl(
                r#"var = {"a": {"b": 1}, "c": 2}; r = del(var.a); [r, var]"#,
                value!({})
            ),
            Ok(value!([{"b": 1}, {"c": 2}]))
        );

        // Root-path Query on an internal variable (object, array, and scalar).
        let ident = Ident::new("var");
        let mut local_env = state::LocalEnv::default();
        local_env.insert_variable(
            ident.clone(),
            crate::compiler::type_def::Details {
                type_def: TypeDef::any(),
                value: None,
            },
        );
        let query = Query::new(
            Target::Internal(
                Variable::new(
                    crate::diagnostic::Span::default(),
                    ident.clone(),
                    &local_env,
                )
                .unwrap(),
            ),
            OwnedValuePath::root(),
        );
        let func = DelFn {
            query,
            compact: ConstOrExpr::Const(false),
        };
        let tz = TimeZone::default();

        for (initial, expected_return, expected_remaining) in [
            (value!({"a": 1}), value!({"a": 1}), value!({})),
            (value!([1, 2]), value!([1, 2]), value!([])),
            (value!(42), value!(42), value!(null)),
        ] {
            let mut target = TargetValue {
                value: value!({}),
                metadata: Value::Object(ObjectMap::new()),
                secrets: Secrets::default(),
            };
            let mut runtime_state = state::RuntimeState::default();
            runtime_state.insert_variable(ident.clone(), initial);
            let mut ctx = Context::new(&mut target, &mut runtime_state, &tz);

            let ret = func.resolve(&mut ctx).unwrap();
            assert_eq!(ret, expected_return);
            assert_eq!(runtime_state.variable(&ident), Some(&expected_remaining));
        }
    }

    #[test]
    fn del_expression_target() {
        use crate::compiler::expression::{Container, Literal, Object, Query, Target};
        use crate::path::OwnedValuePath;

        assert_eq!(
            eval_vrl(r#"del({"a": {"b": 1}, "c": 2}.a)"#, value!({})),
            Ok(value!({"b": 1}))
        );
        assert_eq!(
            eval_vrl(r#"del(merge({"a": 1}, {"b": 2}).b)"#, value!({})),
            Ok(value!(2))
        );

        // Root-path Query on a Container expression target.
        let container = Container::new(crate::compiler::expression::Variant::Object(Object::from(
            BTreeMap::from([(KeyString::from("a"), Literal::Integer(1).into())]),
        )));
        let func = DelFn {
            query: Query::new(Target::Container(container), OwnedValuePath::root()),
            compact: ConstOrExpr::Const(false),
        };
        let mut target = TargetValue {
            value: value!({}),
            metadata: Value::Object(ObjectMap::new()),
            secrets: Secrets::default(),
        };
        let mut runtime_state = state::RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut target, &mut runtime_state, &tz);
        assert_eq!(func.resolve(&mut ctx).unwrap(), value!({"a": 1}));
    }

    #[test]
    fn del_compact_and_arrays() {
        assert_eq!(
            eval_vrl(
                r#"var = {"a": {"b": 1}, "c": 2}; del(var.a.b, true); var"#,
                value!({})
            ),
            Ok(value!({"c": 2}))
        );
        assert_eq!(
            eval_vrl(
                r#"var = {"a": {"b": 1}, "c": 2}; del(var.a.b, false); var"#,
                value!({})
            ),
            Ok(value!({"a": {}, "c": 2}))
        );
        assert_eq!(
            eval_vrl("var = [10, 20, 30]; r = del(var[-1]); [r, var]", value!({})),
            Ok(value!([30, [10, 20]]))
        );
        assert_eq!(
            eval_vrl("var = [10, 20]; r = del(var[-5]); [r, var]", value!({})),
            Ok(value!([null, [10, 20]]))
        );
        assert_eq!(
            eval_vrl(r#"var = {"a": [1]}; del(var.a[0], true); var"#, value!({})),
            Ok(value!({}))
        );
        assert_eq!(
            eval_vrl(
                r#"var = {"a": {}}; r = del(var.a.missing, true); [r, var]"#,
                value!({})
            ),
            Ok(value!([null, {"a": {}}]))
        );
    }

    #[test]
    fn del_read_only_path_rejected() {
        use crate::compiler::{CompileConfig, compile_with_state};
        use crate::path::parse_target_path;

        let fns = crate::stdlib::all();
        let state = state::TypeState::default();
        let mut config = CompileConfig::default();
        config.set_read_only_path(parse_target_path(".ro").unwrap(), true);

        let err = compile_with_state("del(.ro)", &fns, &state, config)
            .err()
            .expect("expected read-only path error");
        let msg = format!("{err:?}");
        assert!(
            msg.contains("is read-only, and cannot be deleted"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn del_dynamic_compact_side_effects_and_type_error() {
        // Dynamic compact with side effects mutating the same variable must run before del.
        assert_eq!(
            eval_vrl(
                r#"var = {"a": 1}; r = del(var.a, { var.a = 2; false }); [r, var]"#,
                value!({})
            ),
            Ok(value!([2, {}]))
        );

        // Non-boolean compact is rejected by VRL's type checker at compile time...
        let err = eval_vrl("del(.a, .flag)", value!({"a": 1, "flag": "not_a_bool"}))
            .expect_err("expected compile-time type error for non-boolean compact");
        assert!(
            err.contains("invalid argument type"),
            "unexpected error: {err}"
        );

        // ...and DelFn::resolve itself errors on a non-boolean runtime compact expression.
        let mut func = DelFn::new(".a");
        func.compact = ConstOrExpr::Expr(Box::new(crate::compiler::expression::Literal::from(
            "not_a_bool",
        )));
        let mut object: Value = btreemap! { "a" => 1 }.into();
        let mut runtime_state = state::RuntimeState::default();
        let tz = TimeZone::default();
        let mut ctx = Context::new(&mut object, &mut runtime_state, &tz);
        let err = func
            .resolve(&mut ctx)
            .expect_err("expected runtime boolean error")
            .to_string();
        assert!(
            err.contains("expected boolean"),
            "unexpected runtime error: {err}"
        );
    }

    #[test]
    fn del_type_info_omitted_compact_is_false() {
        use crate::path::parse_value_path;

        let func = DelFn::new(".a.b");
        let mut state = state::TypeState::default();
        let inner = Kind::object(Collection::from(BTreeMap::from([(
            "b".into(),
            Kind::integer(),
        )])));
        let root = Kind::object(Collection::from(BTreeMap::from([("a".into(), inner)])));
        state
            .external
            .update_target(crate::compiler::type_def::Details {
                type_def: TypeDef::from(root),
                value: None,
            });

        let info = func.type_info(&state);
        let a_kind = info
            .state
            .external
            .target_kind()
            .at_path(&parse_value_path(".a").unwrap());
        assert!(
            a_kind.is_object() && !a_kind.contains_undefined(),
            "expected .a to remain a definite object when compact is omitted, got {a_kind:?}"
        );
    }
}
