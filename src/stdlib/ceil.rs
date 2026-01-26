use super::util::round_to_precision;
use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_PRECISION: LazyLock<Value> = LazyLock::new(|| Value::Integer(0));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::FLOAT | kind::INTEGER,
            required: true,
            description: "The number to round up.",
            default: None,
        },
        Parameter {
            keyword: "precision",
            kind: kind::INTEGER,
            required: false,
            description: "The number of decimal places to round to.",
            default: Some(&DEFAULT_PRECISION),
        },
    ]
});

fn ceil(value: Value, precision: Value) -> Resolved {
    let precision = precision.try_integer()?;

    match value {
        Value::Float(f) => Ok(Value::from_f64_or_zero(round_to_precision(
            *f,
            precision,
            f64::ceil,
        ))),
        value @ Value::Integer(_) => Ok(value),
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::float() | Kind::integer(),
        }
        .into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Ceil;

impl Function for Ceil {
    fn identifier(&self) -> &'static str {
        "ceil"
    }

    fn usage(&self) -> &'static str {
        "Rounds the `value` up to the specified `precision`."
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let precision = arguments.optional("precision");

        Ok(CeilFn { value, precision }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Round a number up (without precision)",
                source: "ceil(4.345)",
                result: Ok("5.0"),
            },
            example! {
                title: "Round a number up (with precision)",
                source: "ceil(4.345, precision: 2)",
                result: Ok("4.35"),
            },
            example! {
                title: "Round an integer up (noop)",
                source: "ceil(5)",
                result: Ok("5"),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct CeilFn {
    value: Box<dyn Expression>,
    precision: Option<Box<dyn Expression>>,
}

impl FunctionExpression for CeilFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let precision = self
            .precision
            .map_resolve_with_default(ctx, || DEFAULT_PRECISION.clone())?;
        let value = self.value.resolve(ctx)?;

        ceil(value, precision)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        match Kind::from(self.value.type_def(state)) {
            v if v.is_float() || v.is_integer() => v.into(),
            _ => Kind::integer().or_float().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        ceil => Ceil;

        lower {
            args: func_args![value: value!(1234.2)],
            want: Ok(value!(1235.0)),
            tdef: TypeDef::float(),
        }

        higher {
            args: func_args![value: value!(1234.8)],
            want: Ok(value!(1235.0)),
            tdef: TypeDef::float(),
        }

        integer {
            args: func_args![value: value!(1234)],
            want: Ok(value!(1234)),
            tdef: TypeDef::integer(),
        }

        precision {
            args: func_args![value: value!(1234.39429),
                             precision: value!(1)
            ],
            want: Ok(value!(1234.4)),
            tdef: TypeDef::float(),
        }

        bigger_precision {
            args: func_args![value: value!(1234.56725),
                             precision: value!(4)
            ],
            want: Ok(value!(1234.5673)),
            tdef: TypeDef::float(),
        }

        huge_number {
             args: func_args![value: value!(9_876_543_210_123_456_789_098_765_432_101_234_567_890_987_654_321.987_654_321),
                             precision: value!(5)
            ],
            want: Ok(value!(9_876_543_210_123_456_789_098_765_432_101_234_567_890_987_654_321.987_66)),
            tdef: TypeDef::float(),
        }
    ];
}
