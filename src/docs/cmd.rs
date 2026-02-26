use crate::compiler::Function;
use clap::Parser;
use std::path::PathBuf;
use std::{io};

use super::{build_functions_doc, document_functions_to_dir};

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
        #[allow(clippy::print_stdout)]
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
