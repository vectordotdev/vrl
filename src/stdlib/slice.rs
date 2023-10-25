use std::ops::Range;

use crate::compiler::prelude::*;

#[allow(clippy::cast_possible_wrap)]
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
            _ if start < 0 || start > len => {
                Err(format!(r#""start" must be between "{}" and "{len}""#, -len).into())
            }
            _ if end < start => Err(r#""end" must be greater or equal to "start""#.into()),
            _ if end > len => Ok(start as usize..len as usize),
            _ => Ok(start as usize..end as usize),
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

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES | kind::ARRAY,
                required: true,
            },
            Parameter {
                keyword: "start",
                kind: kind::INTEGER,
                required: true,
            },
            Parameter {
                keyword: "end",
                kind: kind::INTEGER,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "string start",
                source: r#"slice!("foobar", 3)"#,
                result: Ok("bar"),
            },
            Example {
                title: "string start..end",
                source: r#"slice!("foobar", 2, 4)"#,
                result: Ok("ob"),
            },
            Example {
                title: "array start",
                source: r#"slice!([0, 1, 2], 1)"#,
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
            v if v.is_array() => td.union(v).collect_subtypes(),
            _ => td.or_bytes().or_array(Collection::any()),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::value;

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
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer())).fallible(),
        }

        array_1 {
            args: func_args![value: vec![0, 1, 2],
                             start: 1
            ],
            want: Ok(vec![1, 2]),
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer())).fallible(),
        }

        array_minus_2 {
            args: func_args![value: vec![0, 1, 2],
                             start: -2
            ],
            want: Ok(vec![1, 2]),
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer())).fallible(),
        }

        array_mixed_types {
            args: func_args![value: value!([0, "ook", true]),
                             start: 1
            ],
            want: Ok(value!(["ook", true])),
            tdef: TypeDef::array(Collection::from_unknown(Kind::integer().or_bytes().or_boolean())).fallible(),
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
