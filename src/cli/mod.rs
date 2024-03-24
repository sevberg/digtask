use clap::Subcommand;

use self::run::RunArgs;

pub mod run;

#[derive(Debug, Subcommand)]
pub enum Commands {
    Run(RunArgs),
}
