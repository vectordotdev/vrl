use crate::compiler::prelude::*;

const MAX_LENGTH: usize = 100_000;

static DEFAULT_STEP: Value = Value::Integer(1);

fn range(start: Value, end: Value, step: Value) -> Resolved {
    let start = start.try_integer()?;
    let end = end.try_integer()?;
    let step = step.try_integer()?;

    if step == 0 {
        return Err(r#""step" must not be zero"#.into());
    }

    let span = if (step > 0 && end > start) || (step < 0 && start > end) {
        start.abs_diff(end)
    } else {
        0
    };
    let length = usize::try_from(span.div_ceil(step.unsigned_abs()))
        .ok()
        .filter(|length| *length <= MAX_LENGTH)
        .ok_or_else(|| format!("range is too large: must have at most {MAX_LENGTH} elements"))?;

    // `checked_add` only overflows past the last element, which `take` never reaches.
    Ok(std::iter::successors(Some(start), |n| n.checked_add(step))
        .take(length)
        .map(Value::Integer)
        .collect::<Vec<_>>()
        .into())
}

#[derive(Clone, Copy, Debug)]
pub struct Range;

impl Function for Range {
    fn identifier(&self) -> &'static str {
        "range"
    }

    fn description(&self) -> &'static str {
        "Returns an array of integers from `start` (inclusive) to `end` (exclusive), incremented by `step`."
    }

    fn category(&self) -> &'static str {
        Category::Array.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &[
            "`step` is zero.",
            "The resulting array would have more than 100,000 elements.",
        ]
    }

    fn return_kind(&self) -> u16 {
        kind::ARRAY
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[
            Parameter::required("start", kind::INTEGER, "The first value of the range."),
            Parameter::required(
                "end",
                kind::INTEGER,
                "The end of the range. This value is not included in the result.",
            ),
            Parameter::optional(
                "step",
                kind::INTEGER,
                "The increment between values. Can be negative, but not zero.",
            )
            .default(&DEFAULT_STEP),
        ];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Generate a range of integers",
                source: "range!(500, 505)",
                result: Ok("[500, 501, 502, 503, 504]"),
            },
            example! {
                title: "Generate a range with a step",
                source: "range!(0, 10, 3)",
                result: Ok("[0, 3, 6, 9]"),
            },
            example! {
                title: "Generate a decreasing range",
                source: "range!(5, 0, -1)",
                result: Ok("[5, 4, 3, 2, 1]"),
            },
            example! {
                title: "Expand a range expressed as a string",
                source: r#"bounds = split("1..4", ".."); range!(to_int!(bounds[0]), to_int!(bounds[1]) + 1)"#,
                result: Ok("[1, 2, 3, 4]"),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let start = arguments.required("start");
        let end = arguments.required("end");
        let step = arguments.optional("step");

        Ok(RangeFn { start, end, step }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct RangeFn {
    start: Box<dyn Expression>,
    end: Box<dyn Expression>,
    step: Option<Box<dyn Expression>>,
}

impl FunctionExpression for RangeFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let start = self.start.resolve(ctx)?;
        let end = self.end.resolve(ctx)?;
        let step = self
            .step
            .map_resolve_with_default(ctx, || DEFAULT_STEP.clone())?;

        range(start, end, step)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::array(Collection::from_unknown(Kind::integer())).fallible()
    }
}

#[cfg(test)]
mod tests {
    use crate::value;

    use super::*;

    test_function![
        range => Range;

        ascending {
            args: func_args![start: 500, end: 503],
            want: Ok(value!([500, 501, 502])),
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer())).fallible(),
        }

        with_step {
            args: func_args![start: 0, end: 10, step: 3],
            want: Ok(value!([0, 3, 6, 9])),
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer())).fallible(),
        }

        negative_step {
            args: func_args![start: 5, end: 0, step: -2],
            want: Ok(value!([5, 3, 1])),
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer())).fallible(),
        }

        empty_when_start_equals_end {
            args: func_args![start: 3, end: 3],
            want: Ok(value!([])),
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer())).fallible(),
        }

        empty_when_step_goes_the_wrong_way {
            args: func_args![start: 0, end: 5, step: -1],
            want: Ok(value!([])),
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer())).fallible(),
        }

        near_integer_bounds {
            args: func_args![start: i64::MAX - 2, end: i64::MAX],
            want: Ok(value!([(i64::MAX - 2), (i64::MAX - 1)])),
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer())).fallible(),
        }

        zero_step {
            args: func_args![start: 0, end: 5, step: 0],
            want: Err(r#""step" must not be zero"#),
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer())).fallible(),
        }

        too_large {
            args: func_args![start: i64::MIN, end: i64::MAX],
            want: Err("range is too large: must have at most 100000 elements"),
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer())).fallible(),
        }
    ];
}
