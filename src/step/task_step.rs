use crate::{
    token::TokenedJsonValue,
    vars::{
        no_overrides, RawVariableMap, RawVariableMapTrait, VariableMap, VariableMapStack,
        VariableMapStackTrait,
    },
};
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::HashMap, path::Path, process::Command};

use super::common::{StepEvaluationResult, StepMethods};

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

    fn process_variables(&self, var_stack: &VariableMapStack) -> Result<VariableMap> {
        let step_vars = match &self.vars {
            None => (*var_stack.last().expect(
                "A TaskStep should always have it's parent's vars as the last item in the var stack",
            )).clone(),
            Some(rawvars) => rawvars.evaluate(&var_stack, &no_overrides(), false)?,
        };

        Ok(step_vars)
    }

    fn test_if_statement(
        &self,
        var_stack: &VariableMapStack,
        env: Option<&HashMap<String, String>>,
        dir: Option<&String>,
    ) -> Result<Option<(usize, String)>> {
        // Test If statements
        match &self.r#if {
            None => Ok(None),
            Some(statements) => {
                let mut output = None;
                for (i, statement) in statements.iter().enumerate() {
                    let statement = statement.evaluate_tokens_to_string("if-test", var_stack)?;

                    let mut command = Command::new("bash");
                    command.arg("-c");
                    let _command = command.arg(format!("test {}", statement));

                    contextualize_command(_command, env, dir);

                    let command_output = command.output()?;
                    let result = command_output.status;
                    if !result.success() {
                        output = Some((i + 1, statement));
                        break;
                    }
                }
                Ok(output)
            }
        }
    }

    fn log(&self, step_i: usize, message: String) {
        println!("STEP:{} -- {}", step_i, message)
    }

    fn _prepare_subtask(
        &self,
        step_i: usize,
        step_vars: &VariableMap,
        var_stack: &VariableMapStack,
        env: Option<&HashMap<String, String>>,
        dir: Option<&String>,
        map_vars: Option<&Vec<(String, String)>>,
    ) -> Result<Vec<PreparedTaskStep>> {
        let output = match map_vars {
            None => {
                let task = PreparedTaskStep {
                    // Note we clone everything so that each task manages it's own data
                    task: self.task.clone(),
                    vars: step_vars.clone(),
                    env: env.cloned(),
                    dir: dir.cloned(),
                    over: self.over.clone(),
                };
                self.log(
                    step_i,
                    format!(
                        "Queueing Task {} - '{}'",
                        &task.task,
                        serde_json::to_string(&task.vars)?,
                    ),
                );
                vec![task]
            }

            Some(map_vars) => {
                let mut map_vars = map_vars.clone();
                match map_vars.pop() {
                    None => self._prepare_subtask(step_i, step_vars, var_stack, env, dir, None)?,
                    Some((target_key, source_key)) => {
                        let source_value_vec = match source_key.evaluate_tokens(var_stack)? {
                            serde_json::Value::Array(x) => x.clone(),
                            serde_json::Value::Object(_) => {
                                bail!("Unable to map over object variable '{}'", source_key)
                            }
                            other => vec![other.clone()],
                        };

                        let mut output = Vec::new();
                        for source_value in source_value_vec.into_iter() {
                            let mut new_step_vars = step_vars.clone();
                            new_step_vars.insert(target_key.clone(), source_value);

                            let new_tasks = self._prepare_subtask(
                                step_i,
                                &new_step_vars,
                                var_stack,
                                env,
                                dir,
                                Some(&map_vars),
                            )?;
                            output.extend(new_tasks);
                        }

                        output
                    }
                }
            }
        };
        Ok(output)
    }

    fn prepare_subtasks(
        &self,
        step_i: usize,
        step_vars: &VariableMap,
        var_stack: &VariableMapStack,
        env: Option<&HashMap<String, String>>,
        dir: Option<&String>,
    ) -> Result<StepEvaluationResult> {
        let output = match &self.over {
            None => {
                let tasks = self._prepare_subtask(step_i, step_vars, var_stack, env, dir, None)?;
                StepEvaluationResult::SubmitTasks(tasks)
            }
            Some(map_over) => {
                let map_vars = Vec::from_iter(map_over.clone().into_iter());
                let tasks =
                    self._prepare_subtask(step_i, step_vars, var_stack, env, dir, Some(&map_vars))?;
                StepEvaluationResult::SubmitTasks(tasks)
            }
        };

        Ok(output)
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

        let mut var_stack = var_stack.clone();
        let step_vars = self.process_variables(&var_stack)?;
        var_stack.push(&step_vars);

        let exit_on_if = self.test_if_statement(&var_stack, env.as_ref(), dir.as_ref())?;
        let output = match exit_on_if {
            Some((if_stmt_id, if_stmt_str)) => {
                self.log(
                    step_i,
                    format!(
                        "Skipped due to if statement #{}, '{}'",
                        if_stmt_id, if_stmt_str
                    ),
                );
                StepEvaluationResult::SkippedDueToIfStatement((if_stmt_id, if_stmt_str))
            }
            None => {
                self.prepare_subtasks(step_i, &step_vars, &var_stack, env.as_ref(), dir.as_ref())?
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
