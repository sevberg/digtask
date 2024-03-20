use crate::vars::VariableMapStack;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::common::{StepConfig, StepEvaluationResult, StepMethods};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ParallelStepConfig {
    pub parallel: Vec<StepConfig>,
}

impl StepMethods for ParallelStepConfig {
    fn evaluate(
        &self,
        step_i: usize,
        var_stack: &VariableMapStack,
    ) -> Result<StepEvaluationResult> {
        let mut output = Vec::new();
        for (_, step) in self.parallel.iter().enumerate() {
            let outcome = match step {
                StepConfig::Parallel(_) => {
                    Err(anyhow!("A parallel step cannot have nested parallel steps"))
                }
                other => match other.evaluate(step_i, var_stack)? {
                    // Ok(result) => match result {
                    StepEvaluationResult::SubmitTasks(tasks) => Ok(Some(tasks)),
                    _ => Ok(None),
                    // },
                    // Err(error) => Err(error),
                },
            };

            match outcome {
                Ok(Some(tasks)) => output.extend(tasks),
                Ok(None) => {}
                Err(error) => return Err(error), // mention something about the parallel step index?
            }
        }

        match output.is_empty() {
            true => Ok(StepEvaluationResult::CompletedWithOutput(JsonValue::Null)),
            false => Ok(StepEvaluationResult::SubmitTasks(output)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::vars::no_vars;

    use super::*;

    #[test]
    fn test_parallel_bash_steps() -> Result<()> {
        let step_config = ParallelStepConfig {
            parallel: vec![
                StepConfig::Simple("whoami".into()),
                StepConfig::Simple("pwd".into()),
            ],
        };
        let output = step_config.evaluate(0, &no_vars())?;

        match output {
            StepEvaluationResult::CompletedWithOutput(val) => {
                assert_eq!(val, serde_json::Value::Null);
            }
            other => bail!("Expected an empty completion, instead got '{:?}'", other),
        };

        Ok(())
    }

    #[test]
    fn test_parallel_with_failure() -> Result<()> {
        let step_config = ParallelStepConfig {
            parallel: vec![
                StepConfig::Simple("whoami".into()),
                StepConfig::Simple("_this_is_an_expected_error_".into()), // <- not a real command
            ],
        };
        let output = step_config.evaluate(0, &no_vars());

        match output {
            Ok(value) => bail!("Expected a failure, but instead got '{:?}'", value),
            Err(error) => {
                let expected_error = "/bin/bash: _this_is_an_expected_error_: command not found";
                assert_eq!(error.to_string(), expected_error);
            }
        };

        Ok(())
    }

    #[test]
    fn test_nested_parallel() -> Result<()> {
        let step_config = ParallelStepConfig {
            parallel: vec![StepConfig::Parallel(ParallelStepConfig {
                parallel: vec![
                    StepConfig::Simple("whoami".into()),
                    StepConfig::Simple("pwd".into()),
                ],
            })],
        };
        let output = step_config.evaluate(0, &no_vars());

        match output {
            Ok(value) => bail!("Expected a failure, but instead got '{:?}'", value),
            Err(error) => {
                assert_eq!(
                    error.to_string(),
                    "A parallel step cannot have nested parallel steps"
                )
            }
        };

        Ok(())
    }
}
