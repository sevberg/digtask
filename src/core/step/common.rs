use crate::core::{
    executor::DigExecutor,
    run_context::RunContext,
    step::{
        bash_step::BashStep,
        basic_step::BasicStep,
        parallel_step::ParallelStepConfig,
        python_step::PythonStep,
        task_step::{PreparedTaskStep, TaskStepConfig},
    },
    vars::VariableSet,
};
use anyhow::Result;
use serde::{Deserialize, Serialize};

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

pub trait CommandConfigMethods {
    fn ensure_not_a_command(obj: &serde_json::Value) -> Result<()>;
}

impl CommandConfigMethods for CommandConfig {
    fn ensure_not_a_command(obj: &serde_json::Value) -> Result<()> {
        if let serde_json::Value::Object(_) = obj {
            BasicStep::ensure_not_a_command(obj)?;
            BashStep::ensure_not_a_command(obj)?;
            PythonStep::ensure_not_a_command(obj)?;
        }
        Ok(())
    }
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
