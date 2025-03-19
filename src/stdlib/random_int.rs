use crate::compiler::prelude::*;
use rand::{thread_rng, Rng};
use std::ops::Range;

const INVALID_RANGE_ERR: &str = "max must be greater than min";

fn random_int(min: Value, max: Value) -> Resolved {
    let range = get_range(min, max)?;

    let i: i64 = thread_rng().gen_range(range);

    Ok(Value::Integer(i))
}

fn get_range(min: Value, max: Value) -> std::result::Result<Range<i64>, &'static str> {
    let min = min.try_integer().expect("min must be an integer");
    let max = max.try_integer().expect("max must be an integer");

    if max <= min {
        return Err(INVALID_RANGE_ERR);
    }

    Ok(min..max)
}

#[derive(Clone, Copy, Debug)]
pub struct RandomInt;

impl Function for RandomInt {
    fn identifier(&self) -> &'static str {
        "random_int"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "min",
                kind: kind::INTEGER,
                required: true,
            },
            Parameter {
                keyword: "max",
                kind: kind::INTEGER,
                required: true,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "generate a random int from 0 to 10",
            source: "
				i = random_int(0, 10)
				i >= 0 && i < 10
                ",
            result: Ok("true"),
        }]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let min = arguments.required("min");
        let max = arguments.required("max");

        if let (Some(min), Some(max)) = (min.resolve_constant(state), max.resolve_constant(state)) {
            // check if range is valid
            let _: Range<i64> =
                get_range(min, max.clone()).map_err(|err| function::Error::InvalidArgument {
                    keyword: "max",
                    value: max,
                    error: err,
                })?;
        }

        Ok(RandomIntFn { min, max }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct RandomIntFn {
    min: Box<dyn Expression>,
    max: Box<dyn Expression>,
}

impl FunctionExpression for RandomIntFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let min = self.min.resolve(ctx)?;
        let max = self.max.resolve(ctx)?;

        random_int(min, max)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        match (
            self.min.resolve_constant(state),
            self.max.resolve_constant(state),
        ) {
            (Some(min), Some(max)) => {
                if get_range(min, max).is_ok() {
                    TypeDef::integer()
                } else {
                    TypeDef::integer().fallible()
                }
            }
            _ => TypeDef::integer().fallible(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;
    // positive tests are handled by examples

    test_function![
        random_int => RandomInt;

        bad_range {
            args: func_args![min: value!(1), max: value!(1)],
            want: Err("invalid argument"),
            tdef: TypeDef::integer().fallible(),
        }
    ];
}
