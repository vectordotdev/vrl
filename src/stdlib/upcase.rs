use crate::compiler::prelude::*;

fn upcase(value: &Value) -> Resolved {
    Ok(value.try_bytes_utf8_lossy()?.to_uppercase().into())
}

#[derive(Clone, Copy, Debug)]
pub struct Upcase;

impl Function for Upcase {
    fn identifier(&self) -> &'static str {
        "upcase"
    }

    fn usage(&self) -> &'static str {
        "Upcases `value`, where upcase is defined according to the Unicode Derived Core Property Uppercase."
    }

    fn category(&self) -> &'static str {
        Category::String.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Upcase a string",
            source: r#"upcase("Hello, World!")"#,
            result: Ok("HELLO, WORLD!"),
        }]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
            description: "The string to convert to uppercase.",
            default: None,
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(UpcaseFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct UpcaseFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for UpcaseFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        upcase(&value)
    }

    fn type_def(&self, _: &TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        upcase => Upcase;

        simple {
            args: func_args![value: "FOO 2 bar"],
            want: Ok(value!("FOO 2 BAR")),
            tdef: TypeDef::bytes(),
        }
    ];
}
