use anyhow::Result;
use serde::Deserialize;

use crate::{
    config::RequeueConfig,
    step::{
        common::{StepConfig, StepEvaluationResult, StepMethods},
        task_step::PreparedTaskStep,
    },
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
    pub fn build_vars(
        &self,
        var_stack: &VariableMapStack,
        var_overrides: &VariableMap,
    ) -> Result<VariableMap> {
        let task_vars = match &self.vars {
            None => VariableMap::new(),
            Some(rawvars) => rawvars.evaluate(var_stack, var_overrides, true)?,
        };

        Ok(task_vars)
    }

    pub fn prepare<'a>(
        &'a self,
        default_label: &str,
        var_stack: &'a VariableMapStack,
        var_overrides: &VariableMap,
    ) -> Result<PreparedTask> {
        let task_vars = self.build_vars(var_stack, var_overrides)?;
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
    pub steps: Vec<StepConfig>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub silent: bool,
    pub task_vars: VariableMap,
    pub forcing: ForcingBehaviour,
}

impl PreparedTask {
    fn spawn(
        &self,
        task_def: PreparedTaskStep,
        var_stack: &VariableMapStack,
        config: &RequeueConfig,
    ) -> Result<PreparedTask> {
        let task = config.get_task(&task_def.task)?;
        let task = task.prepare(&task_def.task, var_stack, &task_def.vars)?;
        Ok(task)
    }

    pub fn evaluate(&mut self, var_stack: &VariableMapStack, config: &RequeueConfig) -> Result<()> {
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

            let mut sub_tasks = match step_output {
                StepEvaluationResult::CompletedWithNoOutput => None,
                StepEvaluationResult::Requeue(task_def) => {
                    Some(self.spawn(task_def, var_stack, config)?)
                }
                StepEvaluationResult::SkippedDueToIfStatement(_) => None,
                StepEvaluationResult::CompletedWithOutput(step_output) => {
                    // Check for storage
                    match step.get_store() {
                        Some(key) => {
                            self.task_vars.insert(key.clone(), step_output);
                            None
                        }
                        None => None,
                    }
                }
            };

            match &mut sub_tasks {
                Some(sub_task) => sub_task.evaluate(var_stack, config)?,
                None => (),
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
