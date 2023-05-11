use super::util;
use crate::compiler::prelude::*;

fn is_nullish(value: Value) -> bool {
    util::is_nullish(&value)
}

#[derive(Clone, Copy, Debug)]
pub struct IsNullish;

impl Function for IsNullish {
    fn identifier(&self) -> &'static str {
        "is_nullish"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "null",
            source: r#"is_nullish(null)"#,
            result: Ok("true"),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        Ok(IsNullishFn { value }.as_expr())
    }
}

#[derive(Clone, Debug)]
struct IsNullishFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsNullishFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        Ok(is_nullish(value).into())
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
        is_nullish => IsNullish;

        empty_string {
            args: func_args![value: value!("")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        single_space_string {
            args: func_args![value: value!(" ")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        multi_space_string {
            args: func_args![value: value!("     ")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        newline_string {
            args: func_args![value: value!("\n")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        carriage_return_string {
            args: func_args![value: value!("\r")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        dash_string {
            args: func_args![value: value!("-")],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        null {
            args: func_args![value: value!(null)],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        non_empty_string {
            args: func_args![value: value!("hello world")],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        // Shows that a non-string/null literal returns false
        integer {
            args: func_args![value: value!(427)],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        // Shows that a non-literal type returns false
        array {
            args: func_args![value: value!([1, 2, 3])],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
