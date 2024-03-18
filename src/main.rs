mod config;
mod step;
mod task;
mod token;
mod vars;

use anyhow::{anyhow, Result};
use clap::Parser;
use config::RequeueConfig;
use serde_json::json;
use vars::{no_vars, RawVariableMapTrait, VariableMap};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The config file to load
    #[arg(short, long, default_value = "requeue.yaml")]
    source: String,
    /// The task to run
    #[arg(short, long)]
    task: String,
    /// Variables to override in the executed task
    #[arg(short, long)]
    var: Vec<String>,
    // /// Number of times to greet
    // #[arg(short, long, default_value_t = 1)]
    // count: u8,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let config = RequeueConfig::load_yaml(&args.source)?;

    // handle overrides
    let mut var_overrides = VariableMap::new();
    for var in args.var.iter() {
        let (key, value) = var.split_once("=").ok_or(anyhow!(
            "A key value pair should be given as KEY=VALUE. Got '{}'",
            var
        ))?;
        let value = serde_json::from_str(value).unwrap_or(json!(value));
        var_overrides.insert(key.to_string(), value);
    }

    println!("{:?}", var_overrides);
    // handle global variables
    let global_vars = match &config.vars {
        None => VariableMap::new(),
        Some(rawvars) => rawvars.evaluate(&no_vars(), &var_overrides, true)?,
    };
    println!("{:?}", global_vars);

    let main_task = config.get_task(&args.task)?;

    let var_stack = vec![&global_vars];
    main_task
        .prepare("main", &var_stack, &var_overrides)?
        .evaluate(&var_stack, &config)?;

    Ok(())
}
