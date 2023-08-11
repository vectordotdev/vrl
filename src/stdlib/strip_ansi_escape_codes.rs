use crate::compiler::prelude::*;
use bytes::Bytes;

fn strip_ansi_escape_codes(bytes: Value) -> Resolved {
    let bytes = bytes.try_bytes()?;
    let stripped_bytes = Bytes::from(strip_ansi_escapes::strip(&bytes));
    Ok(stripped_bytes.into())
}

#[derive(Clone, Copy, Debug)]
pub struct StripAnsiEscapeCodes;

impl Function for StripAnsiEscapeCodes {
    fn identifier(&self) -> &'static str {
        "strip_ansi_escape_codes"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(StripAnsiEscapeCodesFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct StripAnsiEscapeCodesFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for StripAnsiEscapeCodesFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let bytes = self.value.resolve(ctx)?;

        strip_ansi_escape_codes(bytes)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        // We're marking this as infallible, because `strip_ansi_escapes` only
        // fails if it can't write to the buffer, which is highly unlikely to
        // occur.
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        strip_ansi_escape_codes => StripAnsiEscapeCodes;

        no_codes {
            args: func_args![value: "foo bar"],
            want: Ok("foo bar"),
            tdef: TypeDef::bytes().infallible(),
        }

        strip_1 {
            args: func_args![value: "\x1b[3;4Hfoo bar"],
            want: Ok("foo bar"),
            tdef: TypeDef::bytes().infallible(),
        }

        strip_2 {
            args: func_args![value: "\x1b[46mfoo\x1b[0m bar"],
            want: Ok("foo bar"),
            tdef: TypeDef::bytes().infallible(),
        }

        strip_3 {
            args: func_args![value: "\x1b[=3lfoo bar"],
            want: Ok("foo bar"),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
