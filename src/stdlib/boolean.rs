use crate::compiler::prelude::*;

fn boolean(value: Value) -> Resolved {
    match value {
        v @ Value::Boolean(_) => Ok(v),
        v => Err(format!("expected boolean, got {}", v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Boolean;

impl Function for Boolean {
    fn identifier(&self) -> &'static str {
        "bool"
    }

    fn usage(&self) -> &'static str {
        "The value to check if it is a Boolean."
    }

    fn category(&self) -> &'static str {
        Category::Type.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not a Boolean."]
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns `value` if it's a Boolean.",
            "Raises an error if not a Boolean.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to check if it is a Boolean.",
            default: None,
            enum_variants: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Valid Boolean",
                source: "bool(false)",
                result: Ok("false"),
            },
            example! {
                title: "Invalid Boolean",
                source: "bool!(42)",
                result: Err(
                    r#"function call error for "bool" at (0:9): expected boolean, got integer"#,
                ),
            },
            example! {
                title: "Valid Boolean from path",
                source: indoc! {r#"
                    . = { "value": true }
                    bool(.value)
                "#},
                result: Ok("true"),
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

        Ok(BooleanFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct BooleanFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for BooleanFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        boolean(self.value.resolve(ctx)?)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        let non_boolean = !self.value.type_def(state).is_boolean();

        TypeDef::boolean().maybe_fallible(non_boolean)
    }
}
