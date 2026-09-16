#![deny(warnings)]

use std::process::ExitCode;

use clap::Parser;
use vrl::docs::{Opts, cmd::docs};

fn main() -> ExitCode {
    let code = docs(&Opts::parse(), &vrl::stdlib::all());
    ExitCode::from(u8::try_from(code).expect("exitcode values must fit in a u8"))
}
