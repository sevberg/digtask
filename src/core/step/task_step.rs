use crate::core::{
    common::default_false,
    config::{DirConfig, EnvConfig},
    executor::DigExecutor,
    gate::{test_run_gates, RunGates},
    run_context::RunContext,
    step::common::{StepEvaluationResult, StepMethods},
    token::TokenedJsonValue,
    vars::{RawVariableMap, StackMode, VariableSet},
};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TaskStepConfig {
    pub task: String,
    pub vars: Option<RawVariableMap>,
    pub env: EnvConfig,
    pub dir: DirConfig,
    pub r#if: Option<RunGates>,
    pub over: Option<HashMap<String, String>>,
    #[serde(default = "default_false")]
    pub silent: bool,
}

impl TaskStepConfig {
    // async fn test_if_statement(
    //     &self,
    //     vars: &VariableSet,
    //     context: &RunContext,
    //     executor: &DigExecutor<'_>,
    // ) -> Result<Option<(usize, String)>> {
    //     // Test If statements
    //     match &self.r#if {
    //         None => Ok(None),
    //         Some(statements) => {
    //             let mut output = None;
    //             for (i, statement) in statements.iter().enumerate() {
    //                 let statement = statement.evaluate_tokens_to_string("if-test", vars)?;

    //                 let mut command = Command::new("bash");
    //                 command.arg("-c");
    //                 let _command = command.arg(format!("test {}", statement));

    //                 contextualize_command(_command, context);

    //                 let lock = executor.limiter.acquire().await;
    //                 let command_output = command.output().await?;
    //                 drop(lock);

    //                 let result = command_output.status;
    //                 if !result.success() {
    //                     output = Some((i + 1, statement));
    //                     break;
    //                 }
    //             }
    //             Ok(output)
    //         }
    //     }
    // }

    fn log(&self, step_i: usize, message: String) {
        println!("STEP:{} -- {}", step_i, message)
    }

    fn _prepare_subtasks(
        &self,
        step_i: usize,
        vars: &VariableSet,
        context: RunContext,
        map_vars: Option<&Vec<(String, String)>>,
    ) -> Result<Vec<PreparedTaskStep>> {
        let output = match map_vars {
            None => {
                let task = PreparedTaskStep {
                    // Note we clone everything so that each task manages it's own data
                    task: self.task.clone(),
                    vars: vars.clone(),
                    context,
                    // over: self.over.clone(),
                };
                self.log(
                    step_i,
                    format!(
                        "Queueing Task {} - '{}'",
                        &task.task,
                        serde_json::to_string(&task.vars.local_vars)?,
                    ),
                );
                vec![task]
            }

            Some(map_vars) => {
                let mut map_vars = map_vars.clone();
                match map_vars.pop() {
                    None => self._prepare_subtasks(step_i, vars, context, None)?,
                    Some((target_key, source_key)) => {
                        let source_value_vec = match source_key.evaluate_tokens(vars)? {
                            serde_json::Value::Array(x) => x.clone(),
                            serde_json::Value::Object(_) => {
                                bail!("Unable to map over object variable '{}'", source_key)
                            }
                            other => vec![other.clone()],
                        };

                        let mut output = Vec::new();
                        for source_value in source_value_vec.into_iter() {
                            let mut new_step_vars = vars.clone();
                            new_step_vars.insert(target_key.clone(), source_value);

                            let new_tasks = self._prepare_subtasks(
                                step_i,
                                &new_step_vars,
                                context.clone(),
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
        vars: &VariableSet,
        context: RunContext,
    ) -> Result<StepEvaluationResult> {
        let output = match &self.over {
            None => {
                let tasks = self._prepare_subtasks(step_i, vars, context, None)?;
                StepEvaluationResult::SubmitTasks(tasks)
            }
            Some(map_over) => {
                #[allow(clippy::useless_conversion)]
                // Using 'into_iter' below is not useless, since we need a vector of Strings, not '&String's
                let map_vars = Vec::from_iter(map_over.clone().into_iter());
                let tasks = self._prepare_subtasks(step_i, vars, context, Some(&map_vars))?;
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
    async fn evaluate(
        &self,
        step_i: usize,
        vars: &VariableSet,
        context: &RunContext,
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        let mut context = context.clone();
        let vars = match &self.vars {
            None => vars.stack(StackMode::CopyLocals),
            Some(raw_vars) => {
                vars.stack_raw_variables(raw_vars, StackMode::EmptyLocals, &context, executor)
                    .await?
            }
        };
        context.update(self.env.as_ref(), self.dir.as_ref(), self.silent, &vars)?;

        let runif_result = test_run_gates(self.r#if.as_ref(), &vars, &context, executor).await?;
        let output = match runif_result {
            Some((id, exit)) => {
                self.log(
                    step_i,
                    format!("Skipped due to if statement #{}, '{}'", id, exit.statement),
                );
                StepEvaluationResult::SkippedDueToIfStatement((id, exit.statement))
            }
            None => self.prepare_subtasks(step_i, &vars, context)?,
        };

        Ok(output)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PreparedTaskStep {
    pub task: String,
    pub vars: VariableSet,
    pub context: RunContext,
}

#[cfg(test)]
mod tests {

    use serde_json::json;

    use crate::{core::vars::VariableMap, testing_block_on};

    use super::*;

    fn _make_vars() -> VariableSet {
        let mut output = VariableSet::new();
        output.insert("key1".into(), json!(vec!["hats", "bats", "rats"]));
        output.insert("key2".into(), json!(17));
        output
    }

    fn _make_raw_vars() -> RawVariableMap {
        let mut output = RawVariableMap::new();
        output.insert("key2".into(), json!("{{key2}}").into());
        output.insert("key3".into(), json!("cats").into());
        output.insert("key4".into(), json!(22).into());
        output
    }

    #[test]
    fn test_only_name() -> Result<()> {
        let task_config = TaskStepConfig {
            task: "test_task".to_string(),
            vars: None,
            env: None,
            dir: None,
            r#if: None,
            over: None,
            silent: false,
        };

        let vars = _make_vars();
        let context = RunContext::default();

        let output = testing_block_on!(ex, task_config.evaluate(0, &vars, &context, &ex))?;

        match output {
            StepEvaluationResult::SubmitTasks(mut tasks) => {
                assert_eq!(tasks.len(), 1);
                let task_def = tasks.pop().unwrap();
                assert_eq!(task_def.task, "test_task");
                assert_eq!(task_def.vars.parent().unwrap(), &vars.local_vars);
                assert_eq!(task_def.vars.local_vars, vars.local_vars);
                assert!(task_def.context.env.is_none());
                assert!(task_def.context.dir.is_none());
                Ok(())
            }
            other => bail!("Expected to 'SubmitTasks', got '{:?}'", other),
        }
    }

    #[test]
    fn test_env_dir() -> Result<()> {
        let env: HashMap<String, String> =
            vec![("WHO_YOU_GUNNA_CALL".to_string(), "mom?".to_string())]
                .into_iter()
                .collect();
        let dir = "/".to_string();

        let task_config = TaskStepConfig {
            task: "test_task".to_string(),
            vars: None,
            env: Some(env.clone()),
            dir: Some(dir.clone()),
            r#if: None,
            over: None,
            silent: false,
        };

        let vars = _make_vars();
        let context = RunContext::default();
        let output = testing_block_on!(ex, task_config.evaluate(0, &vars, &context, &ex))?;

        match output {
            StepEvaluationResult::SubmitTasks(mut tasks) => {
                assert_eq!(tasks.len(), 1);
                let task_def = tasks.pop().unwrap();
                assert_eq!(task_def.task, "test_task");
                assert_eq!(task_def.vars.parent().unwrap(), &vars.local_vars);
                assert_eq!(task_def.vars.local_vars, vars.local_vars);
                assert_eq!(task_def.context.env, Some(env));
                assert_eq!(task_def.context.dir, Some(dir));
                Ok(())
            }
            other => bail!("Expected to 'SubmitTasks', got '{:?}'", other),
        }
    }

    #[test]
    fn test_skippable() -> Result<()> {
        let task_config = TaskStepConfig {
            task: "test_task".to_string(),
            vars: None,
            env: None,
            dir: None,
            r#if: Some(vec!["\"cats\" = \"dogs\"".into()]),
            over: None,
            silent: false,
        };

        let vars = _make_vars();
        let context = RunContext::default();
        let output = testing_block_on!(ex, task_config.evaluate(0, &vars, &context, &ex))?;

        match output {
            StepEvaluationResult::SkippedDueToIfStatement((statement_num, reason)) => {
                assert_eq!(statement_num, 0);
                assert_eq!(reason, "\"cats\" = \"dogs\"");
                Ok(())
            }
            other => bail!("Expected a 'SkippedDueToIfStatement', got '{:?}'", other),
        }
    }

    #[test]
    fn test_empty_vars() -> Result<()> {
        let task_config = TaskStepConfig {
            task: "test_task".to_string(),
            vars: Some(RawVariableMap::new()),
            env: None,
            dir: None,
            r#if: None,
            over: None,
            silent: false,
        };

        let vars = _make_vars();
        let context = RunContext::default();
        let output = testing_block_on!(ex, task_config.evaluate(0, &vars, &context, &ex))?;

        match output {
            StepEvaluationResult::SubmitTasks(mut tasks) => {
                assert_eq!(tasks.len(), 1);
                let task_def = tasks.pop().unwrap();
                assert_eq!(task_def.task, "test_task");
                assert_eq!(task_def.vars.parent().unwrap(), &vars.local_vars);
                assert!(task_def.vars.local_vars.is_empty());
                assert!(task_def.context.env.is_none());
                assert!(task_def.context.dir.is_none());
                Ok(())
            }
            other => bail!("Expected to 'SubmitTasks', got '{:?}'", other),
        }
    }

    #[test]
    fn test_loop_over() -> Result<()> {
        let task_config = TaskStepConfig {
            task: "test_task".to_string(),
            vars: Some(_make_raw_vars()),
            env: None,
            dir: None,
            r#if: None,
            over: Some(
                vec![("key3".to_string(), "{{key1}}".to_string())]
                    .into_iter()
                    .collect(),
            ),
            silent: false,
        };

        let vars = _make_vars();
        let context = RunContext::default();
        let output = testing_block_on!(ex, task_config.evaluate(0, &vars, &context, &ex))?;

        match output {
            StepEvaluationResult::SubmitTasks(mut tasks) => {
                assert_eq!(tasks.len(), 3);

                // Queued Task 1
                let task_def = tasks.pop().unwrap();
                assert_eq!(task_def.task, "test_task");
                assert!(task_def.context.env.is_none());
                assert!(task_def.context.dir.is_none());
                assert_eq!(task_def.vars.parent().unwrap(), &vars.local_vars);

                let expected_vars: VariableMap = vec![
                    // ("key1".to_string(), json!("")),
                    ("key2".to_string(), json!(17)),
                    ("key3".to_string(), json!("rats")),
                    ("key4".to_string(), json!(22)),
                ]
                .into_iter()
                .collect();

                assert_eq!(task_def.vars.local_vars, expected_vars);

                // Queued Task 2
                let task_def = tasks.pop().unwrap();
                let expected_vars: VariableMap = vec![
                    // ("key1".to_string(), json!("")),
                    ("key2".to_string(), json!(17)),
                    ("key3".to_string(), json!("bats")),
                    ("key4".to_string(), json!(22)),
                ]
                .into_iter()
                .collect();

                assert_eq!(task_def.vars.local_vars, expected_vars);

                // Queued Task 3
                let task_def = tasks.pop().unwrap();
                let expected_vars: VariableMap = vec![
                    // ("key1".to_string(), json!("")),
                    ("key2".to_string(), json!(17)),
                    ("key3".to_string(), json!("hats")),
                    ("key4".to_string(), json!(22)),
                ]
                .into_iter()
                .collect();

                assert_eq!(task_def.vars.local_vars, expected_vars);

                Ok(())
            }
            other => bail!("Expected to 'SubmitTasks', got '{:?}'", other),
        }
    }
}
