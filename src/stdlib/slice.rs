use std::ops::Range;

use crate::compiler::prelude::*;
use std::sync::LazyLock;

static DEFAULT_END: LazyLock<Value> = LazyLock::new(|| Value::Bytes(Bytes::from("String length")));

static PARAMETERS: LazyLock<Vec<Parameter>> = LazyLock::new(|| {
    vec![
        Parameter {
            keyword: "value",
            kind: kind::BYTES | kind::ARRAY,
            required: true,
            description: "The string or array to slice.",
            default: None,
        },
        Parameter {
            keyword: "start",
            kind: kind::INTEGER,
            required: true,
            description: "The inclusive start position. A zero-based index that can be negative.",
            default: None,
        },
        Parameter {
            keyword: "end",
            kind: kind::INTEGER,
            required: false,
            description: "The exclusive end position. A zero-based index that can be negative.",
            default: Some(&DEFAULT_END),
        },
    ]
});

#[allow(clippy::cast_possible_wrap)]
#[allow(clippy::cast_sign_loss)]
#[allow(clippy::cast_possible_truncation)] //TODO evaluate removal options
fn slice(start: i64, end: Option<i64>, value: Value) -> Resolved {
    let range = |len: i64| -> ExpressionResult<Range<usize>> {
        let start = match start {
            start if start < 0 => start + len,
            start => start,
        };

        let end = match end {
            Some(end) if end < 0 => end + len,
            Some(end) => end,
            None => len,
        };

        match () {
            () if start < 0 || start > len => {
                Err(format!(r#""start" must be between "{}" and "{len}""#, -len).into())
            }
            () if end < start => Err(r#""end" must be greater or equal to "start""#.into()),
            () if end > len => Ok(start as usize..len as usize),
            () => Ok(start as usize..end as usize),
        }
    };
    match value {
        Value::Bytes(v) => range(v.len() as i64)
            .map(|range| v.slice(range))
            .map(Value::from),
        Value::Array(mut v) => range(v.len() as i64)
            .map(|range| v.drain(range).collect::<Vec<_>>())
            .map(Value::from),
        value => Err(ValueError::Expected {
            got: value.kind(),
            expected: Kind::bytes() | Kind::array(Collection::any()),
        }
        .into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Slice;

impl Function for Slice {
    fn identifier(&self) -> &'static str {
        "slice"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Returns a slice of `value` between the `start` and `end` positions.

            If the `start` and `end` parameters are negative, they refer to positions counting from the right of the
            string or array. If `end` refers to a position that is greater than the length of the string or array,
            a slice up to the end of the string or array is returned.
        "}
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS.as_slice()
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Slice a string (positive index)",
                source: r#"slice!("Supercalifragilisticexpialidocious", start: 5, end: 13)"#,
                result: Ok("califrag"),
            },
            example! {
                title: "Slice a string (negative index)",
                source: r#"slice!("Supercalifragilisticexpialidocious", start: 5, end: -14)"#,
                result: Ok("califragilistic"),
            },
            example! {
                title: "String start",
                source: r#"slice!("foobar", 3)"#,
                result: Ok("bar"),
            },
            example! {
                title: "Array start",
                source: "slice!([0, 1, 2], 1)",
                result: Ok("[1, 2]"),
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
        let start = arguments.required("start");
        let end = arguments.optional("end");

        Ok(SliceFn { value, start, end }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct SliceFn {
    value: Box<dyn Expression>,
    start: Box<dyn Expression>,
    end: Option<Box<dyn Expression>>,
}

impl FunctionExpression for SliceFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let start = self.start.resolve(ctx)?.try_integer()?;
        let end = match &self.end {
            Some(expr) => Some(expr.resolve(ctx)?.try_integer()?),
            None => None,
        };
        let value = self.value.resolve(ctx)?;

        slice(start, end, value)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let td = TypeDef::from(Kind::never()).fallible();

        match self.value.type_def(state) {
            v if v.is_bytes() => td.union(v),
            v if v.is_array() => td.union(v),
            _ => td.or_bytes().or_array(Collection::any()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::value;
    use std::collections::BTreeMap;

    use super::*;

    test_function![
        slice => Slice;

        bytes_0 {
            args: func_args![value: "foo",
                             start: 0
            ],
            want: Ok("foo"),
            tdef: TypeDef::bytes().fallible(),
        }

        bytes_1 {
            args: func_args![value: "foo",
                             start: 1
            ],
            want: Ok("oo"),
            tdef: TypeDef::bytes().fallible(),
        }

        bytes_2 {
            args: func_args![value: "foo",
                             start: 2
            ],
            want: Ok("o"),
            tdef: TypeDef::bytes().fallible(),
        }

        bytes_minus_2 {
            args: func_args![value: "foo",
                             start: -2
            ],
            want: Ok("oo"),
            tdef: TypeDef::bytes().fallible(),
        }

        bytes_empty {
            args: func_args![value: "foo",
                             start: 3
            ],
            want: Ok(""),
            tdef: TypeDef::bytes().fallible(),
        }

        bytes_empty_start_end {
            args: func_args![value: "foo",
                             start: 2,
                             end: 2
            ],
            want: Ok(""),
            tdef: TypeDef::bytes().fallible(),
        }

        bytes_overrun {
            args: func_args![value: "foo",
                             start: 0,
                             end: 4
            ],
            want: Ok("foo"),
            tdef: TypeDef::bytes().fallible(),
        }

        bytes_start_overrun {
            args: func_args![value: "foo",
                             start: 1,
                             end: 5
            ],
            want: Ok("oo"),
            tdef: TypeDef::bytes().fallible(),
        }

        bytes_negative  {
            args: func_args![value: "Supercalifragilisticexpialidocious",
                             start: -7
            ],
            want: Ok("docious"),
            tdef: TypeDef::bytes().fallible(),
        }

        bytes_middle {
            args: func_args![value: "Supercalifragilisticexpialidocious",
                             start: 5,
                             end: 9
            ],
            want: Ok("cali"),
            tdef: TypeDef::bytes().fallible(),
        }

        array_0 {
            args: func_args![value: vec![0, 1, 2],
                             start: 0
            ],
            want: Ok(vec![0, 1, 2]),
            tdef: TypeDef::array(Collection::from_parts(BTreeMap::from([
                (Index::from(0), Kind::integer()),
                (Index::from(1), Kind::integer()),
                (Index::from(2), Kind::integer()),
            ]), Kind::undefined())).fallible(),
        }

        array_1 {
            args: func_args![value: vec![0, 1, 2],
                             start: 1
            ],
            want: Ok(vec![1, 2]),
            tdef: TypeDef::array(Collection::from_parts(BTreeMap::from([
                (Index::from(0), Kind::integer()),
                (Index::from(1), Kind::integer()),
                (Index::from(2), Kind::integer()),
            ]), Kind::undefined())).fallible(),
        }

        array_minus_2 {
            args: func_args![value: vec![0, 1, 2],
                             start: -2
            ],
            want: Ok(vec![1, 2]),
            tdef: TypeDef::array(Collection::from_parts(BTreeMap::from([
                (Index::from(0), Kind::integer()),
                (Index::from(1), Kind::integer()),
                (Index::from(2), Kind::integer()),
            ]), Kind::undefined())).fallible(),
        }

        array_mixed_types {
            args: func_args![value: value!([0, "ook", true]),
                             start: 1
            ],
            want: Ok(value!(["ook", true])),
                       tdef: TypeDef::array(Collection::from_parts(BTreeMap::from([
                (Index::from(0), Kind::integer()),
                (Index::from(1), Kind::bytes()),
                (Index::from(2), Kind::boolean()),
            ]), Kind::undefined())).fallible(),
        }

        error_after_end {
            args: func_args![value: "foo",
                             start: 4
            ],
            want: Err(r#""start" must be between "-3" and "3""#),
            tdef: TypeDef::bytes().fallible(),
        }

        error_minus_before_start {
            args: func_args![value: "foo",
                             start: -4
            ],
            want: Err(r#""start" must be between "-3" and "3""#),
            tdef: TypeDef::bytes().fallible(),
        }

        error_start_end {
            args: func_args![value: "foo",
                             start: 2,
                             end: 1
            ],
            want: Err(r#""end" must be greater or equal to "start""#),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
