#![deny(warnings, clippy::pedantic)]
pub mod cmd;
mod repl;

use crate::compiler::runtime::Terminate;
pub use cmd::{cmd, Opts};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {}", .0)]
    Io(#[from] std::io::Error),

    // this is the set of rendered end-user diagnostic errors when a VRL program fails to compile
    #[error("{}", .0)]
    Parse(String),

    #[error(transparent)]
    Runtime(#[from] Terminate),

    #[error("input error: {}", .0)]
    Json(#[from] serde_json::Error),

    #[error("repl feature disabled, program input required")]
    ReplFeature,

    #[error("error setting up readline: {}", .0)]
    Readline(#[from] rustyline::error::ReadlineError),
}
