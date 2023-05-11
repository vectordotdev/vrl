use crate::compiler::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct StripWhitespace;

impl Function for StripWhitespace {
    fn identifier(&self) -> &'static str {
        "strip_whitespace"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "start whitespace",
                source: r#"strip_whitespace("  foobar")"#,
                result: Ok("foobar"),
            },
            Example {
                title: "end whitespace",
                source: r#"strip_whitespace("foo bar  ")"#,
                result: Ok("foo bar"),
            },
            Example {
                title: "newlines",
                source: r#"strip_whitespace("\n\nfoo bar\n  ")"#,
                result: Ok("foo bar"),
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

        Ok(StripWhitespaceFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct StripWhitespaceFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for StripWhitespaceFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        Ok(value.try_bytes_utf8_lossy()?.trim().into())
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        strip_whitespace => StripWhitespace;

        empty {
            args: func_args![value: ""],
            want: Ok(""),
            tdef: TypeDef::bytes().infallible(),
        }

        just_spaces {
            args: func_args![value: "      "],
            want: Ok(""),
            tdef: TypeDef::bytes().infallible(),
        }

        no_spaces {
            args: func_args![value: "hi there"],
            want: Ok("hi there"),
            tdef: TypeDef::bytes().infallible(),
        }

        spaces {
            args: func_args![value: "           hi there        "],
            want: Ok("hi there"),
            tdef: TypeDef::bytes().infallible(),
        }

        unicode_whitespace {
            args: func_args![value: " \u{3000}\u{205F}\u{202F}\u{A0}\u{9} ❤❤ hi there ❤❤  \u{9}\u{A0}\u{202F}\u{205F}\u{3000} "],
            want: Ok("❤❤ hi there ❤❤"),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
