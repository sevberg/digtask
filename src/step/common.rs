use crate::step::basic_step::BasicStep;
use crate::step::{bash_step::BashStep, python_step::PythonStep};
use crate::vars::VariableMapStack;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

#[derive(PartialEq, Debug)]
pub enum StepEvaluationResult {
    SkippedDueToIfStatement((usize, String)),
    CompletedWithNoOutput,
    CompletedWithOutput(JsonValue),
    QueuedOneLocalTask(String),
    QueuedManyLocalTasks(Vec<String>),
}

pub trait StepMethods {
    fn evaluate(&self, step_i: usize, var_stack: &VariableMapStack)
        -> Result<StepEvaluationResult>;
    fn get_store(&self) -> Option<&String>;
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum StepConfig {
    Simple(String),
    Config(CommandConfig),
}

impl StepMethods for StepConfig {
    fn get_store(&self) -> Option<&String> {
        match &self {
            StepConfig::Simple(x) => None,
            StepConfig::Config(x) => x.get_store(),
        }
    }
    fn evaluate(
        &self,
        step_i: usize,
        var_stack: &VariableMapStack,
    ) -> Result<StepEvaluationResult> {
        match &self {
            StepConfig::Simple(x) => BashStep::new(x).evaluate(step_i, var_stack),
            StepConfig::Config(x) => x.evaluate(step_i, var_stack),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
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

    fn evaluate(
        &self,
        step_i: usize,
        var_stack: &VariableMapStack,
    ) -> Result<StepEvaluationResult> {
        match &self {
            CommandConfig::Basic(x) => x.evaluate(step_i, var_stack),
            CommandConfig::Bash(x) => x.evaluate(step_i, var_stack),
            CommandConfig::Python(x) => x.evaluate(step_i, var_stack),
            // CommandConfig::Jq(x) => x.evaluate(var_stack),
        }
    }
}
