use clap::Parser;
use vrl::docs::{Opts, cmd::docs};

fn main() {
    std::process::exit(docs(&Opts::parse(), &vrl::stdlib::all()));
}
