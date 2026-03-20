use std::process::ExitCode;

use clap::Parser;
use vrl::docs::{Opts, cmd::docs};

fn main() -> ExitCode {
    ExitCode::from(docs(&Opts::parse(), &vrl::stdlib::all()) as u8)
}
