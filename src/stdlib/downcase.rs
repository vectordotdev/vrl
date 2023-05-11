use crate::compiler::prelude::*;

fn downcase(value: Value) -> Resolved {
    Ok(value.try_bytes_utf8_lossy()?.to_lowercase().into())
}

#[derive(Clone, Copy, Debug)]
pub struct Downcase;

impl Function for Downcase {
    fn identifier(&self) -> &'static str {
        "downcase"
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

        Ok(DowncaseFn { value }.as_expr())
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "downcase",
            source: r#"downcase("FOO 2 BAR")"#,
            result: Ok("foo 2 bar"),
        }]
    }
}

#[derive(Debug, Clone)]
struct DowncaseFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for DowncaseFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        downcase(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        downcase => Downcase;

        simple {
            args: func_args![value: "FOO 2 bar"],
            want: Ok(value!("foo 2 bar")),
            tdef: TypeDef::bytes(),
        }
    ];
}
