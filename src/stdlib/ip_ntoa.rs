use crate::compiler::prelude::*;
use std::{convert::TryInto, net::Ipv4Addr};

fn ip_ntoa(value: Value) -> Resolved {
    let i: u32 = value
        .try_integer()?
        .try_into()
        .map_err(|_| String::from("cannot convert to bytes: integer does not fit in u32"))?;

    Ok(Ipv4Addr::from(i).to_string().into())
}

#[derive(Clone, Copy, Debug)]
pub struct IpNtoa;

impl Function for IpNtoa {
    fn identifier(&self) -> &'static str {
        "ip_ntoa"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::INTEGER,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "Example",
            source: r#"ip_ntoa!(16909060)"#,
            result: Ok("1.2.3.4"),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(IpNtoaFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct IpNtoaFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IpNtoaFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        ip_ntoa(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        ip_ntoa => IpNtoa;

        invalid {
            args: func_args![value: i64::from(u32::MAX) + 1],
            want: Err("cannot convert to bytes: integer does not fit in u32"),
            tdef: TypeDef::bytes().fallible(),
        }

        valid {
            args: func_args![value: 16_909_060],
            want: Ok(value!("1.2.3.4")),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
