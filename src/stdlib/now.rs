use crate::compiler::prelude::*;
use chrono::Utc;

#[derive(Clone, Copy, Debug)]
pub struct Now;

impl Function for Now {
    fn identifier(&self) -> &'static str {
        "now"
    }

    fn examples(&self) -> &'static [Example] {
        &[Example {
            title: "now",
            source: r#"now() != """#,
            result: Ok("true"),
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
