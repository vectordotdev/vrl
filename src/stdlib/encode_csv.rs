use crate::compiler::prelude::*;

// TODO: armand do we include a `pretty` bool parameter? like in encode_json?
// TODO: armand we may need to include it in the benchs? I dont know what this is lol
fn encode_csv(value: &Value) -> Value {
    42.into()
}

#[derive(Clone, Copy, Debug)]
pub struct EncodeCsv;

// TODO: armand check if i implemented every needed method
impl Function for EncodeCsv {
    fn identifier(&self) -> &'static str {
        "encode_csv"
    }

    fn usage(&self) -> &'static str {
        "Encodes the `value` to CSV."
    }

    fn category(&self) -> &'static str {
        Category::Codec.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BYTES
    }

    fn examples(&self) -> &'static [Example] {
        todo!()
    }

    fn compile(
        &self,
        _state: &TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(EncodeCsvFn { value }.as_expr())
    }

    fn parameters(&self) -> &'static [Parameter] {
        const PARAMETERS: &[Parameter] = &[Parameter::required(
            "value",
            kind::ANY,
            "The value to convert to a CSV string.",
        )];
        PARAMETERS
    }
}

#[derive(Clone, Debug)]
struct EncodeCsvFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for EncodeCsvFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self
            .value
            .resolve(ctx)?;

        Ok(encode_csv(&value))
    }

    fn type_def(&self, _state: &TypeState) -> TypeDef {
        // TODO: armand i can't think about cases where this might fail. Have to check with the doc
        TypeDef::bytes().infallible()
    }
}
