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
                    Value::Integer(index) => OwnedSegment::Index(index as isize),
                    value => {
                        return Err(format!(
                            r#"path segment must be either string or integer, not {}"#,
                            value.kind()
                        )
                        .into())
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
            .into())
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

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::OBJECT | kind::ARRAY,
                required: true,
            },
            Parameter {
                keyword: "path",
                kind: kind::ARRAY,
                required: true,
            },
            Parameter {
                keyword: "data",
                kind: kind::ANY,
                required: true,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "set existing field",
                source: r#"set!(value: {"foo": "bar"}, path: ["foo"], data: "baz")"#,
                result: Ok(r#"{ "foo": "baz" }"#),
            },
            Example {
                title: "nested fields",
                source: r#"set!(value: {}, path: ["foo", "bar"], data: "baz")"#,
                result: Ok(r#"{ "foo": { "bar" : "baz" } }"#),
            },
            Example {
                title: "indexing",
                source: r#"set!(value: [{ "foo": "bar" }], path: [0, "foo", "bar"], data: "baz")"#,
                result: Ok(r#"[{ "foo": { "bar": "baz" } }]"#),
            },
            Example {
                title: "nested indexing",
                source: r#"set!(value: {"foo": { "bar": [] }}, path: ["foo", "bar", 1], data: "baz")"#,
                result: Ok(r#"{ "foo": { "bar": [null, "baz"] } }"#),
            },
            Example {
                title: "external target",
                source: indoc! {r#"
                    . = { "foo": true }
                    set!(value: ., path: ["bar"], data: "baz")
                "#},
                result: Ok(r#"{ "foo": true, "bar": "baz" }"#),
            },
            Example {
                title: "variable",
                source: indoc! {r#"
                    var = { "foo": true }
                    set!(value: var, path: ["bar"], data: "baz")
                "#},
                result: Ok(r#"{ "foo": true, "bar": "baz" }"#),
            },
            Example {
                title: "invalid indexing",
                source: r#"set!(value: [], path: ["foo"], data: "baz")"#,
                result: Ok(r#"{ "foo": "baz" }"#),
            },
            Example {
                title: "invalid segment type",
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
            td = td.or_array(Collection::any())
        };

        if value_td.is_object() {
            td = td.or_object(Collection::any())
        };

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
