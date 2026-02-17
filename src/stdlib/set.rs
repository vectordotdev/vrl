use crate::compiler::prelude::*;
use crate::path::{OwnedSegment, OwnedValuePath};

fn set(path: Value, mut value: Value, data: Value) -> Resolved {
    let path = match path {
        Value::Array(segments) => {
            let mut insert = OwnedValuePath::root();

            for segment in segments {
                let segment = match segment {
                    Value::Bytes(path) => {
                        OwnedSegment::Field(String::from_utf8_lossy(&path).into())
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

                insert.push_segment(segment);
            }

            insert
        }
        value => {
            return Err(ValueError::Expected {
                got: value.kind(),
                expected: Kind::array(Collection::any()) | Kind::bytes(),
            }
            .into());
        }
    };
    value.insert(&path, data);
    Ok(value)
}

#[derive(Clone, Copy, Debug)]
pub struct Set;

impl Function for Set {
    fn identifier(&self) -> &'static str {
        "set"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Dynamically insert data into the path of a given object or array.

            If you know the path you want to assign a value to,
            use static path assignments such as `.foo.bar[1] = true` for
            improved performance and readability. However, if you do not
            know the path names, use the dynamic `set` function to
            insert the data into the object or array.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Path.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["The `path` segment must be a string or an integer."]
    }

    fn return_kind(&self) -> u16 {
        kind::OBJECT | kind::ARRAY
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[
            Parameter::required(
                "value",
                kind::OBJECT | kind::ARRAY,
                "The object or array to insert data into.",
            ),
            Parameter::required(
                "path",
                kind::ARRAY,
                "An array of path segments to insert the value into.",
            ),
            Parameter::required("data", kind::ANY, "The data to be inserted."),
        ];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Single-segment top-level field",
                source: r#"set!(value: { "foo": "bar" }, path: ["foo"], data: "baz")"#,
                result: Ok(r#"{ "foo": "baz" }"#),
            },
            example! {
                title: "Multi-segment nested field",
                source: r#"set!(value: { "foo": { "bar": "baz" } }, path: ["foo", "bar"], data: "qux")"#,
                result: Ok(r#"{ "foo": { "bar": "qux" } }"#),
            },
            example! {
                title: "Array",
                source: r#"set!(value: ["foo", "bar", "baz"], path: [-2], data: 42)"#,
                result: Ok(r#"["foo", 42, "baz"]"#),
            },
            example! {
                title: "Nested fields",
                source: r#"set!(value: {}, path: ["foo", "bar"], data: "baz")"#,
                result: Ok(r#"{ "foo": { "bar" : "baz" } }"#),
            },
            example! {
                title: "Nested indexing",
                source: r#"set!(value: {"foo": { "bar": [] }}, path: ["foo", "bar", 1], data: "baz")"#,
                result: Ok(r#"{ "foo": { "bar": [null, "baz"] } }"#),
            },
            example! {
                title: "External target",
                source: indoc! {r#"
                    . = { "foo": true }
                    set!(value: ., path: ["bar"], data: "baz")
                "#},
                result: Ok(r#"{ "foo": true, "bar": "baz" }"#),
            },
            example! {
                title: "Variable",
                source: indoc! {r#"
                    var = { "foo": true }
                    set!(value: var, path: ["bar"], data: "baz")
                "#},
                result: Ok(r#"{ "foo": true, "bar": "baz" }"#),
            },
            example! {
                title: "Invalid indexing",
                source: r#"set!(value: [], path: ["foo"], data: "baz")"#,
                result: Ok(r#"{ "foo": "baz" }"#),
            },
            example! {
                title: "Invalid segment type",
                source: r#"set!({"foo": { "bar": [92, 42] }}, ["foo", true], "baz")"#,
                result: Err(
                    r#"function call error for "set" at (0:56): path segment must be either string or integer, not boolean"#,
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
        let data = arguments.required("data");

        Ok(SetFn { value, path, data }.as_expr())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SetFn {
    value: Box<dyn Expression>,
    path: Box<dyn Expression>,
    data: Box<dyn Expression>,
}

impl FunctionExpression for SetFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let path = self.path.resolve(ctx)?;
        let value = self.value.resolve(ctx)?;
        let data = self.data.resolve(ctx)?;

        set(path, value, data)
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
        set => Set;

        array {
            args: func_args![value: value!([]), path: vec![0], data: true],
            want: Ok(vec![true]),
            tdef: TypeDef::array(Collection::any()).fallible(),
        }

        object {
            args: func_args![value: value!({}), path: vec!["foo"], data: true],
            want: Ok(value!({ "foo": true })),
            tdef: TypeDef::object(Collection::any()).fallible(),
        }
    ];
}
