use crate::compiler::prelude::*;
use crate::value;

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

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "variant",
                kind: kind::BYTES,
                required: false,
            },
        ]
    }

    fn examples(&self) -> &'static [Example] {
        &[
            Example {
                title: "object",
                source: r#"is_json("{}")"#,
                result: Ok("true"),
            },
            Example {
                title: "string",
                source: r#"is_json(s'"test"')"#,
                result: Ok("true"),
            },
            Example {
                title: "invalid",
                source: r#"is_json("}{")"#,
                result: Ok("false"),
            },
            Example {
                title: "exact_variant",
                source: r#"is_json("{}", variant: "object")"#,
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
            args: func_args![value: r#"{}"#],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        string {
            args: func_args![value: r#""test""#],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        invalid {
            args: func_args![value: r#"}{"#],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        exact_variant {
            args: func_args![value: r#"{}"#, variant: "object"],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        exact_variant_invalid {
            args: func_args![value: r#"123"#, variant: "null"],
            want: Ok(value!(false)),
            tdef: TypeDef::boolean().infallible(),
        }

        variant_with_spaces {
            args: func_args![value: r#"   []"#, variant: "array"],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().infallible(),
        }

        invalid_variant {
            args: func_args![value: r#"[]"#, variant: "invalid-variant"],
            want: Err(r#"invalid enum variant""#),
            tdef: TypeDef::boolean().infallible(),
        }

        invalid_variant_type {
            args: func_args![value: r#"[]"#, variant: 100],
            want: Err(r#"invalid enum variant""#),
            tdef: TypeDef::boolean().infallible(),
        }
    ];
}
