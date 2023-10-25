use crate::compiler::prelude::*;

fn int(value: Value) -> Resolved {
    match value {
        v @ Value::Integer(_) => Ok(v),
        v => Err(format!(r#"expected integer, got {}"#, v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Integer;

impl Function for Integer {
    fn identifier(&self) -> &'static str {
        "int"
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "valid",
                source: r#"int(42)"#,
                result: Ok("42"),
            },
            Example {
                title: "invalid",
                source: "int!(true)",
                result: Err(
                    r#"function call error for "int" at (0:10): expected integer, got boolean"#,
                ),
            },
        ]
    }

    fn compile(
        &self,
        _state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");

        Ok(IntegerFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct IntegerFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IntegerFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        int(self.value.resolve(ctx)?)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let non_integer = !self.value.type_def(state).is_integer();

        TypeDef::integer().maybe_fallible(non_integer)
    }
}
