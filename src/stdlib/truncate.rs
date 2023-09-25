use crate::compiler::prelude::*;

fn truncate(value: Value, limit: Value, ellipsis: Value, suffix: Value) -> Resolved {
    let mut value = value.try_bytes_utf8_lossy()?.into_owned();
    let limit = limit.try_integer()?;
    let limit = if limit < 0 { 0 } else { limit as usize };
    let ellipsis = ellipsis.try_boolean()?;
    let suffix = suffix.try_bytes_utf8_lossy()?.to_string();
    let pos = if let Some((pos, chr)) = value.char_indices().take(limit).last() {
        // char_indices gives us the starting position of the character at limit,
        // we want the end position.
        pos + chr.len_utf8()
    } else {
        // We have an empty string
        0
    };
    if value.len() > pos {
        value.truncate(pos);
        if ellipsis {
            value.push_str("...");
        } else if !suffix.is_empty() {
            value.push_str(&suffix);
        }
    }
    Ok(value.into())
}

#[derive(Clone, Copy, Debug)]
pub struct Truncate;

impl Function for Truncate {
    fn identifier(&self) -> &'static str {
        "truncate"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "limit",
                kind: kind::INTEGER,
                required: true,
            },
            Parameter {
                keyword: "ellipsis",
                kind: kind::BOOLEAN,
                required: false,
            },
            Parameter {
                keyword: "suffix",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "truncate",
                source: r#"truncate("foobar", 3)"#,
                result: Ok("foo"),
            },
            Example {
                title: "ellipsis",
                source: r#"truncate("foobarzoo", 3, suffix: "...")"#,
                result: Ok("foo..."),
            },
            Example {
                title: "custom suffix",
                source: r#"truncate("foo bar zoo", 4, suffix: "[TRUNCATED]")"#,
                result: Ok("foo [TRUNCATED]"),
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
        let limit = arguments.required("limit");
        let ellipsis = arguments.optional("ellipsis").unwrap_or(expr!(false));
        let suffix = arguments.optional("suffix").unwrap_or(expr!(""));

        Ok(TruncateFn {
            value,
            limit,
            ellipsis,
            suffix,
        }
        .as_expr())
    }
}

#[derive(Debug, Clone)]
struct TruncateFn {
    value: Box<dyn Expression>,
    limit: Box<dyn Expression>,
    ellipsis: Box<dyn Expression>,
    suffix: Box<dyn Expression>,
}

impl FunctionExpression for TruncateFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let limit = self.limit.resolve(ctx)?;
        let ellipsis = self.ellipsis.resolve(ctx)?;
        let suffix = self.suffix.resolve(ctx)?;

        truncate(value, limit, ellipsis, suffix)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        truncate => Truncate;

        empty {
             args: func_args![value: "Super",
                              limit: 0,
             ],
             want: Ok(""),
             tdef: TypeDef::bytes().infallible(),
         }

        ellipsis {
            args: func_args![value: "Super",
                             limit: 0,
                             ellipsis: true
            ],
            want: Ok("..."),
            tdef: TypeDef::bytes().infallible(),
        }

        complete {
            args: func_args![value: "Super",
                             limit: 10
            ],
            want: Ok("Super"),
            tdef: TypeDef::bytes().infallible(),
        }

        exact {
            args: func_args![value: "Super",
                             limit: 5,
                             ellipsis: true
            ],
            want: Ok("Super"),
            tdef: TypeDef::bytes().infallible(),
        }

        big {
            args: func_args![value: "Supercalifragilisticexpialidocious",
                             limit: 5
            ],
            want: Ok("Super"),
            tdef: TypeDef::bytes().infallible(),
        }

        big_ellipsis {
            args: func_args![value: "Supercalifragilisticexpialidocious",
                             limit: 5,
                             ellipsis: true,
            ],
            want: Ok("Super..."),
            tdef: TypeDef::bytes().infallible(),
        }

        unicode {
            args: func_args![value: "♔♕♖♗♘♙♚♛♜♝♞♟",
                             limit: 6,
                             ellipsis: true
            ],
            want: Ok("♔♕♖♗♘♙..."),
            tdef: TypeDef::bytes().infallible(),
        }

        alternative_suffix {
            args: func_args![value: "Super",
                             limit: 1,
                             suffix: "[TRUNCATED]"
            ],
            want: Ok("S[TRUNCATED]"),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
