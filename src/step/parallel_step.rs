use crate::vars::VariableMapStack;
use anyhow::{bail, Result};
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
        for step in self.parallel.iter() {
            match step {
                StepConfig::Parallel(_) => {
                    bail!("A parallel step cannot have nested parallel steps")
                }
                other => match other.evaluate(step_i, var_stack)? {
                    StepEvaluationResult::SubmitTasks(tasks) => output.extend(tasks.into_iter()),
                    _ => (),
                },
            }
        }

        match output.is_empty() {
            true => Ok(StepEvaluationResult::CompletedWithOutput(JsonValue::Null)),
            false => Ok(StepEvaluationResult::SubmitTasks(output)),
        }
    }
}
