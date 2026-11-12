use anyhow::Result;
use clap::{Parser, Subcommand};

mod deny;

#[derive(Parser)]
#[command(about = "VRL development tooling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Check the repository
    Check {
        #[command(subcommand)]
        command: Check,
    },
}

#[derive(Subcommand)]
enum Check {
    Deny(deny::Cli),
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Check {
            command: Check::Deny(cli),
        } => cli.exec(),
    }
}
