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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum StepConfig {
    Simple(String),
    Config(CommandConfig),
}

impl StepConfig {
    pub fn evaluate(&self, var_stack: &VariableMapStack) -> Result<StepEvaluationResult> {
        match &self {
            StepConfig::Simple(x) => BashStep::new(x).evaluate(var_stack),
            StepConfig::Config(x) => x.evaluate(var_stack),
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

impl CommandConfig {
    pub fn evaluate(&self, var_stack: &VariableMapStack) -> Result<StepEvaluationResult> {
        match &self {
            CommandConfig::Basic(x) => x.evaluate(var_stack),
            CommandConfig::Bash(x) => x.evaluate(var_stack),
            CommandConfig::Python(x) => x.evaluate(var_stack),
            // CommandConfig::Jq(x) => x.evaluate(var_stack),
        }
    }
}
