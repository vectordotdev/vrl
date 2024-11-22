use crate::compiler::prelude::*;
use crc32fast::Hasher;

fn crc32(value: Value) -> Resolved {
    let value = value.try_bytes()?;
    let mut hasher = Hasher::new();
    hasher.update(&value);
    Ok(format!("{:x}", hasher.finalize()).into())
}

#[derive(Clone, Copy, Debug)]
pub struct Crc32;

impl Function for Crc32 {
    fn identifier(&self) -> &'static str {
        "crc32"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "crc32",
            source: r#"crc32("foobar")"#,
            result: Ok("9ef61f95"),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(Crc32Fn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct Crc32Fn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for Crc32Fn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        crc32(value)
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
        crc32 => Crc32;

        crc32 {
            args: func_args![value: "foo"],
            want: Ok(value!("8c736521")),
            tdef: TypeDef::bytes().infallible(),
        }
    ];
}
