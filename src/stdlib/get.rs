use crate::compiler::prelude::*;
use crate::path::{OwnedSegment, OwnedValuePath};

fn get(value: Value, value_path: Value) -> Resolved {
    let path = match value_path {
        Value::Array(array) => {
            let mut path = OwnedValuePath::root();

            for segment in array {
                let segment = match segment {
                    Value::Bytes(field) => {
                        OwnedSegment::field(String::from_utf8_lossy(&field).as_ref())
                    }
                    Value::Integer(index) => OwnedSegment::index(index as isize),
                    value => {
                        return Err(format!(
                            "path segment must be either string or integer, not {}",
                            value.kind()
                        )
                        .into())
                    }
                };
                path.push(segment);
            }

            path
        }
        value => {
            return Err(ValueError::Expected {
                got: value.kind(),
                expected: Kind::array(Collection::any()),
            }
            .into())
        }
    };
    Ok(value.get(&path).cloned().unwrap_or(Value::Null))
}

#[derive(Clone, Copy, Debug)]
pub struct Get;

impl Function for Get {
    fn identifier(&self) -> &'static str {
        "get"
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
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "returns existing field",
                source: r#"get!(value: {"foo": "bar"}, path: ["foo"])"#,
                result: Ok(r#""bar""#),
            },
            Example {
                title: "returns null for unknown field",
                source: r#"get!(value: {"foo": "bar"}, path: ["baz"])"#,
                result: Ok("null"),
            },
            Example {
                title: "nested path",
                source: r#"get!(value: {"foo": { "bar": true }}, path: ["foo", "bar"])"#,
                result: Ok("true"),
            },
            Example {
                title: "indexing",
                source: "get!(value: [92, 42], path: [0])",
                result: Ok("92"),
            },
            Example {
                title: "nested indexing",
                source: r#"get!(value: {"foo": { "bar": [92, 42] }}, path: ["foo", "bar", 1])"#,
                result: Ok("42"),
            },
            Example {
                title: "external target",
                source: indoc! {r#"
                    . = { "foo": true }
                    get!(value: ., path: ["foo"])
                "#},
                result: Ok("true"),
            },
            Example {
                title: "variable",
                source: indoc! {r#"
                    var = { "foo": true }
                    get!(value: var, path: ["foo"])
                "#},
                result: Ok("true"),
            },
            Example {
                title: "missing index",
                source: r#"get!(value: {"foo": { "bar": [92, 42] }}, path: ["foo", "bar", 1, -1])"#,
                result: Ok("null"),
            },
            Example {
                title: "invalid indexing",
                source: r#"get!(value: [42], path: ["foo"])"#,
                result: Ok("null"),
            },
            Example {
                title: "invalid segment type",
                source: r#"get!(value: {"foo": { "bar": [92, 42] }}, path: ["foo", true])"#,
                result: Err(
                    r#"function call error for "get" at (0:62): path segment must be either string or integer, not boolean"#,
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

        Ok(GetFn { value, path }.as_expr())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GetFn {
    value: Box<dyn Expression>,
    path: Box<dyn Expression>,
}

impl FunctionExpression for GetFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let path = self.path.resolve(ctx)?;
        let value = self.value.resolve(ctx)?;

        get(value, path)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::any().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        get => Get;

        any {
            args: func_args![value: value!([42]), path: value!([0])],
            want: Ok(42),
            tdef: TypeDef::any().fallible(),
        }
    ];
}
