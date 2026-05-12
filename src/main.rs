use clap::Parser;
use vrl::cli::{Opts, cmd::cmd};

fn main() {
    std::process::exit(cmd(&Opts::parse(), vrl::stdlib::all()));
}
