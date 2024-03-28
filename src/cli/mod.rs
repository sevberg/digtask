use clap::Subcommand;

use self::into::IntoArgs;

pub mod into;

#[derive(Debug, Subcommand)]
pub enum Commands {
    Into(IntoArgs),
}
