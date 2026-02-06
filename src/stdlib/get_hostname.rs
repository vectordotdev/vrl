use crate::compiler::prelude::*;

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(feature = "__mock_return_values_for_tests", allow(dead_code))]
fn get_hostname() -> Resolved {
    Ok(hostname::get()
        .map_err(|error| format!("failed to get hostname: {error}"))?
        .to_string_lossy()
        .into())
}

#[derive(Clone, Copy, Debug)]
pub struct GetHostname;

impl Function for GetHostname {
    fn identifier(&self) -> &'static str {
        "get_hostname"
    }

    fn usage(&self) -> &'static str {
        "Returns the local system's hostname."
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["Internal hostname resolution failed."]
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        _: ArgumentList,
    ) -> Compiled {
        Ok(GetHostnameFn.as_expr())
    }

    #[cfg(target_arch = "wasm32")]
    fn compile(
        &self,
        _state: &state::TypeState,
        ctx: &mut FunctionCompileContext,
        _: ArgumentList,
    ) -> Compiled {
        Ok(super::WasmUnsupportedFunction::new(ctx.span(), TypeDef::bytes().fallible()).as_expr())
    }

    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Get hostname",
            source: r#"get_hostname!() != """#,
            result: Ok("true"),
        }]
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Get hostname",
            source: r#"get_hostname!()"#,
            result: Ok("my-hostname"),
        }]
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone)]
struct GetHostnameFn;

#[cfg(not(target_arch = "wasm32"))]
impl FunctionExpression for GetHostnameFn {
    #[cfg(not(feature = "__mock_return_values_for_tests"))]
    fn resolve(&self, _: &mut Context) -> Resolved {
        get_hostname()
    }

    #[cfg(feature = "__mock_return_values_for_tests")]
    fn resolve(&self, _: &mut Context) -> Resolved {
        Ok("my-hostname".into())
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::bytes().fallible()
    }
}
