use crate::compiler::prelude::*;

#[allow(clippy::cast_possible_wrap)]
fn seahash(value: Value) -> Resolved {
    let value = value.try_bytes()?;
    Ok(Value::Integer(seahash::hash(&value) as i64))
}

#[derive(Clone, Copy, Debug)]
pub struct Seahash;

impl Function for Seahash {
    fn identifier(&self) -> &'static str {
        "seahash"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Calculates a [Seahash](https://docs.rs/seahash/latest/seahash/) hash of the `value`.
            **Note**: Due to limitations in the underlying VRL data types, this function converts the unsigned 64-bit integer SeaHash result to a signed 64-bit integer. Results higher than the signed 64-bit integer maximum value wrap around to negative values.
        "}
    }

    fn category(&self) -> &'static str {
        Category::Cryptography.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::INTEGER
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Calculate seahash",
                source: r#"seahash("foobar")"#,
                result: Ok("5348458858952426560"),
            },
            example! {
                title: "Calculate negative seahash",
                source: r#"seahash("bar")"#,
                result: Ok("-2796170501982571315"),
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

        Ok(SeahashFn { value }.as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The string to calculate the hash for.",
            default: None,
        }]
    }
}

#[derive(Debug, Clone)]
struct SeahashFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for SeahashFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        seahash(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::integer().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        seahash => Seahash;

        seahash {
             args: func_args![value: "foo"],
             want: Ok(4_413_582_353_838_009_230_i64),
             tdef: TypeDef::integer().infallible(),
        }

        seahash_buffer_overflow {
             args: func_args![value: "bar"],
             want: Ok(-2_796_170_501_982_571_315_i64),
             tdef: TypeDef::integer().infallible(),
        }
    ];
}
