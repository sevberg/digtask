use anyhow::Result;
use async_recursion::async_recursion;
use futures::future::join_all;
use serde::Deserialize;

use crate::{
    config::RequeueConfig,
    executor::DigExecutor,
    step::{
        common::{StepConfig, StepEvaluationResult, StepMethods},
        task_step::PreparedTaskStep,
    },
    token::TokenedJsonValue,
    vars::{RawVariableMap, StackMode, VariableMap, VariableMapStack, VariableSet},
};

use colored::Colorize;

fn default_false() -> bool {
    true
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum ForcingContext {
    NotForced,
    ExplicitlyForced,
    ParentIsForced,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum ForcingBehaviour {
    Never,
    Always,
    Inherit,
}
fn default_forcing() -> ForcingBehaviour {
    ForcingBehaviour::Inherit
}

#[derive(Deserialize, Debug)]
pub struct TaskConfig {
    pub label: Option<String>,
    pub steps: Vec<StepConfig>, // Vec<TaskStep>,
    pub inputs: Option<Vec<String>>,
    pub outputs: Option<Vec<String>>,
    #[serde(default = "default_false")]
    pub silent: bool,
    pub vars: Option<RawVariableMap>,
    #[serde(default = "default_forcing")]
    pub forcing: ForcingBehaviour,
}

impl TaskConfig {
    pub async fn prepare<'a>(
        &'a self,
        default_label: &str,
        vars: &VariableSet,
        executor: &DigExecutor<'_>,
    ) -> Result<PreparedTask> {
        let vars = match &self.vars {
            None => vars.stack(StackMode::EmptyLocals),
            Some(raw_vars) => {
                vars.stack_raw_variables(raw_vars, StackMode::EmptyLocals, executor)
                    .await?
            }
        };

        let label = match &self.label {
            Some(val) => val.evaluate_tokens_to_string("label", &vars)?,
            None => default_label.to_string(),
        };

        let inputs = match &self.inputs {
            Some(val) => val
                .iter()
                .map(|x| x.evaluate_tokens_to_string("input path", &vars))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        let outputs = match &self.outputs {
            Some(val) => val
                .iter()
                .map(|x| x.evaluate_tokens_to_string("output path", &vars))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };

        let output = PreparedTask {
            label: label,
            steps: self.steps.clone(),
            inputs: inputs,
            outputs: outputs,
            silent: self.silent,
            vars: vars,
            forcing: self.forcing.clone(),
        };
        Ok(output)
    }
}

#[derive(Debug)]
pub struct PreparedTask {
    pub label: String,
    pub steps: Vec<StepConfig>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub silent: bool,
    pub vars: VariableSet,
    pub forcing: ForcingBehaviour,
}

impl PreparedTask {
    #[async_recursion(?Send)]
    pub async fn evaluate(
        &mut self,
        config: &RequeueConfig,
        executor: &DigExecutor<'_>,
    ) -> Result<()> {
        // TODO: Evaluate Inputs
        // TODO: Evaluate Outputs
        // TODO: Evaluate forcing

        self.log("Begin");

        for (step_i, step) in self.steps.iter().enumerate() {
            let step_output = step.evaluate(step_i, &self.vars, &executor).await?;

            let subtasks = match step_output {
                StepEvaluationResult::SubmitTasks(submittable_tasks) => Some(submittable_tasks),
                StepEvaluationResult::SkippedDueToIfStatement(_) => None,
                StepEvaluationResult::CompletedWithOutput(step_output) => {
                    // Check for storage
                    match step.get_store() {
                        Some(key) => {
                            self.vars.insert(key.clone(), step_output);
                            None
                        }
                        None => None,
                    }
                }
            };

            // We drop the limiter at this point so as to not consume a Semaphore token while executing subtasks. Meanwhile,
            // the current task will just be waiting...
            match subtasks {
                None => (),
                Some(subtasks) => {
                    let mut subtask_futures = Vec::new();
                    for subtask in subtasks.iter() {
                        subtask_futures.push(self.make_subtask_future(config, subtask, executor));
                    }
                    let outcomes = join_all(subtask_futures.into_iter()).await;
                    for outcome in outcomes {
                        match outcome {
                            Ok(_) => (),
                            Err(error) => return Err(error),
                        }
                    }
                }
            }
        }

        self.log("Finished");

        Ok(())
    }

    async fn make_subtask_future(
        &self,
        config: &RequeueConfig,
        subtask: &PreparedTaskStep,
        executor: &DigExecutor<'_>,
    ) -> Result<()> {
        let subtask_config = config.get_task(&subtask.task)?;
        let mut subtask = subtask_config
            .prepare(&subtask.task, &self.vars, executor)
            .await?;
        subtask.evaluate(config, executor).await
    }

    fn log(&self, message: &str) {
        let message = format!("TASK:{} -- {}", self.label, message).blue();
        println!("{}", message)
    }
    #[allow(dead_code)]
    fn log_bad(&self, message: &str) {
        let message = format!("TASK:{} -- {}", self.label, message).red();
        println!("{}", message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task() -> Result<()> {
        todo!()
    }
}
