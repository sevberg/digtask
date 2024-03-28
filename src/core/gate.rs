use anyhow::{anyhow, Result};
use async_process::Command;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::core::{
    common::contextualize_command, executor::DigExecutor, run_context::RunContext,
    token::TokenedJsonValue, vars::VariableSet,
};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct RunGateTestConfig {
    test: String,
    allow: Option<Vec<usize>>,
    deny: Option<Vec<usize>>,
}

impl RunGateTestConfig {
    pub async fn evaluate(
        &self,
        vars: &VariableSet,
        context: &RunContext,
        executor: &DigExecutor<'_>,
    ) -> Result<Option<RunGateNonZeroExit>> {
        let statement = self.test.evaluate_tokens_to_string("test-gate", vars)?;

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
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(untagged, rename_all = "kebab-case")]
pub enum RunGate {
    Internal(String),
    Test(RunGateTestConfig),
}

impl From<&str> for RunGate {
    fn from(value: &str) -> Self {
        RunGate::Internal(value.to_string())
    }
}

impl RunGate {
    pub async fn evaluate(
        &self,
        vars: &VariableSet,
        context: &RunContext,
        executor: &DigExecutor<'_>,
    ) -> Result<Option<RunGateNonZeroExit>> {
        match &self {
            RunGate::Internal(entry) => RunGate::evaluate_internal(entry, vars),
            RunGate::Test(test_config) => test_config.evaluate(vars, context, executor).await,
        }
    }

    fn evaluate_internal(entry: &str, vars: &VariableSet) -> Result<Option<RunGateNonZeroExit>> {
        let mut entry_split: Vec<&str> = entry.splitn(2, '=').collect();

        let rhs = entry_split
            .pop()
            .expect("An If-statement should have at least one element")
            .trim()
            .evaluate_tokens(vars)?;

        let lhs = match entry_split.pop() {
            Some(val) => val.trim().evaluate_tokens(vars)?,
            None => json!(true),
        };

        if !entry_split.is_empty() {
            return Err(anyhow!(
                "An If-Statement should only be splittable along one '=' character"
            ));
        }

        if lhs != rhs {
            let statement = format!("{} = {}", lhs, rhs);
            Ok(Some(RunGateNonZeroExit { code: 1, statement }))
        } else {
            Ok(None)
        }
    }
}
pub type RunGates = Vec<RunGate>;

pub struct RunGateNonZeroExit {
    pub code: i32,
    pub statement: String,
}

// pub async fn test_run_gate(
//     statement: &String,
//     vars: &VariableSet,
//     context: &RunContext,
//     executor: &DigExecutor<'_>,
// ) -> Result<Option<RunGateNonZeroExit>> {
//     let statement = statement.evaluate_tokens_to_string("run gate", vars)?;

//     let mut command = Command::new("bash");
//     command.arg("-c");
//     let _command = command.arg(format!("test {}", statement));
//     contextualize_command(_command, context);

//     // println!("LOCKING - {:?}", executor.limiter);
//     let lock = executor.limiter.acquire().await;
//     let output = command.output().await?;
//     drop(lock);
//     // println!("UNLOCKING");

//     match output.status.code() {
//         None => panic!("The test has been canceled"),
//         Some(code) => match code {
//             0 => Ok(None),
//             nonzero => Ok(Some(RunGateNonZeroExit {
//                 code: nonzero,
//                 statement,
//             })),
//         },
//     }
// }

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
                let run_gate_outcome = statement.evaluate(vars, context, executor).await?;

                output = run_gate_outcome.map(|v| (i, v));

                if output.is_some() {
                    break;
                }
            }
            Ok(output)
        }
    }
}
