use crate::compiler::prelude::*;

fn int(value: Value) -> Resolved {
    match value {
        v @ Value::Integer(_) => Ok(v),
        v => Err(format!("expected integer, got {}", v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Integer;

impl Function for Integer {
    fn identifier(&self) -> &'static str {
        "int"
    }

    fn usage(&self) -> &'static str {
        "Returns `value` if it is an integer, otherwise returns an error. This enables the type checker to guarantee that the returned value is an integer and can be used in any function that expects an integer."
    }

    fn category(&self) -> &'static str {
        Category::Type.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not an integer."]
    }

    fn return_kind(&self) -> u16 {
        kind::INTEGER
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns the `value` if it's an integer.",
            "Raises an error if not an integer.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to check if it is an integer.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Declare an integer type",
                source: indoc! {r#"
                    . = { "value": 42 }
                    int(.value)
                "#},
                result: Ok("42"),
            },
            example! {
                title: "Declare an integer type (literal)",
                source: "int(42)",
                result: Ok("42"),
            },
            example! {
                title: "Invalid integer type",
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
