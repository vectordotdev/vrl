use crate::compiler::function::EnumVariant;
use crate::compiler::prelude::*;
use crate::value;

static VARIANT_ENUM: &[EnumVariant] = &[
    EnumVariant {
        value: "object",
        description: "JSON object - {}",
    },
    EnumVariant {
        value: "array",
        description: "JSON array - []",
    },
    EnumVariant {
        value: "string",
        description: "JSON-formatted string values wrapped with quote marks",
    },
    EnumVariant {
        value: "number",
        description: "Integer or float numbers",
    },
    EnumVariant {
        value: "bool",
        description: "True or false",
    },
    EnumVariant {
        value: "null",
        description: "Exact null value",
    },
];

static PARAMETERS: &[Parameter] = &[
    Parameter::required(
        "value",
        kind::BYTES,
        "The value to check if it is a valid JSON document.",
    ),
    Parameter::optional(
        "variant",
        kind::BYTES,
        "The variant of the JSON type to explicitly check for.",
    )
    .enum_variants(VARIANT_ENUM),
];

fn is_json(value: Value) -> Resolved {
    let bytes = value.try_bytes()?;

    match serde_json::from_slice::<'_, serde::de::IgnoredAny>(&bytes) {
        Ok(_) => Ok(value!(true)),
        Err(_) => Ok(value!(false)),
    }
}

fn is_json_with_variant(value: Value, variant: &Bytes) -> Resolved {
    let bytes = value.try_bytes()?;

    if serde_json::from_slice::<'_, serde::de::IgnoredAny>(&bytes).is_ok() {
        for c in bytes {
            return match c {
                // Search for the first non whitespace char
                b' ' | b'\n' | b'\t' | b'\r' => continue,
                b'{' => Ok(value!(variant.as_ref() == b"object")),
                b'[' => Ok(value!(variant.as_ref() == b"array")),
                b't' | b'f' => Ok(value!(variant.as_ref() == b"bool")),
                b'-' | b'0'..=b'9' => Ok(value!(variant.as_ref() == b"number")),
                b'"' => Ok(value!(variant.as_ref() == b"string")),
                b'n' => Ok(value!(variant.as_ref() == b"null")),
                _ => break,
            };
        }
    }

    Ok(value!(false))
}

fn variants() -> Vec<Value> {
    vec![
        value!("object"),
        value!("array"),
        value!("bool"),
        value!("number"),
        value!("string"),
        value!("null"),
    ]
}

#[derive(Clone, Copy, Debug)]
pub struct IsJson;

impl Function for IsJson {
    fn identifier(&self) -> &'static str {
        "is_json"
    }

    fn usage(&self) -> &'static str {
        "Check if the string is a valid JSON document."
    }

    fn category(&self) -> &'static str {
        Category::Type.as_ref()
    }

    fn return_kind(&self) -> u16 {
        kind::BOOLEAN
    }

    fn return_rules(&self) -> &'static [&'static str] {
        &[
            "Returns `true` if `value` is a valid JSON document.",
            "Returns `false` if `value` is not JSON-formatted.",
        ]
    }

    fn parameters(&self) -> &'static [Parameter] {
        PARAMETERS
    }

    fn examples(&self) -> &'static [Example] {
        &[
            example! {
                title: "Valid JSON object",
                source: r#"is_json("{}")"#,
                result: Ok("true"),
            },
            example! {
                title: "Non-valid value",
                source: r#"is_json("{")"#,
                result: Ok("false"),
            },
            example! {
                title: "Exact variant",
                source: r#"is_json("{}", variant: "object")"#,
                result: Ok("true"),
            },
            example! {
                title: "Non-valid exact variant",
                source: r#"is_json("{}", variant: "array")"#,
                result: Ok("false"),
            },
            example! {
                title: "Valid JSON string",
                source: r#"is_json(s'"test"')"#,
                result: Ok("true"),
            },
        ]
    }

    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let variant = arguments.optional_enum("variant", &variants(), state)?;

        match variant {
            Some(raw_variant) => {
                let variant = raw_variant
                    .try_bytes()
                    .map_err(|e| Box::new(e) as Box<dyn DiagnosticMessage>)?;
                Ok(IsJsonVariantsFn { value, variant }.as_expr())
            }
            None => Ok(IsJsonFn { value }.as_expr()),
        }
    }
}

#[derive(Clone, Debug)]
struct IsJsonFn {
    value: Box<dyn Expression>,
}

impl FunctionExpression for IsJsonFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        is_json(value)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[derive(Clone, Debug)]
struct IsJsonVariantsFn {
    value: Box<dyn Expression>,
    variant: Bytes,
}

impl FunctionExpression for IsJsonVariantsFn {
    fn resolve(&self, ctx: &mut Context) -> Resolved {
        let value = self.value.resolve(ctx)?;
        let variant = &self.variant;

        is_json_with_variant(value, variant)
    }

    fn type_def(&self, _: &state::TypeState) -> TypeDef {
        TypeDef::boolean().infallible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    test_function![
        is_json => IsJson;

        object {
            args: func_args![value: "{}"],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        string {
            args: func_args![value: r#""test""#],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        invalid {
            args: func_args![value: "}{"],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        exact_variant {
            args: func_args![value: "{}", variant: "object"],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        exact_variant_invalid {
            args: func_args![value: "123", variant: "null"],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        variant_with_spaces {
            args: func_args![value: "   []", variant: "array"],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        invalid_variant {
            args: func_args![value: "[]", variant: "invalid-variant"],
            want: Err(r#"invalid enum variant""#),
            tdef: TypeDef::boolean().infallible(),
        }

        invalid_variant_type {
            args: func_args![value: "[]", variant: 100],
            want: Err(r#"invalid enum variant""#),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
