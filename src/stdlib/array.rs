use crate::compiler::prelude::*;

fn array(value: Value) -> Resolved {
    match value {
        v @ Value::Array(_) => Ok(v),
        v => Err(format!("expected array, got {}", v.kind()).into()),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Array;

impl Function for Array {
    fn identifier(&self) -> &'static str {
        "array"
    }

    fn usage(&self) -> &'static str {
        "Returns `value` if it is an array, otherwise returns an error. This enables the type checker to guarantee that the returned value is an array and can be used in any function that expects an array."
    }

    fn category(&self) -> &'static str {
        Category::Type.as_ref()
    }

    fn internal_failure_reasons(&self) -> &'static [&'static str] {
        &["`value` is not an array."]
    }

    fn return_kind(&self) -> u16 {
        kind::ARRAY
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns the `value` if it's an array.",
            "Raises an error if not an array.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[Parameter {
            keyword: "value",
            kind: kind::ANY,
            required: true,
            description: "The value to check if it is an array.",
            default: None,
        }]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Declare an array type",
                source: indoc! {"
                    .value = [1, 2, 3]
                    array(.value)
                "},
                result: Ok("[1,2,3]"),
            },
            example! {
                title: "Valid array literal",
                source: "array([1,2,3])",
                result: Ok("[1,2,3]"),
            },
            example! {
                title: "Invalid type",
                source: "array!(true)",
                result: Err(
                    r#"function call error for "array" at (0:12): expected array, got boolean"#,
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

        Ok(ArrayFn { value }.as_expr())
    }
}

#[derive(Debug, Clone)]
struct ArrayFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for ArrayFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        array(self.value.resolve(ctx)?)
    }

    fn type_def(&self, state: &state::TypeState) -> TypeDef {
        self.value
            .type_def(state)
            .fallible_unless(Kind::array(Collection::any()))
            .restrict_array()
    }
}
