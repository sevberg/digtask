use crate::core::{
    executor::DigExecutor,
    run_context::RunContext,
    step::common::{SingularStepConfig, StepEvaluationResult, StepMethods},
    vars::VariableSet,
};
use anyhow::Result;
use futures::future::join_all;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ParallelStepConfig {
    pub parallel: Vec<SingularStepConfig>,
}

impl StepMethods for ParallelStepConfig {
    async fn evaluate(
        &self,
        step_i: usize,
        vars: &VariableSet,
        context: &RunContext,
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        let mut tasks = Vec::new();
        for step in self.parallel.iter() {
            tasks.push(step.evaluate(step_i, vars, context, executor))
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

            if let Some(tasks) = outcome {
                output.extend(tasks)
            }
        }

        match output.is_empty() {
            true => Ok(StepEvaluationResult::Completed("".to_string())),
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
        let context = RunContext::default();
        let output = testing_block_on!(ex, step_config.evaluate(0, &vars, &context, &ex))?;

        match output {
            StepEvaluationResult::Completed(val) => {
                assert_eq!(val, "".to_string());
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
        let context = RunContext::default();
        let output = testing_block_on!(ex, step_config.evaluate(0, &vars, &context, &ex))?;

        match output {
            StepEvaluationResult::Completed(val) => {
                assert_eq!(val, "".to_string());
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
                SingularStepConfig::Simple(">&2 echo \"This is an expected error\"; exit 1".into()), // <- not a real command
            ],
        };
        let vars = VariableSet::new();
        let context = RunContext::default();
        let output = testing_block_on!(ex, step_config.evaluate(0, &vars, &context, &ex));

        match output {
            Ok(value) => bail!("Expected a failure, but instead got '{:?}'", value),
            Err(error) => {
                let expected_error = "This is an expected error";
                assert_eq!(error.to_string(), expected_error);
            }
        };

        Ok(())
    }
}
