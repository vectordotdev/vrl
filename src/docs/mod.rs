#![deny(warnings, clippy::pedantic)]
pub mod cmd;

pub use cmd::{Opts, docs};

use crate::compiler::Function;
use crate::compiler::value::kind;
use crate::core::Value;
use crate::prelude::function::EnumVariant;
use crate::prelude::{Example, Parameter};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{fs, io};
use tracing::{debug, info};

#[derive(Serialize, Deserialize)]
#[allow(dead_code)]
pub struct FunctionDoc {
    pub anchor: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub arguments: Vec<ArgumentDoc>,
    pub r#return: ReturnDoc,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub internal_failure_reasons: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<ExampleDoc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    pub pure: bool,
    #[serde(default, skip_serializing)]
    deprecated: bool,
}

#[derive(Serialize, Deserialize)]
pub struct ArgumentDoc {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub r#type: Vec<String>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub r#enum: IndexMap<String, String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_default_value"
    )]
    pub default: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ReturnDoc {
    pub types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ExampleDoc {
    pub title: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#return: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "deserialize_raises")]
    pub raises: Option<String>,
    // docs.json-only fields: ignored during serialization
    #[serde(default, skip_serializing)]
    output: Option<serde_json::Value>,
    #[serde(default, skip_serializing)]
    skip_test: Option<bool>,
}

/// Writes function documentation files into `output_dir`
///
/// # Errors
/// - Failed to create `output_dir`.
/// - Failed to write or create file in `output_dir`.
/// - JSON serialization error.
///
/// # Panics
/// Will panic if any function's example has an input that is not valid JSON
pub fn document_functions_to_dir(
    functions: &[Box<dyn Function>],
    output_dir: &Path,
    extension: &str,
) -> io::Result<()> {
    write_function_docs_to_dir(build_functions_doc(functions), output_dir, extension)
}

/// Writes pre-built function docs into `output_dir`
///
/// # Errors
/// - Failed to create `output_dir`.
/// - Failed to write or create file in `output_dir`.
/// - JSON serialization error.
pub fn write_function_docs_to_dir(
    docs: Vec<FunctionDoc>,
    output_dir: &Path,
    extension: &str,
) -> io::Result<()> {
    fs::create_dir_all(output_dir)?;

    for doc in &docs {
        let filename = format!("{}.{extension}", doc.name);
        let filepath = output_dir.join(&filename);
        let mut json = serde_json::to_string_pretty(doc)?;
        json.push('\n');

        fs::write(&filepath, json)?;

        debug!(path = ?filepath.display(), "Generated file");
    }

    info!("VRL documentation generation complete.");
    Ok(())
}

/// Reads function documentation from a docs.json file (Vector website format).
///
/// Expects the file to contain a top-level object with a `remap.functions` map.
/// Applies format conversions:
/// - Unwraps `input` from the `{"log": {...}}` wrapper
/// - Extracts `raises` from `{"runtime": "..."}` object format
/// - Drops `deprecated`, `output`, and `skip_test` fields
///
/// # Errors
/// - Failed to read or parse the file.
pub fn read_functions_from_file(path: &Path) -> io::Result<Vec<FunctionDoc>> {
    let content = fs::read_to_string(path)?;
    let docs_json: DocsJson = serde_json::from_str(&content)?;
    let mut functions: Vec<FunctionDoc> = docs_json.remap.functions.into_values().collect();

    for func in &mut functions {
        for example in &mut func.examples {
            // Unwrap the {"log": {...}} wrapper from input
            example.input = example.input.take().and_then(|v| {
                if let serde_json::Value::Object(ref obj) = v {
                    if let Some(log_val) = obj.get("log") {
                        return Some(log_val.clone());
                    }
                }
                Some(v)
            });
        }
    }

    Ok(functions)
}

#[derive(Deserialize)]
struct DocsJson {
    remap: RemapSection,
}

#[derive(Deserialize)]
struct RemapSection {
    functions: IndexMap<String, FunctionDoc>,
}

/// Deserializes `default` from a string, integer, or boolean value.
fn deserialize_default_value<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    Ok(value.map(|v| match v {
        serde_json::Value::String(s) => s,
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }))
}

/// Deserializes `raises` from either a plain string or `{"runtime": "..."}` object.
fn deserialize_raises<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    Ok(value.and_then(|v| match v {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Object(ref obj) => obj
            .get("runtime")
            .and_then(serde_json::Value::as_str)
            .map(String::from),
        _ => None,
    }))
}

/// # Panics
/// Will panic if any function's example has an input that is not valid JSON
#[must_use]
pub fn build_functions_doc(functions: &[Box<dyn Function>]) -> Vec<FunctionDoc> {
    functions
        .iter()
        .map(|f| build_function_doc(f.as_ref()))
        .collect()
}

/// # Panics
/// Will panic if any function's example has an input that is not valid JSON
pub fn build_function_doc(func: &dyn Function) -> FunctionDoc {
    let name = func.identifier().to_string();

    let arguments: Vec<ArgumentDoc> = func
        .parameters()
        .iter()
        .map(|param| {
            let Parameter {
                keyword,
                kind,
                required,
                description,
                default,
                enum_variants,
            } = param;

            let name = keyword.trim().to_string();
            let description = description.trim().to_string();
            let default = default.map(pretty_value);
            let r#type = kind_to_types(*kind);
            let r#enum = enum_variants
                .unwrap_or_default()
                .iter()
                .map(|EnumVariant { value, description }| {
                    (value.to_string(), description.to_string())
                })
                .collect();

            ArgumentDoc {
                name,
                description,
                required: *required,
                r#type,
                default,
                r#enum,
            }
        })
        .collect();

    let examples: Vec<ExampleDoc> = func
        .examples()
        .iter()
        .map(|example| {
            let Example {
                title,
                source,
                result,
                input,
                file: _,
                line: _,
                deterministic: _,
            } = example;

            let (r#return, raises) = match result {
                Ok(result) => {
                    // Try to parse as JSON, otherwise treat as string
                    let value = serde_json::from_str(result)
                        .unwrap_or_else(|_| serde_json::Value::String(result.to_string()));
                    (Some(value), None)
                }
                Err(error) => (None, Some(error.to_string())),
            };

            let source = source.to_string();
            let title = title.to_string();
            let input = input
                .map(|s| serde_json::from_str(s).expect("VRL example input must be valid JSON"));
            ExampleDoc {
                title,
                source,
                input,
                r#return,
                raises,
                output: None,
                skip_test: None,
            }
        })
        .collect();

    FunctionDoc {
        anchor: name.clone(),
        name,
        category: func.category().to_string(),
        description: trim_str(func.usage()),
        arguments,
        r#return: ReturnDoc {
            types: kind_to_types(func.return_kind()),
            rules: trim_slice(func.return_rules()),
        },
        internal_failure_reasons: trim_slice(func.internal_failure_reasons()),
        examples,
        notices: trim_slice(func.notices()),
        pure: func.pure(),
        deprecated: false,
    }
}

fn kind_to_types(kind_bits: u16) -> Vec<String> {
    // All type bits combined
    if (kind_bits & kind::ANY) == kind::ANY {
        return vec!["any".to_string()];
    }

    let mut types = Vec::new();

    if (kind_bits & kind::BYTES) == kind::BYTES {
        types.push("string".to_string());
    }
    if (kind_bits & kind::INTEGER) == kind::INTEGER {
        types.push("integer".to_string());
    }
    if (kind_bits & kind::FLOAT) == kind::FLOAT {
        types.push("float".to_string());
    }
    if (kind_bits & kind::BOOLEAN) == kind::BOOLEAN {
        types.push("boolean".to_string());
    }
    if (kind_bits & kind::OBJECT) == kind::OBJECT {
        types.push("object".to_string());
    }
    if (kind_bits & kind::ARRAY) == kind::ARRAY {
        types.push("array".to_string());
    }
    if (kind_bits & kind::TIMESTAMP) == kind::TIMESTAMP {
        types.push("timestamp".to_string());
    }
    if (kind_bits & kind::REGEX) == kind::REGEX {
        types.push("regex".to_string());
    }
    if (kind_bits & kind::NULL) == kind::NULL {
        types.push("null".to_string());
    }

    assert!(!types.is_empty(), "kind_bits {kind_bits} produced no types");

    types
}

fn pretty_value(v: &Value) -> String {
    if let Value::Bytes(b) = v {
        str::from_utf8(b).map_or_else(|_| v.to_string(), String::from)
    } else {
        v.to_string()
    }
}

fn trim_str(s: &'static str) -> String {
    s.trim().to_string()
}

fn trim_slice(slice: &'static [&'static str]) -> Vec<String> {
    slice.iter().map(|s| s.trim().to_string()).collect()
}
