mod config;
mod step;
mod task;
mod token;
mod vars;

use anyhow::Result;
use clap::Parser;
use config::RequeueConfig;
use vars::{no_vars, RawVariableMapTrait, VariableMap};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The config file to load
    #[arg(short, long, default_value = "requeue.yaml")]
    source: String,
    // /// Number of times to greet
    // #[arg(short, long, default_value_t = 1)]
    // count: u8,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let config = RequeueConfig::load_yaml(&args.source)?;
    let global_vars = match config.vars {
        None => VariableMap::new(),
        Some(rawvars) => rawvars.evaluate(&no_vars())?,
    };

    println!("{:?}", global_vars);

    Ok(())
}
