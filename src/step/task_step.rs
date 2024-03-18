use crate::{
    task::PreparedTask,
    token::TokenedJsonValue,
    vars::{no_overrides, RawVariableMap, RawVariableMapTrait, VariableMap, VariableMapStack},
};
use anyhow::{anyhow, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{
    borrow::BorrowMut,
    collections::HashMap,
    ops::Deref,
    path::Path,
    process::{Command, ExitStatus},
};

use super::common::{StepEvaluationResult, StepMethods};

fn default_inherit_parent_vars() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TaskStepConfig {
    pub task: String,
    pub vars: Option<RawVariableMap>,
    pub env: Option<HashMap<String, String>>,
    pub dir: Option<String>,
    pub r#if: Option<Vec<String>>,
    // pub store: Option<String>,
    pub over: Option<HashMap<String, String>>,
    // #[serde(default = "default_inherit_parent_vars")]
    // pub inherit_parent_vars: bool,
}

fn contextualize_command(
    command: &mut Command,
    env: Option<&HashMap<String, String>>,
    dir: Option<&String>,
) {
    match env {
        None => (),
        Some(envmap) => {
            command.envs(envmap);
        }
    }
    match dir {
        None => (),
        Some(dir) => {
            command.current_dir(dir);
        }
    }
}

impl TaskStepConfig {
    fn build_envs(&self, var_stack: &VariableMapStack) -> Result<Option<HashMap<String, String>>> {
        let output = match &self.env {
            None => None,
            Some(envmap) => {
                let mut output_envmap: HashMap<String, String> = HashMap::new();
                envmap
                    .iter()
                    .map(|(key, val)| {
                        let key = key.evaluate_tokens_to_string("env-key", var_stack)?;
                        let val = val.evaluate_tokens_to_string("env-value", var_stack)?;
                        output_envmap.insert(key, val);
                        Ok(())
                    })
                    .collect::<Result<Vec<()>>>()?;

                Some(output_envmap)
            }
        };

        Ok(output)
    }

    fn build_dir(&self, var_stack: &VariableMapStack) -> Result<Option<String>> {
        let output = match &self.dir {
            None => None,
            Some(specified_dir) => {
                let specified_dir = specified_dir.evaluate_tokens_to_string("dir", var_stack)?;
                let path = Path::new(specified_dir.as_str());

                if !path.is_dir() {
                    return Err(anyhow!("Invalid directory '{}'", specified_dir));
                }

                Some(specified_dir)
            }
        };

        Ok(output)
    }

    // pub fn build_vars(
    //     &self,
    //     var_stack: &VariableMapStack,
    //     var_overrides: &VariableMap,
    // ) -> Result<VariableMap> {
    //     let task_vars = match &self.vars {
    //         None => VariableMap::new(),
    //         Some(rawvars) => rawvars.evaluate(var_stack, var_overrides)?,
    //     };

    //     Ok(task_vars)
    // }

    fn test_if_statement(
        &self,
        statement: &String,
        env: Option<&HashMap<String, String>>,
        dir: Option<&String>,
    ) -> Result<ExitStatus> {
        let mut command = Command::new("bash");
        command.arg("-c");
        let _command = command.arg(format!("test {}", statement));
        contextualize_command(_command, env, dir);

        let output = command.output()?;
        Ok(output.status)
    }
}

impl StepMethods for TaskStepConfig {
    fn get_store(&self) -> Option<&String> {
        None
    }
    fn evaluate(
        &self,
        step_i: usize,
        var_stack: &VariableMapStack,
    ) -> Result<StepEvaluationResult> {
        let env = self.build_envs(var_stack)?;
        let dir = self.build_dir(var_stack)?;

        // Process the var stack
        let mut var_stack = var_stack.clone();

        let step_vars = match &self.vars {
            None => (*var_stack.last().expect(
                "A TaskStep should always have it's parent's vars as the last item in the var stack",
            )).clone(),
            Some(rawvars) => rawvars.evaluate(&var_stack, &no_overrides(), false)?,
        };
        var_stack.push(&step_vars);

        // Test If statements
        let exit_on_if = match &self.r#if {
            None => None,
            Some(statements) => {
                let mut output = None;
                for (i, statement) in statements.iter().enumerate() {
                    let statement = statement.evaluate_tokens_to_string("if-test", &var_stack)?;
                    let result = self.test_if_statement(&statement, env.as_ref(), dir.as_ref())?;
                    if !result.success() {
                        output = Some((i + 1, statement));
                        break;
                    }
                }
                output
            }
        };

        let output = match exit_on_if {
            Some((if_stmt_id, if_stmt_str)) => {
                println!(
                    "STEP:{} -- Skipped due to if statement #{}, '{}'",
                    step_i, if_stmt_id, if_stmt_str
                );
                StepEvaluationResult::SkippedDueToIfStatement((if_stmt_id, if_stmt_str))
            }
            None => {
                let task = PreparedTaskStep {
                    task: self.task.clone(),
                    vars: step_vars,
                    env: env,
                    dir: dir,
                    over: self.over.clone(),
                };
                println!(
                    "STEP:{} -- Queueing Task {} - '{}'",
                    step_i,
                    &task.task,
                    serde_json::to_string(&task.vars)?
                );
                StepEvaluationResult::Requeue(task)
            }
        };

        Ok(output)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PreparedTaskStep {
    pub task: String,
    pub vars: VariableMap,
    pub env: Option<HashMap<String, String>>,
    pub dir: Option<String>,
    // pub r#if: Option<Vec<String>>,
    // pub store: Option<String>,
    pub over: Option<HashMap<String, String>>,
}
