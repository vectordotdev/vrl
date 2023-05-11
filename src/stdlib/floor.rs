use crate::compiler::prelude::*;

use super::util::round_to_precision;

fn floor(precision: Option<Value>, value: Value) -> Resolved {
    let precision = match precision {
        Some(value) => value.try_integer()?,
        None => 0,
    };
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

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::ANY,
                required: true,
            },
            Parameter {
                keyword: "precision",
                kind: kind::ANY,
                required: false,
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
        let precision = arguments.optional("precision");

        Ok(FloorFn { value, precision }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "floor",
            source: r#"floor(9.8)"#,
            result: Ok("9.0"),
        }]
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
            .as_ref()
            .map(|expr| expr.resolve(ctx))
            .transpose()?;
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
