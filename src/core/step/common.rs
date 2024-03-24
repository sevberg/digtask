use crate::executor::DigExecutor;
use crate::run_context::RunContext;
use crate::step::basic_step::BasicStep;
use crate::step::{bash_step::BashStep, python_step::PythonStep};
use crate::vars::VariableSet;
use anyhow::Result;
use async_process::Command;
use serde::{Deserialize, Serialize};

use super::parallel_step::ParallelStepConfig;
use super::task_step::{PreparedTaskStep, TaskStepConfig};

pub fn contextualize_command(command: &mut Command, context: &RunContext) {
    match &context.env {
        None => (),
        Some(envmap) => {
            command.envs(envmap);
        }
    }
    match &context.dir {
        None => (),
        Some(dir) => {
            command.current_dir(dir);
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum StepEvaluationResult {
    SkippedDueToIfStatement((usize, String)),
    Completed(String),
    SubmitTasks(Vec<PreparedTaskStep>),
}

pub trait StepMethods {
    async fn evaluate(
        &self,
        step_i: usize,
        vars: &VariableSet,
        context: &RunContext,
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult>;
    fn get_store(&self) -> Option<&String> {
        None
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum SingularStepConfig {
    Simple(String),
    Config(CommandConfig),
    Task(TaskStepConfig),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum StepConfig {
    Single(SingularStepConfig),
    Parallel(ParallelStepConfig),
}

impl From<&str> for StepConfig {
    fn from(value: &str) -> Self {
        StepConfig::Single(SingularStepConfig::Simple(value.to_string()))
    }
}

impl StepMethods for SingularStepConfig {
    fn get_store(&self) -> Option<&String> {
        match &self {
            SingularStepConfig::Simple(_) => None,
            SingularStepConfig::Config(x) => x.get_store(),
            SingularStepConfig::Task(x) => x.get_store(),
        }
    }
    async fn evaluate(
        &self,
        step_i: usize,
        vars: &VariableSet,
        context: &RunContext,
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        match &self {
            SingularStepConfig::Simple(x) => {
                BashStep::new(x)
                    .evaluate(step_i, vars, context, executor)
                    .await
            }
            SingularStepConfig::Config(x) => x.evaluate(step_i, vars, context, executor).await,
            SingularStepConfig::Task(x) => x.evaluate(step_i, vars, context, executor).await,
        }
    }
}

impl StepMethods for StepConfig {
    fn get_store(&self) -> Option<&String> {
        match &self {
            StepConfig::Single(x) => x.get_store(),
            StepConfig::Parallel(x) => x.get_store(),
        }
    }
    async fn evaluate(
        &self,
        step_i: usize,
        vars: &VariableSet,
        context: &RunContext,
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        match &self {
            StepConfig::Single(x) => x.evaluate(step_i, vars, context, executor).await,
            StepConfig::Parallel(x) => x.evaluate(step_i, vars, context, executor).await,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
pub enum CommandConfig {
    Basic(BasicStep),
    Bash(BashStep),
    Python(PythonStep),
}

impl StepMethods for CommandConfig {
    fn get_store(&self) -> Option<&String> {
        match &self {
            CommandConfig::Basic(x) => x.get_store(),
            CommandConfig::Bash(x) => x.get_store(),
            CommandConfig::Python(x) => x.get_store(),
            // CommandConfig::Jq(x) => x.get_store(),
        }
    }

    async fn evaluate(
        &self,
        step_i: usize,
        vars: &VariableSet,
        context: &RunContext,
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        match &self {
            CommandConfig::Basic(x) => x.evaluate(step_i, vars, context, executor).await,
            CommandConfig::Bash(x) => x.evaluate(step_i, vars, context, executor).await,
            CommandConfig::Python(x) => x.evaluate(step_i, vars, context, executor).await, // CommandConfig::Jq(x) => x.evaluate(var_stack, executor),
        }
    }
}
