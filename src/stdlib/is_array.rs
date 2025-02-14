use crate::compiler::prelude::*;
use crate::value;

#[derive(Clone, Copy, Debug)]
pub struct IsArray;

impl Function for IsArray {
    fn identifier(&self) -> &'static str {
        "is_array"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "array",
                source: "is_array([1, 2, 3])",
                result: Ok("true"),
            },
            Example {
                title: "boolean",
                source: "is_array(true)",
                result: Ok("false"),
            },
            Example {
                title: "null",
                source: "is_array(null)",
                result: Ok("false"),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut CompileContext,
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
