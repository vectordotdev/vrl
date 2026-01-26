use crate::compiler::prelude::*;
use crate::path::{OwnedSegment, OwnedValuePath};
use std::sync::LazyLock;

static DEFAULT_COMPACT: LazyLock<Value> = LazyLock::new(|| Value::Boolean(false));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::OBJECT | kind::ARRAY,
            required: true,
            description: "The object or array to remove data from.",
            default: None,
        },
        Parameter {
            keyword: "path",
            kind: kind::ARRAY,
            required: true,
            description: "An array of path segments to remove the value from.",
            default: None,
        },
        Parameter {
            keyword: "compact",
            kind: kind::BOOLEAN,
            required: false,
            description: "After deletion, if `compact` is `true`, any empty objects or
arrays left are also removed.",
            default: Some(&DEFAULT_COMPACT),
        },
    ]
});

fn remove(path: Value, compact: Value, mut value: Value) -> Resolved {
    let path = match path {
        Value::Array(path) => {
            let mut lookup = OwnedValuePath::root();

            for segment in path {
                let segment = match segment {
                    Value::Bytes(field) => {
                        OwnedSegment::Field(String::from_utf8_lossy(&field).into())
                    }
                    #[allow(clippy::cast_possible_truncation)] //TODO evaluate removal options
                    Value::Integer(index) => OwnedSegment::Index(index as isize),
                    value => {
                        return Err(format!(
                            "path segment must be either string or integer, not {}",
                            value.kind()
                        )
                        .into());
                    }
                };

                lookup.segments.push(segment);
            }

            lookup
        }
        value => {
            return Err(ValueError::Expected {
                got: value.kind(),
                expected: Kind::array(Collection::any()),
            }
            .into());
        }
    };
    let compact = compact.try_boolean()?;
    value.remove(&path, compact);
    Ok(value)
}

#[derive(Clone, Copy, Debug)]
pub struct Remove;

impl Function for Remove {
    fn identifier(&self) -> &'static str {
        "remove"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Dynamically remove the value for a given path.

            If you know the path you want to remove, use
            the `del` function and static paths such as `del(.foo.bar[1])`
            to remove the value at that path. The `del` function returns the
            deleted value, and is more performant than `remove`.
            However, if you do not know the path names, use the dynamic
            `remove` function to remove the value at the provided path.
        "}
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Single-segment top-level field",
                source: r#"remove!(value: { "foo": "bar" }, path: ["foo"])"#,
                result: Ok("{}"),
            },
            example! {
                title: "Remove unknown field",
                source: r#"remove!(value: {"foo": "bar"}, path: ["baz"])"#,
                result: Ok(r#"{ "foo": "bar" }"#),
            },
            example! {
                title: "Multi-segment nested field",
                source: r#"remove!(value: { "foo": { "bar": "baz" } }, path: ["foo", "bar"])"#,
                result: Ok(r#"{ "foo": {} }"#),
            },
            example! {
                title: "Array indexing",
                source: r#"remove!(value: ["foo", "bar", "baz"], path: [-2])"#,
                result: Ok(r#"["foo", "baz"]"#),
            },
            example! {
                title: "Compaction",
                source: r#"remove!(value: { "foo": { "bar": [42], "baz": true } }, path: ["foo", "bar", 0], compact: true)"#,
                result: Ok(r#"{ "foo": { "baz": true } }"#),
            },
            example! {
                title: "Compact object",
                source: r#"remove!(value: {"foo": { "bar": true }}, path: ["foo", "bar"], compact: true)"#,
                result: Ok("{}"),
            },
            example! {
                title: "Compact array",
                source: r#"remove!(value: {"foo": [42], "bar": true }, path: ["foo", 0], compact: true)"#,
                result: Ok(r#"{ "bar": true }"#),
            },
            example! {
                title: "External target",
                source: indoc! {r#"
                    . = { "foo": true }
                    remove!(value: ., path: ["foo"])
                "#},
                result: Ok("{}"),
            },
            example! {
                title: "Variable",
                source: indoc! {r#"
                    var = { "foo": true }
                    remove!(value: var, path: ["foo"])
                "#},
                result: Ok("{}"),
            },
            example! {
                title: "Missing index",
                source: r#"remove!(value: {"foo": { "bar": [92, 42] }}, path: ["foo", "bar", 1, -1])"#,
                result: Ok(r#"{ "foo": { "bar": [92, 42] } }"#),
            },
            example! {
                title: "Invalid indexing",
                source: r#"remove!(value: [42], path: ["foo"])"#,
                result: Ok("[42]"),
            },
            example! {
                title: "Invalid segment type",
                source: r#"remove!(value: {"foo": { "bar": [92, 42] }}, path: ["foo", true])"#,
                result: Err(
                    r#"function call error for "remove" at (0:65): path segment must be either string or integer, not boolean"#,
                ),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let path = arguments.required("path");
        let compact = arguments.optional("compact");

        Ok(RemoveFn {
            value,
            path,
            compact,
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RemoveFn {
    value: Box<dyn Expression>,
    path: Box<dyn Expression>,
    compact: Option<Box<dyn Expression>>,
}

impl FunctionExpression for RemoveFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let path = self.path.resolve(ctx)?;
        let compact = self
            .compact
            .map_resolve_with_default(ctx, || DEFAULT_COMPACT.clone())?;
        let value = self.value.resolve(ctx)?;

        remove(path, compact, value)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let value_td = self.value.type_def(state);

        let mut td = TypeDef::from(Kind::never()).fallible();

        if value_td.is_array() {
            td = td.or_array(Collection::any());
        }

        if value_td.is_object() {
            td = td.or_object(Collection::any());
        }

        td
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        remove => Remove;

        array {
            args: func_args![value: value!([42]), path: value!([0])],
            want: Ok(value!([])),
            tdef: TypeDef::array(Collection::any()).fallible(),
        }

        object {
            args: func_args![value: value!({ "foo": 42 }), path: value!(["foo"])],
            want: Ok(value!({})),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }
    ];
}
