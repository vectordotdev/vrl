use clap::Parser;
use vrl::docs::{cmd::docs, Opts};

fn main() {
    std::process::exit(docs(&Opts::parse(), vrl::stdlib::all()));
}
