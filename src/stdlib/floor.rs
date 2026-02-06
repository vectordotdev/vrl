use crate::compiler::prelude::*;

use super::util::round_to_precision;
use std::sync::LazyLock;

static DEFAULT_PRECISION: LazyLock<Value> = LazyLock::new(|| Value::Integer(0));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The number to round down.",
            default: None,
        },
        Parameter {
            keyword: "precision",
            kind: kind::ANY,
            required: false,
            description: "The number of decimal places to round to.",
            default: Some(&DEFAULT_PRECISION),
        },
    ]
});

fn floor(precision: Value, value: Value) -> Resolved {
    let precision = precision.try_integer()?;

    match value {
        Value::Float(f) => Ok(Value::from_f64_or_zero(round_to_precision(
            *f,
            precision,
            f64::floor,
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
pub struct Floor;

impl Function for Floor {
    fn identifier(&self) -> &'static str {
        "floor"
    }

    fn usage(&self) -> &'static str {
        "Rounds the `value` down to the specified `precision`."
    }

    fn return_kind(&self) -> u16 {
        kind::INTEGER | kind::FLOAT
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns an integer if `precision` is `0` (this is the default). Returns a float otherwise.",
        ]
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

        Ok(FloorFn { value, precision }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Round a number down (without precision)",
                source: "floor(9.8)",
                result: Ok("9.0"),
            },
            example! {
                title: "Round a number down (with precision)",
                source: "floor(4.345, precision: 2)",
                result: Ok("4.34"),
            },
        ]
    }
}

#[derive(Clone, Debug)]
struct FloorFn {
    value: Box<dyn Expression>,
    precision: Option<Box<dyn Expression>>,
}

impl FunctionExpression for FloorFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let precision = self
            .precision
            .map_resolve_with_default(ctx, || DEFAULT_PRECISION.clone())?;
        let value = self.value.resolve(ctx)?;

        floor(precision, value)
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
        floor => Floor;

        lower {
            args: func_args![value: 1234.2],
            want: Ok(value!(1234.0)),
            tdef: TypeDef::float(),
        }

        higher {
            args: func_args![value: 1234.8],
            want: Ok(value!(1234.0)),
            tdef: TypeDef::float(),
        }

        exact {
            args: func_args![value: 1234],
            want: Ok(value!(1234)),
            tdef: TypeDef::integer(),
        }

        precision {
            args: func_args![value: 1234.39429,
                             precision: 1],
            want: Ok(value!(1234.3)),
            tdef: TypeDef::float(),
        }

        bigger_precision {
            args: func_args![value: 1234.56789,
                             precision: 4],
            want: Ok(value!(1234.5678)),
            tdef: TypeDef::float(),
        }

        huge_number {
            args: func_args![value: 9_876_543_210_123_456_789_098_765_432_101_234_567_890_987_654_321.987_654_321,
                             precision: 5],
            want: Ok(value!(9_876_543_210_123_456_789_098_765_432_101_234_567_890_987_654_321.987_65)),
            tdef: TypeDef::float(),
        }
    ];
}
