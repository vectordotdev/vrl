use crate::compiler::prelude::*;
use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;

// This needs to be static because validate_json_schema needs to read a file
// and the file path needs to be a literal.
static EXAMPLE_JSON_SCHEMA_EXPR: LazyLock<&str> = LazyLock::new(|| {
    let path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../tests/data/jsonschema/validate_json_schema/schema_with_email_format.json")
        .display()
        .to_string();

    Box::leak(
        format!(r#"validate_json_schema!(s'{{ "productUser": "foo@bar.com" }}', "{path}", false)"#)
            .into_boxed_str(),
    )
});

static EXAMPLES: LazyLock<Vec<Example>> = LazyLock::new(|| {
    vec![Example {
        title: "valid payload",
        source: &EXAMPLE_JSON_SCHEMA_EXPR,
        result: Ok("true"),
    }]
});

#[cfg(not(target_arch = "wasm32"))]
use non_wasm::ValidateJsonSchemaFn;
#[derive(Clone, Copy, Debug)]
pub struct ValidateJsonSchema;

impl Function for ValidateJsonSchema {
    fn identifier(&self) -> &'static str {
        "validate_json_schema"
    }

    fn examples(&self) -> &'static [Example] {
        EXAMPLES.as_slice()
    }

    fn parameters(&self) -> &'static [Parameter] {
        &[
            Parameter {
                keyword: "value",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "schema_definition",
                kind: kind::BYTES,
                required: true,
            },
            Parameter {
                keyword: "ignore_unknown_formats",
                kind: kind::BOOLEAN,
                required: false,
            },
        ]
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn compile(
        &self,
        state: &state::TypeState,
        _ctx: &mut FunctionCompileContext,
        arguments: ArgumentList,
    ) -> Compiled {
        let value = arguments.required("value");
        let schema_definition = arguments.required_literal("schema_definition", state)?;
        let ignore_unknown_formats = arguments
            .optional("ignore_unknown_formats")
            .unwrap_or(expr!(false));

        let schema_file_str = schema_definition
            .try_bytes_utf8_lossy()
            .expect("schema definition file must be a string");

        let schema_file_path = std::path::Path::new(schema_file_str.as_ref());

        Ok(ValidateJsonSchemaFn {
            value,
            schema_path: PathBuf::from(schema_file_path),
            ignore_unknown_formats,
        }
        .as_expr())
    }

    #[cfg(target_arch = "wasm32")]
    fn compile(
        &self,
        _state: &state::TypeState,
        ctx: &mut FunctionCompileContext,
        _arguments: ArgumentList,
    ) -> Compiled {
        Ok(super::WasmUnsupportedFunction::new(ctx.span(), TypeDef::bytes().fallible()).as_expr())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod non_wasm {
    use super::{
        Context, Expression, FunctionExpression, Resolved, TypeDef, VrlValueConvert, state,
    };
    use crate::prelude::ExpressionError;
    use crate::stdlib::json_utils::bom::StripBomFromUTF8;
    use crate::value;
    use jsonschema;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, LazyLock, RwLock};

    // Global cache for compiled schema validators, this allows us to reuse the compiled
    // schema across multiple calls to the function, which is important for performance.
    static SCHEMA_CACHE: LazyLock<RwLock<HashMap<PathBuf, Arc<jsonschema::Validator>>>> =
        LazyLock::new(|| RwLock::new(HashMap::new()));

    #[derive(Debug, Clone)]
    pub(super) struct ValidateJsonSchemaFn {
        pub(super) value: Box<dyn Expression>,
        pub(super) schema_path: PathBuf, // Path to the schema file, also used as cache key
        pub(super) ignore_unknown_formats: Box<dyn Expression>,
    }

    impl FunctionExpression for ValidateJsonSchemaFn {
        fn resolve(&self, ctx: &mut Context) -> Resolved {
            let value = self.value.resolve(ctx)?;
            let ignore_unknown_formats = self.ignore_unknown_formats.resolve(ctx)?.try_boolean()?;

            // Get bytes without extra allocation if possible
            let bytes = value.try_bytes()?;
            let stripped_bytes = bytes.strip_bom();

            // Quick empty check
            if bytes.is_empty() {
                return Err(ExpressionError::from("Empty JSON value")); // Empty JSON is typically invalid
            }

            // Fast path: check if it's valid JSON first (cheaper than full parsing)
            let json_value = if stripped_bytes.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::from_slice(stripped_bytes).map_err(|e| format!("Invalid JSON: {e}"))?
            };

            let schema_validator =
                get_or_compile_schema(&self.schema_path, ignore_unknown_formats)?;

            let validation_errors = schema_validator
                .iter_errors(&json_value)
                .map(|e| {
                    format!(
                        "{} at {}",
                        e,
                        if e.instance_path.as_str().is_empty() {
                            "/"
                        } else {
                            e.instance_path.as_str()
                        }
                    )
                })
                .collect::<Vec<String>>()
                .join(", ");

            if validation_errors.is_empty() {
                Ok(value!(true))
            } else {
                Err(ExpressionError::from(format!(
                    "JSON schema validation failed: {validation_errors}"
                )))
            }
        }

        fn type_def(&self, _: &state::TypeState) -> TypeDef {
            TypeDef::boolean().fallible()
        }
    }

    // Reads the JSON schema definition from a file and returns it as a serde_json::Value.
    // Returns an error if the file cannot be read or parsed.
    // The path must be a literal string.
    // This function is used to load the schema definition for the validate_json_schema function.
    // it will not fetch remote references, so the schema must be self-contained.
    pub(super) fn get_json_schema_definition(path: &Path) -> Result<serde_json::Value, String> {
        let b = std::fs::read(path).map_err(|e| {
            format!(
                "Failed to open schema definition file '{}': {e}",
                path.display()
            )
        })?;
        let schema: serde_json::Value = serde_json::from_slice(&b).map_err(|e| {
            format!(
                "Failed to parse schema definition file '{}': {e}",
                path.display()
            )
        })?;
        Ok(schema)
    }

    pub(super) fn get_or_compile_schema(
        schema_path: &Path,
        ignore_unknown_formats: bool,
    ) -> Result<Arc<jsonschema::Validator>, String> {
        // Try read lock first
        {
            let cache = SCHEMA_CACHE.read().unwrap();
            if let Some(schema) = cache.get(schema_path) {
                return Ok(schema.clone());
            }
        }

        // Need to compile - get write lock
        let mut cache = SCHEMA_CACHE.write().unwrap();

        // Double-check pattern
        if let Some(schema) = cache.get(schema_path) {
            return Ok(schema.clone());
        }

        let schema_definition = get_json_schema_definition(schema_path)
            .map_err(|e| format!("JSON schema not found: {e}"))?;

        // Compile schema
        let compiled_schema = jsonschema::options()
            .should_validate_formats(true)
            .should_ignore_unknown_formats(ignore_unknown_formats)
            .build(&schema_definition)
            .map_err(|e| format!("Failed to compile schema: {e}"))?;

        let compiled_schema = Arc::new(compiled_schema);
        cache.insert(schema_path.to_path_buf(), compiled_schema.clone());
        Ok(compiled_schema)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value;

    fn test_data_dir() -> PathBuf {
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("tests/data/jsonschema/")
    }

    test_function![
        validate_json_schema => ValidateJsonSchema;

        valid_with_email_format_json {
            args: func_args![
                value: value!("{\"productUser\":\"email@domain.com\"}"),
                schema_definition: test_data_dir().join("validate_json_schema/schema_with_email_format.json").to_str().unwrap().to_owned(),
                ignore_unknown_formats: false],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().fallible(),
        }

        valid_with_array_of_things_json {
            args: func_args![
                value: value!("{\"fruits\":[\"apple\",\"orange\",\"pear\"],\"vegetables\":[{\"veggieName\":\"potato\",\"veggieLike\":true},{\"veggieName\":\"broccoli\",\"veggieLike\":false}]}"),
                schema_definition: test_data_dir().join("validate_json_schema/schema_arrays_of_things.json").to_str().unwrap().to_owned(),
                ignore_unknown_formats: false],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().fallible(),
        }

        invalid_email_json {
            args: func_args![
                value: value!("{\"productUser\":\"invalid-email\"}"),
                schema_definition: test_data_dir().join("validate_json_schema/schema_with_email_format.json").to_str().unwrap().to_owned(),
                ignore_unknown_formats: false],
            want: Err("JSON schema validation failed: \"invalid-email\" is not a \"email\" at /productUser"),
            tdef: TypeDef::boolean().fallible(),
        }

        custom_format_ignored_json {
            args: func_args![
                value: value!("{\"productUser\":\"just-a-string\"}"),
                schema_definition: test_data_dir().join("validate_json_schema/schema_with_custom_format.json").to_str().unwrap().to_owned(),
                ignore_unknown_formats: true],
            want: Ok(value!(true)),
            tdef: TypeDef::boolean().fallible(),
        }

        invalid_empty_json {
            args: func_args![
                value: value!(""),
                schema_definition: test_data_dir().join("validate_json_schema/schema_with_email_format.json").to_str().unwrap().to_owned(),
                ignore_unknown_formats: false],
            want: Err("Empty JSON value"),
            tdef: TypeDef::boolean().fallible(),
        }

    ];
}
