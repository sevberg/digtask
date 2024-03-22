use crate::executor::DigExecutor;
use crate::step::basic_step::BasicStep;
use crate::step::{bash_step::BashStep, python_step::PythonStep};
use crate::vars::VariableSet;
use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::parallel_step::ParallelStepConfig;
use super::task_step::{PreparedTaskStep, TaskStepConfig};

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
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        match &self {
            SingularStepConfig::Simple(x) => {
                BashStep::new(x).evaluate(step_i, vars, executor).await
            }
            SingularStepConfig::Config(x) => x.evaluate(step_i, vars, executor).await,
            SingularStepConfig::Task(x) => x.evaluate(step_i, vars, executor).await,
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
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        match &self {
            StepConfig::Single(x) => x.evaluate(step_i, vars, executor).await,
            StepConfig::Parallel(x) => x.evaluate(step_i, vars, executor).await,
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
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        match &self {
            CommandConfig::Basic(x) => x.evaluate(step_i, vars, executor).await,
            CommandConfig::Bash(x) => x.evaluate(step_i, vars, executor).await,
            CommandConfig::Python(x) => x.evaluate(step_i, vars, executor).await, // CommandConfig::Jq(x) => x.evaluate(var_stack, executor),
        }
    }
}
