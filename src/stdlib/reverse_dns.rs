use crate::compiler::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
mod non_wasm {
    use crate::compiler::prelude::*;
    use crate::value::Value;
    use dns_lookup::lookup_addr;
    use std::net::IpAddr;

    fn reverse_dns(value: Value) -> Resolved {
        let ip: IpAddr = value
            .try_bytes_utf8_lossy()?
            .parse()
            .map_err(|err| format!("unable to parse IP address: {err}"))?;
        let host = lookup_addr(&ip).map_err(|err| format!("unable to perform a lookup : {err}"))?;

        Ok(host.into())
    }

    #[derive(Debug, Clone)]
    pub(super) struct ReverseDnsFn {
        pub(super) value: Box<dyn Expression>,
    }

    impl FunctionExpression for ReverseDnsFn {
        fn resolve(&self, ctx: &mut Context) -> Resolved {
            let value = self.value.resolve(ctx)?;
            reverse_dns(value)
        }

        fn type_def(&self, _: &state::TypeState) -> TypeDef {
            TypeDef::bytes().fallible()
        }
    }
}

#[allow(clippy::wildcard_imports)]
#[cfg(not(target_arch = "wasm32"))]
use non_wasm::*;

#[derive(Clone, Copy, Debug)]
pub struct ReverseDns;

impl Function for ReverseDns {
    fn identifier(&self) -> &'static str {
        "reverse_dns"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::BYTES,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "Example",
            source: r#"reverse_dns!("127.0.0.1")"#,
            result: Ok("localhost"),
        }]
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(ReverseDnsFn { value }.as_expr())
    }

    #[cfg(target_arch = "wasm32")]
    fn compile(
        &self,
        _state: &state::TypeState,
        ctx: &mut FunctionCompileContext,
        _arguments: ArgumentList,
    ) -> Compiled {
        Ok(super::WasmUnsupportedFunction::new(ctx.span(), TypeDef::bytes().fallible()).as_expr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    test_function![
        reverse_dns => ReverseDns;

        invalid_ip {
            args: func_args![value: value!("999.999.999.999")],
            want: Err("unable to parse IP address: invalid IP address syntax"),
            tdef: TypeDef::bytes().fallible(),
        }

        google_ipv4 {
            args: func_args![value: value!("8.8.8.8")],
            want: Ok(value!("dns.google")),
            tdef: TypeDef::bytes().fallible(),
        }

        google_ipv6 {
            args: func_args![value: value!("2001:4860:4860::8844")],
            want: Ok(value!("dns.google")),
            tdef: TypeDef::bytes().fallible(),
        }

        invalid_type {
            args: func_args![value: value!(1)],
            want: Err("expected string, got integer"),
            tdef: TypeDef::bytes().fallible(),
        }
    ];
}
