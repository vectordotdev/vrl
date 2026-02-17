use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_COMPACT: LazyLock<Value> = LazyLock::new(|| Value::Boolean(false));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "target",
            kind: kind::ANY,
            required: true,
            description: "The path of the field to delete",
            default: None,
            enum_variants: None,
        },
        Parameter {
            keyword: "compact",
            kind: kind::BOOLEAN,
            required: false,
            description:
                "After deletion, if `compact` is `true` and there is an empty object or array left,
the empty object or array is also removed, cascading up to the root. This only
applies to the path being deleted, and any parent paths.",
            default: Some(&DEFAULT_COMPACT),
            enum_variants: None,
        },
    ]
});

#[inline]
fn del(query: &expression::Query, compact: bool, ctx: &mut Context) -> Resolved {
    let path = query.path();

    if let Some(target_path) = query.external_path() {
        Ok(ctx
            .target_mut()
            .target_remove(&target_path, compact)
            .ok()
            .flatten()
            .unwrap_or(Value::Null))
    } else if let Some(ident) = query.variable_ident() {
        match ctx.state_mut().variable_mut(ident) {
            Some(value) => {
                let new_value = value.get(path).cloned();
                value.remove(path, compact);
                Ok(new_value.unwrap_or(Value::Null))
            }
            None => Ok(Value::Null),
        }
    } else if let Some(expr) = query.expression_target() {
        let value = expr.resolve(ctx)?;

        // No need to do the actual deletion, as the expression is only
        // available as an argument to the function.
        Ok(value.get(path).cloned().unwrap_or(Value::Null))
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

    fn usage(&self) -> &'static str {
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
        PARAMETERS.as_slice()
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
        _state: &state::TypeState,
        ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let query = arguments.required_query("target")?;
        let compact = arguments.optional("compact");

        if let Some(target_path) = query.external_path()
            && ctx.is_read_only_path(&target_path)
        {
            return Err(function::Error::ReadOnlyMutation {
                context: format!("{query} is read-only, and cannot be deleted"),
            }
            .into());
        }

        Ok(Box::new(DelFn { query, compact }))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct DelFn {
    query: expression::Query,
    compact: Option<Box<dyn Expression>>,
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
            compact: None,
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
        let compact = self
            .compact
            .map_resolve_with_default(ctx, || DEFAULT_COMPACT.clone())?
            .try_boolean()?;
        del(&self.query, compact, ctx)
    }

    fn type_info(&self, state: &state::TypeState) -> TypeInfo {
        let mut state = state.clone();

        let return_type = self.query.apply_type_info(&mut state).impure();

        let compact: Option<bool> = self
            .compact
            .as_ref()
            .and_then(|compact| compact.resolve_constant(&state))
            .and_then(|compact| compact.as_boolean());

        if let Some(compact) = compact {
            self.query.delete_type_def(&mut state.external, compact);
        } else {
            let mut false_result = state.external.clone();
            self.query.delete_type_def(&mut false_result, false);

            let mut true_result = state.external.clone();
            self.query.delete_type_def(&mut true_result, true);

            state.external = false_result.merge(true_result);
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
    use super::*;
    use crate::btreemap;
    use crate::value;

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
}
