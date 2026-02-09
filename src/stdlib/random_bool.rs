use crate::compiler::prelude::*;
use rand::random;

#[allow(clippy::unnecessary_wraps)] // match other VRL function implementations
fn random_bool() -> Resolved {
    let b: bool = random();

    Ok(Value::Boolean(b))
}

#[derive(Clone, Copy, Debug)]
pub struct RandomBool;

impl Function for RandomBool {
    fn identifier(&self) -> &'static str {
        "random_bool"
    }

    fn usage(&self) -> &'static str {
        "Returns a random boolean."
    }

    fn category(&self) -> &'static str {
        Category::Random.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[]
    }

    fn examples(&self) -> &'static [Example] {
        &[example! {
            title: "Random boolean",
            source: "is_boolean(random_bool())",
            result: Ok("true"),
        }]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        _arguments: ArgumentList,
    ) -> Compiled {
        Ok(RandomBoolFn {}.as_expr())
    }
}

#[derive(Debug, Clone)]
struct RandomBoolFn {}

impl FunctionExpression for RandomBoolFn {
    fn resolve(&self, _ctx: &mut Context) -> Resolved {
        random_bool()
    }

    fn type_def(&self, _state: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    // tests are handled by examples
}
