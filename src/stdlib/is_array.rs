use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsArray;

impl Function for IsArray {
    fn identifier(&self) -> &'static str {
        "is_array"
    }

    fn usage(&self) -> &'static str {
        "Check if the `value`'s type is an array."
    }

    fn category(&self) -> &'static str {
        Category::Type.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns `true` if `value` is an array.",
            "Returns `false` if `value` is anything else.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::ANY,
            "The value to check if it is an array.",
        )];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Valid array",
                source: "is_array([1, 2, 3])",
                result: Ok("true"),
            },
            example! {
                title: "Non-matching type",
                source: r#"is_array("a string")"#,
                result: Ok("false"),
            },
            example! {
                title: "Boolean",
                source: "is_array(true)",
                result: Ok("false"),
            },
            example! {
                title: "Null",
                source: "is_array(null)",
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

        Ok(IsArrayFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct IsArrayFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsArrayFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        self.value.resolve(ctx).map(|v| value!(v.is_array()))
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        is_array => IsArray;

        array {
            args: func_args![value: value!([1, 2, 3])],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        integer {
            args: func_args![value: value!(1789)],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
