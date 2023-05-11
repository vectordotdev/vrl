use crate::compiler::prelude::*;
use percent_encoding::percent_decode;

fn decode_percent(value: Value) -> Resolved {
    let value = value.try_bytes()?;
    Ok(percent_decode(&value)
        .decode_utf8_lossy()
        .to_string()
        .into())
}

#[derive(Clone, Copy, Debug)]
pub struct DecodePercent;

impl Function for DecodePercent {
    fn identifier(&self) -> &'static str {
        "decode_percent"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(DecodePercentFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "percent decode string",
            source: r#"decode_percent("foo%20bar%3F")"#,
            result: Ok(r#"foo bar?"#),
        }]
    }
}

#[derive(Clone, Debug)]
struct DecodePercentFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for DecodePercentFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        decode_percent(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::value;

    test_function![
        decode_percent => DecodePercent;

        decode {
            args: func_args![value: value!("foo%20%23%22%3C%3E%3F%60%7B%7D%2F%3A%3B%3D%40%5B%5C%5D%5E%7C%24%25%26%2B%2C%21%27%28%29%7Ebar")],
            want: Ok(value!(r#"foo #"<>?`{}/:;=@[\]^|$%&+,!'()~bar"#)),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
