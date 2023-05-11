use crate::compiler::prelude::*;

fn is_empty(value: Value) -> Resolved {
    let empty = match value {
        Value::Object(v) => v.is_empty(),
        Value::Array(v) => v.is_empty(),
        Value::Bytes(v) => v.is_empty(),
        value => {
            return Err(ValueError::Expected {
                got: value.kind(),
                expected: Kind::array(Collection::any())
                    | Kind::object(Collection::any())
                    | Kind::bytes(),
            }
            .into())
        }
    };

    Ok(empty.into())
}

#[derive(Clone, Copy, Debug)]
pub struct IsEmpty;

impl Function for IsEmpty {
    fn identifier(&self) -> &'static str {
        "is_empty"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::OBJECT | kind::ARRAY | kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "empty string",
                source: r#"is_empty("")"#,
                result: Ok("true"),
            },
            Example {
                title: "empty array",
                source: r#"is_empty([])"#,
                result: Ok("true"),
            },
            Example {
                title: "empty object",
                source: r#"is_empty({})"#,
                result: Ok("true"),
            },
            Example {
                title: "non-empty array",
                source: r#"is_empty([1,2,3])"#,
                result: Ok("false"),
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

        Ok(IsEmptyFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct IsEmptyFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsEmptyFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        is_empty(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        is_empty => IsEmpty;

        empty_array {
            args: func_args![value: value!([])],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        non_empty_array {
            args: func_args![value: value!(["foo"])],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        empty_object {
            args: func_args![value: value!({})],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        non_empty_object {
            args: func_args![value: value!({"foo": "bar"})],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        empty_string {
            args: func_args![value: ""],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        non_empty_string {
            args: func_args![value: "foo"],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
