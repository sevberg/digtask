use crate::{executor::DigExecutor, vars::VariableSet};
use anyhow::Result;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use super::common::{SingularStepConfig, StepEvaluationResult, StepMethods};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ParallelStepConfig {
    pub parallel: Vec<SingularStepConfig>,
}

impl StepMethods for ParallelStepConfig {
    async fn evaluate(
        &self,
        step_i: usize,
        vars: &VariableSet,
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        let mut tasks = Vec::new();
        for (_, step) in self.parallel.iter().enumerate() {
            tasks.push(step.evaluate(step_i, vars, executor))
        }
        let task_outcomes = join_all(tasks).await;

        let mut output = Vec::new();
        for outcome in task_outcomes.into_iter() {
            let outcome = match outcome {
                Ok(result) => match result {
                    StepEvaluationResult::SubmitTasks(tasks) => Some(tasks),
                    _ => None,
                },
                Err(error) => return Err(error),
            };

            match outcome {
                Some(tasks) => output.extend(tasks),
                None => {}
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
    use std::time::Duration;

    use anyhow::bail;
    use rstest::rstest;

    use crate::testing_block_on;

    use super::*;

    #[test]
    fn test_parallel_bash_steps() -> Result<()> {
        let step_config = ParallelStepConfig {
            parallel: vec![
                SingularStepConfig::Simple("whoami".into()),
                SingularStepConfig::Simple("pwd".into()),
            ],
        };
        let vars = VariableSet::new();
        let output = testing_block_on!(ex, step_config.evaluate(0, &vars, &ex))?;

        match output {
            StepEvaluationResult::CompletedWithOutput(val) => {
                assert_eq!(val, serde_json::Value::Null);
            }
            other => bail!("Expected an empty completion, instead got '{:?}'", other),
        };

        Ok(())
    }

    #[rstest]
    #[timeout(Duration::from_millis(300))]
    fn test_true_parallelism() -> Result<()> {
        let step_config = ParallelStepConfig {
            parallel: vec![
                SingularStepConfig::Simple("sleep 0.2".into()),
                SingularStepConfig::Simple("sleep 0.2".into()),
            ],
        };
        let vars = VariableSet::new();
        let output = testing_block_on!(ex, step_config.evaluate(0, &vars, &ex))?;

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
                SingularStepConfig::Simple("whoami".into()),
                SingularStepConfig::Simple("_this_is_an_expected_error_".into()), // <- not a real command
            ],
        };
        let vars = VariableSet::new();
        let output = testing_block_on!(ex, step_config.evaluate(0, &vars, &ex));

        match output {
            Ok(value) => bail!("Expected a failure, but instead got '{:?}'", value),
            Err(error) => {
                let expected_error = "/bin/bash: _this_is_an_expected_error_: command not found";
                assert_eq!(error.to_string(), expected_error);
            }
        };

        Ok(())
    }

    // #[test]
    // fn test_nested_parallel() -> Result<()> {
    //     let step_config = ParallelStepConfig {
    //         parallel: vec![StepConfig::Parallel(ParallelStepConfig {
    //             parallel: vec![
    //                 StepConfig::Simple("whoami".into()),
    //                 StepConfig::Simple("pwd".into()),
    //             ],
    //         })],
    //     };
    //     let output = step_config.evaluate(0, &no_vars());

    //     match output {
    //         Ok(value) => bail!("Expected a failure, but instead got '{:?}'", value),
    //         Err(error) => {
    //             assert_eq!(
    //                 error.to_string(),
    //                 "A parallel step cannot have nested parallel steps"
    //             )
    //         }
    //     };

    //     Ok(())
    // }
}
