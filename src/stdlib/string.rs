use crate::compiler::prelude::*;

fn string(value: Value) -> Resolved {
    match value {
        v @ Value::Bytes(_) => Ok(v),
        v => Err(format!("expected string, got {}", v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct String;

impl Function for String {
    fn identifier(&self) -> &'static str {
        "string"
    }

    fn usage(&self) -> &'static str {
        "Returns `value` if it is a string, otherwise returns an error. This enables the type checker to guarantee that the returned value is a string and can be used in any function that expects a string."
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to check if it is a string.",
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Declare a string type",
                source: indoc! {r#"
                    . = { "message": "{\"field\": \"value\"}" }
                    string(.message)
                "#},
                result: Ok(r#""{\"field\": \"value\"}""#),
            },
            example! {
                title: "Invalid type",
                source: "string!(true)",
                result: Err(
                    r#"function call error for "string" at (0:13): expected string, got boolean"#,
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

        Ok(StringFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct StringFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for StringFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        string(self.value.resolve(ctx)?)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let non_bytes = !self.value.type_def(state).is_bytes();

        TypeDef::bytes().maybe_fallible(non_bytes)
    }
}
