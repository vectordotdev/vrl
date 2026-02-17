use crate::compiler::prelude::*;

fn object(value: Value) -> Resolved {
    match value {
        v @ Value::Object(_) => Ok(v),
        v => Err(format!("expected object, got {}", v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Object;

impl Function for Object {
    fn identifier(&self) -> &'static str {
        "object"
    }

    fn usage(&self) -> &'static str {
        "Returns `value` if it is an object, otherwise returns an error. This enables the type checker to guarantee that the returned value is an object and can be used in any function that expects an object."
    }

    fn category(&self) -> &'static str {
        Category::Type.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not an object."]
    }

    fn return_kind(&self) -> u16 {
        kind::OBJECT
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns the `value` if it's an object.",
            "Raises an error if not an object.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to check if it is an object.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Declare an object type",
                source: indoc! {r#"
                    . = { "value": { "field1": "value1", "field2": "value2" } }
                    object(.value)
                "#},
                result: Ok(r#"{ "field1": "value1", "field2": "value2" }"#),
            },
            example! {
                title: "Invalid type",
                source: "object!(true)",
                result: Err(
                    r#"function call error for "object" at (0:13): expected object, got boolean"#,
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

        Ok(ObjectFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ObjectFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ObjectFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        object(self.value.resolve(ctx)?)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        self.value
            .type_def(state)
            .fallible_unless(Kind::object(Collection::any()))
            .restrict_object()
    }
}
