use crate::compiler::Function;
use clap::Parser;
use std::io;
use std::path::PathBuf;

use super::{build_functions_doc, read_functions_from_file, write_function_docs_to_dir};

/// Vector Remap Language Docs
#[derive(Parser, Debug)]
#[command(name = "VRL", about)]
pub struct Opts {
    /// Output directory to create JSON files. If unspecified output is written to stdout as a JSON
    /// array
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Read function documentation from a docs.json file instead of building from VRL stdlib.
    /// The file should contain a top-level object with a `remap.functions` map.
    #[arg(short, long)]
    input: Option<PathBuf>,

    /// Whether to pretty-print or minify
    #[arg(short, long, default_value_t = false)]
    minify: bool,

    /// File extension for generated files
    #[arg(long, default_value = "json")]
    extension: String,
}

#[must_use]
pub fn docs(opts: &Opts, functions: &[Box<dyn Function>]) -> exitcode::ExitCode {
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

fn run(opts: &Opts, functions: &[Box<dyn Function>]) -> Result<(), io::Error> {
    let docs = if let Some(input) = &opts.input {
        read_functions_from_file(input)?
    } else {
        build_functions_doc(functions)
    };

    if let Some(output) = &opts.output {
        write_function_docs_to_dir(docs, output, &opts.extension)
    } else {
        #[allow(clippy::print_stdout)]
        if opts.minify {
            println!(
                "{}",
                serde_json::to_string(&docs).expect("FunctionDoc serialization should not fail")
            );
        } else {
            println!(
                "{}",
                serde_json::to_string_pretty(&docs)
                    .expect("FunctionDoc serialization should not fail")
            );
        }
        Ok(())
    }
}
