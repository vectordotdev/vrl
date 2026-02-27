use crate::compiler::prelude::*;
use chrono::Utc;

#[derive(Clone, Copy, Debug)]
pub struct Now;

impl Function for Now {
    fn identifier(&self) -> &'static str {
        "now"
    }

    fn usage(&self) -> &'static str {
        "Returns the current timestamp in the UTC timezone with nanosecond precision."
    }

    fn category(&self) -> &'static str {
        Category::Timestamp.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::TIMESTAMP
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Generate a current timestamp",
            source: r#"now()"#,
            result: Ok("2012-03-04T12:34:56.789012345Z"),
            deterministic: false,
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        _: ArgumentList,
    ) -> Compiled {
        Ok(NowFn.as_expr())
    }
}

#[derive(Debug, Clone)]
struct NowFn;

impl FunctionExpression for NowFn {
    fn resolve(&self, _: &mut Context) -> Resolved {
        Ok(Utc::now().into())
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::timestamp()
    }
}
