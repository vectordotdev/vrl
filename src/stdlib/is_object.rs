use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsObject;

impl Function for IsObject {
    fn identifier(&self) -> &'static str {
        "is_object"
    }

    fn usage(&self) -> &'static str {
        "Check if `value`'s type is an object."
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns `true` if `value` is an object.",
            "Returns `false` if `value` is anything else.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to check if it is an object.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Valid object",
                source: r#"is_object({"foo": "bar"})"#,
                result: Ok("true"),
            },
            example! {
                title: "Non-matching type",
                source: r#"is_object("a string")"#,
                result: Ok("false"),
            },
            example! {
                title: "Boolean",
                source: "is_object(true)",
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

        Ok(IsObjectFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct IsObjectFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsObjectFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.value.resolve(ctx).map(|v| value!(v.is_object()))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        is_object => IsObject;

        bytes {
            args: func_args![value: value!("foobar")],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        object {
            args: func_args![value: value!({"foo": "bar"})],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
