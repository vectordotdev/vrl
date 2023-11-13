use crate::compiler::prelude::*;

#[derive(Debug, Clone)]
pub struct WasmUnsupportedFunction {
    span: Span,
    type_def: TypeDef,
}

impl WasmUnsupportedFunction {
    #[must_use]
    pub fn new(span: Span, type_def: TypeDef) -> Self {
        Self { span, type_def }
    }
}

impl FunctionExpression for WasmUnsupportedFunction {
    fn resolve(&self, _: &mut Context) -> Resolved {
        Err(ExpressionError::Abort {
            span: self.span,
            message: Some("This function is not supported in WebAssembly".to_owned()),
        })
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        self.type_def.clone()
    }
}
