use crate::step::basic_step::BasicStep;
// use crate::step::{bash_step::BashStep, python_step::PythonStep};
use crate::token::TokenedJsonValue;
use crate::vars::{VariableMap, VariableMapStack};
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
    // SingleCommand(String),
    Basic(BasicStep),
    // Bash(BashStep),
    // Python(PythonStep),
    // Jq(JqStep),
}

impl StepConfig {
    pub fn evaluate(&self, var_stack: &VariableMapStack) -> Result<StepEvaluationResult> {
        match &self {
            // StepConfig::SingleCommand(x) => BashStep::new(x).evaluate(var_stack),
            StepConfig::Basic(x) => x.evaluate(var_stack),
            // StepConfig::Bash(x) => x.evaluate(var_stack),
            // StepConfig::Python(x) => x.evaluate(var_stack),
            // StepConfig::Jq(x) => x.evaluate(var_stack),
        }
    }
}
