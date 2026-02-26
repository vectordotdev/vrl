use crate::compiler::Function;
use crate::compiler::value::kind;
use crate::core::Value;
use crate::prelude::function::EnumVariant;
use crate::prelude::{Example, Parameter};
use clap::Parser;
use indexmap::IndexMap;
use serde::Serialize;
use std::path::PathBuf;
use std::{fs, io, path::Path};
use tracing::{debug, info};

#[derive(Serialize)]
pub struct FunctionDoc {
    pub anchor: String,
    pub name: String,
    pub category: String,
    pub description: String,
    pub arguments: Vec<ArgumentDoc>,
    pub r#return: ReturnDoc,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub internal_failure_reasons: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<ExampleDoc>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<String>,
    pub pure: bool,
}

#[derive(Serialize)]
pub struct ArgumentDoc {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub r#type: Vec<String>,
    #[serde(skip_serializing_if = "IndexMap::is_empty")]
    pub r#enum: IndexMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

#[derive(Serialize)]
pub struct ReturnDoc {
    pub types: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<String>,
}

#[derive(Serialize)]
pub struct ExampleDoc {
    pub title: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#return: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raises: Option<String>,
}

pub fn document_functions_to_dir(
    functions: &[Box<dyn Function>],
    output_dir: &Path,
) -> io::Result<()> {
    // Ensure output directory exists
    fs::create_dir_all(output_dir)?;

    for doc in build_functions_doc(functions) {
        let filename = format!("{}.cue", doc.name);
        let filepath = output_dir.join(&filename);
        let mut json = serde_json::to_string_pretty(&doc)?;
        json.push('\n');

        fs::write(&filepath, json)?;

        debug!(path = ?filepath.display(), "Generated file");
    }

    info!("VRL documentation generation complete.");
    Ok(())
}

pub fn build_functions_doc(functions: &[Box<dyn Function>]) -> Vec<FunctionDoc> {
    functions.iter().map(|f| build_function_doc(f)).collect()
}

pub fn build_function_doc(func: &Box<dyn Function>) -> FunctionDoc {
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

/// Vector Remap Language Docs
#[derive(Parser, Debug)]
#[command(name = "VRL", about)]
pub struct Opts {
    /// Output directory to create JSON files. If unspecified output is written to stdout as a JSON
    /// array
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Whether to pretty-print or minify
    #[arg(short, long, default_value_t = false)]
    minify: bool,
}

#[must_use]
pub fn docs(opts: &Opts, functions: Vec<Box<dyn Function>>) -> exitcode::ExitCode {
    match run(opts, functions) {
        Ok(()) => exitcode::OK,
        Err(err) => {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("{err}");
            }
            exitcode::SOFTWARE
        }
    }
}

fn run(opts: &Opts, functions: Vec<Box<dyn Function>>) -> Result<(), io::Error> {
    if let Some(output) = &opts.output {
        document_functions_to_dir(functions.as_slice(), output)
    } else {
        if opts.minify {
            println!(
                "{}",
                serde_json::to_string(&build_functions_doc(&functions))
                    .expect("FunctionDoc serialization should not fail")
            );
        } else {
            println!(
                "{}",
                serde_json::to_string_pretty(&build_functions_doc(&functions))
                    .expect("FunctionDoc serialization should not fail")
            );
        }
        Ok(())
    }
}
