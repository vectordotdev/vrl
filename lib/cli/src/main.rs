use clap::Parser;
use vrl::cli::{cmd::cmd, Opts};

fn main() {
    std::process::exit(cmd(&Opts::parse(), vrl::stdlib::all()));
}
