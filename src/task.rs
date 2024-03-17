use anyhow::Result;
use serde::Deserialize;

use crate::{
    step::common::{StepConfig, StepEvaluationResult, StepMethods},
    token::TokenedJsonValue,
    vars::{RawVariableMap, RawVariableMapTrait, VariableMap, VariableMapStack}, // vars::{process_raw_vars, ProcessedVarValueMap, RawVarValueMap, VarPriority},
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
    pub fn build_vars(&self, var_stack: &VariableMapStack) -> Result<VariableMap> {
        let task_vars = match &self.vars {
            None => VariableMap::new(),
            Some(rawvars) => rawvars.evaluate(var_stack)?,
        };

        Ok(task_vars)
    }

    pub fn prepare<'a>(
        &'a self,
        default_label: &str,
        var_stack: &'a VariableMapStack,
    ) -> Result<PreparedTask> {
        let task_vars = self.build_vars(var_stack)?;
        let mut task_var_stack = var_stack.clone();
        task_var_stack.push(&task_vars);

        let label = match &self.label {
            Some(val) => val.evaluate_tokens_to_string("label", &task_var_stack)?,
            None => default_label.to_string(),
        };

        let inputs = match &self.inputs {
            Some(val) => val
                .iter()
                .map(|x| x.evaluate_tokens_to_string("input path", &task_var_stack))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        let outputs = match &self.outputs {
            Some(val) => val
                .iter()
                .map(|x| x.evaluate_tokens_to_string("output path", &task_var_stack))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };

        let output = PreparedTask {
            label: label,
            steps: self.steps.clone(),
            inputs: inputs,
            outputs: outputs,
            silent: self.silent,
            task_vars: task_vars,
            forcing: self.forcing.clone(),
        };
        Ok(output)
    }
}

#[derive(Debug)]
pub struct PreparedTask {
    pub label: String,
    pub steps: Vec<StepConfig>, // Vec<TaskStep>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub silent: bool,
    pub task_vars: VariableMap,
    pub forcing: ForcingBehaviour,
}

impl PreparedTask {
    pub fn evaluate(&mut self, var_stack: &VariableMapStack) -> Result<()> {
        // TODO: Evaluate Inputs
        // TODO: Evaluate Outputs
        // TODO: Evaluate forcing

        self.log("Begin");

        for (step_i, step) in self.steps.iter().enumerate() {
            let step_output = {
                let mut temp_var_stack = var_stack.clone();
                temp_var_stack.push(&self.task_vars);

                step.evaluate(step_i, &temp_var_stack)
            }?;

            match step_output {
                StepEvaluationResult::CompletedWithNoOutput => todo!(),
                StepEvaluationResult::QueuedOneLocalTask(_) => todo!(),
                StepEvaluationResult::QueuedManyLocalTasks(_) => todo!(),
                StepEvaluationResult::SkippedDueToIfStatement(_) => (),
                StepEvaluationResult::CompletedWithOutput(step_output) => {
                    // Check for storage
                    match step.get_store() {
                        Some(key) => {
                            self.task_vars.insert(key.clone(), step_output);
                        }
                        None => (),
                    }
                }
            }
        }
        self.log("Finished");

        Ok(())
    }

    fn log(&self, message: &str) {
        let message = format!("TASK:{} -- {}", self.label, message).blue();
        println!("{}", message)
    }
    fn log_bad(&self, message: &str) {
        let message = format!("TASK:{} -- {}", self.label, message).red();
        println!("{}", message)
    }
}
