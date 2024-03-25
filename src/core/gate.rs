use anyhow::Result;
use async_process::Command;

use crate::core::{
    common::contextualize_command, executor::DigExecutor, run_context::RunContext,
    token::TokenedJsonValue, vars::VariableSet,
};

pub type RunGates = Vec<String>;

pub struct RunGateNonZeroExit {
    pub code: i32,
    pub statement: String,
}

pub async fn test_run_gate(
    statement: &String,
    vars: &VariableSet,
    context: &RunContext,
    executor: &DigExecutor<'_>,
) -> Result<Option<RunGateNonZeroExit>> {
    let statement = statement.evaluate_tokens_to_string("run gate", vars)?;

    let mut command = Command::new("bash");
    command.arg("-c");
    let _command = command.arg(format!("test {}", statement));
    contextualize_command(_command, context);

    // println!("LOCKING - {:?}", executor.limiter);
    let lock = executor.limiter.acquire().await;
    let output = command.output().await?;
    drop(lock);
    // println!("UNLOCKING");

    match output.status.code() {
        None => panic!("The test has been canceled"),
        Some(code) => match code {
            0 => Ok(None),
            nonzero => Ok(Some(RunGateNonZeroExit {
                code: nonzero,
                statement,
            })),
        },
    }
}

pub async fn test_run_gates(
    statements: Option<&RunGates>,
    vars: &VariableSet,
    context: &RunContext,
    executor: &DigExecutor<'_>,
) -> Result<Option<(usize, RunGateNonZeroExit)>> {
    match statements {
        None => Ok(None),
        Some(statements) => {
            // Test If statements
            let mut output = None;
            for (i, statement) in statements.iter().enumerate() {
                let run_gate_outcome = test_run_gate(statement, vars, context, executor).await?;

                output = run_gate_outcome.map(|v| (i, v));

                if output.is_some() {
                    break;
                }
            }
            Ok(output)
        }
    }
}
