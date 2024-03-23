mod config;
mod executor;
mod run_context;
mod step;
mod task;
mod token;
mod vars;

#[cfg(test)]
mod test_utils;

use anyhow::{anyhow, Result};
use clap::Parser;
use config::DigConfig;
use run_context::RunContext;
use serde_json::json;
use vars::{StackMode, VariableSet};

use crate::{executor::DigExecutor, task::ForcingContext};

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// The config file to load
    #[arg(short, long, default_value = "dig.yaml")]
    source: String,
    /// The task to run
    #[arg(default_value = "default")]
    task: String,
    /// Variables to override in the executed task. Can be given multiple times
    #[arg(short, long)]
    var: Vec<String>,
    /// Number of async "threads" to allow in parallel
    #[arg(short, long, default_value_t = 1)]
    processes: usize,
    /// The called task should be forced to run (and subtasks which inherit)
    #[arg(short, long, action)]
    force_first: bool,
    /// All tasks should be forced to run
    #[arg(short = 'F', long, action)]
    force_all: bool,
}

async fn evaluate_main_task(
    user_args: Args,
    config: DigConfig,
    vars: VariableSet,
    executor: &DigExecutor<'_>,
) -> Result<()> {
    // handle global variables
    let dummy_context = RunContext::default();
    let vars = match &config.vars {
        None => vars,
        Some(raw_vars) => {
            vars.stack_raw_variables(raw_vars, StackMode::CopyLocals, &dummy_context, executor)
                .await?
        }
    };

    // Begin execution
    let forcing = match user_args.force_all {
        true => ForcingContext::EverythingForced,
        false => match user_args.force_first {
            true => ForcingContext::ExplicitlyForced,
            false => ForcingContext::NotForced,
        },
    };
    let context = RunContext::new(&forcing, &config.env, &config.dir, &vars)?;

    let mut main_task = config
        .get_task(&user_args.task)?
        .prepare("main", &vars, StackMode::EmptyLocals, context, executor)
        .await?;

    main_task.evaluate(&config, false, executor).await?;

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let config = DigConfig::load_yaml(&args.source)?;

    // handle overrides
    let mut vars = VariableSet::new();
    for var in args.var.iter() {
        let (key, value) = var.split_once("=").ok_or(anyhow!(
            "A key value pair should be given as KEY=VALUE. Got '{}'",
            var
        ))?;
        let value = serde_json::from_str(value).unwrap_or(json!(value));
        vars.insert(key.to_string(), value);
    }

    println!("{:?}", vars);

    // Initialize Async runtime
    let executor = DigExecutor::new(args.processes);

    // Evaluate main task
    let future = evaluate_main_task(args, config, vars, &executor);
    let output = smol::block_on((&executor.executor).run(future))?;

    Ok(output)
}
