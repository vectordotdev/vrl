use crate::compiler::prelude::*;

fn length(value: Value) -> Resolved {
    match value {
        Value::Array(v) => Ok(v.len().into()),
        Value::Object(v) => Ok(v.len().into()),
        Value::Bytes(v) => Ok(v.len().into()),
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::array(Collection::any())
                | Kind::object(Collection::any())
                | Kind::bytes(),
        }
        .into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Length;

impl Function for Length {
    fn identifier(&self) -> &'static str {
        "length"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Returns the length of the `value`.

            * If `value` is an array, returns the number of elements.
            * If `value` is an object, returns the number of top-level keys.
            * If `value` is a string, returns the number of bytes in the string. If
              you want the number of characters, see `strlen`.
        "}
    }

    fn return_kind(&self) -> u16 {
        kind::INTEGER
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "If `value` is an array, returns the number of elements.",
            "If `value` is an object, returns the number of top-level keys.",
            "If `value` is a string, returns the number of bytes in the string.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ARRAY | kind::OBJECT | kind::BYTES,
            required: true,
            description: "The array or object.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Length (object)",
                source: r#"length({ "portland": "Trail Blazers", "seattle": "Supersonics" })"#,
                result: Ok("2"),
            },
            example! {
                title: "Length (nested object)",
                source: r#"length({ "home": { "city": "Portland", "state": "Oregon" }, "name": "Trail Blazers", "mascot": { "name": "Blaze the Trail Cat" } })"#,
                result: Ok("3"),
            },
            example! {
                title: "Length (array)",
                source: r#"length(["Trail Blazers", "Supersonics", "Grizzlies"])"#,
                result: Ok("3"),
            },
            example! {
                title: "Length (string)",
                source: r#"length("The Planet of the Apes Musical")"#,
                result: Ok("30"),
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

        Ok(LengthFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct LengthFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for LengthFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        length(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::integer().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        length => Length;

        non_empty_object_value {
            args: func_args![value: value!({"foo": "bar", "baz": true, "baq": [1, 2, 3]})],
            want: Ok(value!(3)),
            tdef: TypeDef::integer().infallible(),
        }

        empty_object_value {
            args: func_args![value: value!({})],
            want: Ok(value!(0)),
            tdef: TypeDef::integer().infallible(),
        }

        nested_object_value {
            args: func_args![value: value!({"nested": {"foo": "bar"}})],
            want: Ok(value!(1)),
            tdef: TypeDef::integer().infallible(),
        }

        non_empty_array_value {
            args: func_args![value: value!([1, 2, 3, 4, true, "hello"])],
            want: Ok(value!(6)),
            tdef: TypeDef::integer().infallible(),
        }

        empty_array_value {
            args: func_args![value: value!([])],
            want: Ok(value!(0)),
            tdef: TypeDef::integer().infallible(),
        }

        string_value {
            args: func_args![value: value!("hello world")],
            want: Ok(value!(11)),
            tdef: TypeDef::integer().infallible(),
        }
    ];
}
