mod cli;
mod core;

#[cfg(test)]
mod test;

use anyhow::Result;
use clap::Parser;
use cli::run;

use crate::cli::Commands;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
struct MainArgs {
    #[command(subcommand)]
    command: Commands,
}

fn main() -> Result<()> {
    let cli = MainArgs::parse();

    match cli.command {
        Commands::Run(args) => run::main(args),
    }
}
