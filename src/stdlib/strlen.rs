use crate::compiler::prelude::*;

fn strlen(value: &Value) -> Resolved {
    let v = value.try_bytes_utf8_lossy()?;

    Ok(v.chars().count().into())
}

#[derive(Clone, Copy, Debug)]
pub struct Strlen;

impl Function for Strlen {
    fn identifier(&self) -> &'static str {
        "strlen"
    }

    fn usage(&self) -> &'static str {
        indoc! {"
            Returns the number of UTF-8 characters in `value`. This differs from
            `length` which counts the number of bytes of a string.

            **Note**: This is the count of [Unicode scalar values](https://www.unicode.org/glossary/#unicode_scalar_value)
            which can sometimes differ from [Unicode code points](https://www.unicode.org/glossary/#code_point).
        "}
    }

    fn category(&self) -> &'static str {
        Category::Enumerate.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::INTEGER
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] =
            &[Parameter::required("value", kind::BYTES, "The string.")];
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Count Unicode scalar values",
            source: r#"strlen("ñandú")"#,
            result: Ok("5"),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(StrlenFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct StrlenFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for StrlenFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;

        strlen(&value)
    }

    fn type_def(&self, _state: &state::TypeState) -> TypeDef {
        TypeDef::integer().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        strlen => Strlen;

        string_value {
            args: func_args![value: value!("ñandú")],
            want: Ok(value!(5)),
            tdef: TypeDef::integer().infallible(),
        }
    ];
}
