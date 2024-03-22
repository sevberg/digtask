use std::{fs, path::Path, time::SystemTime};

use anyhow::Result;
use async_recursion::async_recursion;
use futures::future::join_all;
use serde::Deserialize;
use serde_json::Value as JsonValue;

use crate::{
    config::DigConfig,
    executor::DigExecutor,
    step::{
        common::{StepConfig, StepEvaluationResult, StepMethods},
        task_step::PreparedTaskStep,
    },
    token::TokenedJsonValue,
    vars::{RawVariableMap, StackMode, VariableSet},
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
    EverythingForced,
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
    pub async fn prepare(
        &self,
        default_label: &str,
        vars: &VariableSet,
        stack_mode: StackMode,
        executor: &DigExecutor<'_>,
    ) -> Result<PreparedTask> {
        // dbg!(vars);
        let vars = match &self.vars {
            None => vars.stack(stack_mode),
            Some(raw_vars) => {
                vars.stack_raw_variables(raw_vars, stack_mode, executor)
                    .await?
            }
        };

        // dbg!(&vars);

        let label = match &self.label {
            Some(val) => val.evaluate_tokens_to_string("label", &vars)?,
            None => default_label.to_string(),
        };

        let inputs = match &self.inputs {
            Some(val) => val
                .iter()
                .map(|x| x.evaluate_tokens_to_string("input path", &vars))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };
        let outputs = match &self.outputs {
            Some(val) => val
                .iter()
                .map(|x| x.evaluate_tokens_to_string("output path", &vars))
                .collect::<Result<Vec<_>, _>>()?,
            None => Vec::new(),
        };

        let output = PreparedTask {
            label: label,
            steps: self.steps.clone(),
            inputs: inputs,
            outputs: outputs,
            silent: self.silent,
            vars: vars,
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
    pub vars: VariableSet,
    pub forcing: ForcingBehaviour,
}

impl PreparedTask {
    fn get_latest_input(&self) -> Result<SystemTime> {
        let mut last_modification = SystemTime::UNIX_EPOCH;
        for path in self.inputs.iter() {
            let file_modified = match fs::metadata(path) {
                Ok(meta) => meta.modified()?,
                Err(error) => {
                    self.log_bad(format!("Couldn't access input file '{}'", path).as_str());
                    return Err(error.into());
                }
            };
            last_modification = last_modification.max(file_modified);
        }

        Ok(last_modification)
    }

    fn get_earliest_output(&self) -> Result<SystemTime> {
        let mut first_modification = SystemTime::now();
        for path in self.outputs.iter() {
            if Path::new(path).exists() {
                let file_modified = fs::metadata(path)?.modified()?;
                first_modification = first_modification.min(file_modified);
            }
        }

        Ok(first_modification)
    }

    async fn do_evaluation(
        &mut self,
        config: &DigConfig,
        capture_output: bool,
        executor: &DigExecutor<'_>,
        is_forced: bool,
    ) -> Result<Option<Vec<String>>> {
        let mut outputs = Vec::new();

        for (step_i, step) in self.steps.iter().enumerate() {
            let step_output = step.evaluate(step_i, &self.vars, &executor).await?;

            let subtasks = match step_output {
                StepEvaluationResult::SubmitTasks(submittable_tasks) => Some(submittable_tasks),
                StepEvaluationResult::SkippedDueToIfStatement(_) => None,
                StepEvaluationResult::Completed(step_output) => {
                    if capture_output {
                        outputs.push(step_output.clone());
                    }

                    // Process Output
                    let step_output_value = match serde_json::from_str::<JsonValue>(&step_output) {
                        Ok(json_val) => json_val,
                        Err(_) => JsonValue::String(step_output),
                    };

                    // Check for storage
                    match step.get_store() {
                        Some(key) => {
                            self.vars.insert(key.clone(), step_output_value);
                            None
                        }
                        None => None,
                    }
                }
            };

            let all_subtask_outputs = match subtasks {
                None => None,
                Some(subtasks) => {
                    let mut subtask_futures = Vec::new();
                    for subtask in subtasks.iter() {
                        subtask_futures.push(self.make_subtask_future(
                            config,
                            subtask,
                            capture_output,
                            executor,
                            is_forced,
                        ));
                    }
                    let subtask_results = join_all(subtask_futures.into_iter()).await;

                    let mut output = Vec::new();
                    for outcome in subtask_results {
                        match outcome {
                            Ok(possible_subtask_output) => match possible_subtask_output {
                                Some(subtask_output) => output.extend(subtask_output),
                                None => (),
                            },
                            Err(error) => return Err(error),
                        }
                    }

                    Some(output)
                }
            };

            match all_subtask_outputs {
                Some(all_subtask_outputs) => outputs.extend(all_subtask_outputs),
                None => (),
            }
        }

        self.log("Finished");

        match capture_output {
            true => Ok(Some(outputs)),
            false => Ok(None),
        }
    }

    #[async_recursion(?Send)]
    pub async fn evaluate(
        &mut self,
        config: &DigConfig,
        capture_output: bool,
        executor: &DigExecutor<'_>,
        forcing: ForcingContext,
    ) -> Result<Option<Vec<String>>> {
        self.log("Begin");

        // Handle Inputs/Outputs
        let latest_input = self.get_latest_input()?;
        let earliest_output = self.get_earliest_output()?;

        // Handle Forcing
        let is_forced = match forcing {
            ForcingContext::EverythingForced => true,
            ForcingContext::NotForced => match self.forcing {
                ForcingBehaviour::Always => true,
                _ => false,
            },
            ForcingContext::ExplicitlyForced => true,
            ForcingContext::ParentIsForced => match self.forcing {
                ForcingBehaviour::Always => true,
                ForcingBehaviour::Inherit => true,
                ForcingBehaviour::Never => false,
            },
        };

        // Handle Skipping
        let skip_reason = match is_forced {
            true => None,
            false => match earliest_output < latest_input {
                true => Some("Outputs are up to date".to_string()),
                false => None,
            },
        };

        // Evaluate, if not skipped
        let output = match skip_reason {
            Some(reason) => {
                self.log(format!("Skipping because {}", reason).as_str());
                Ok(None)
            }
            None => {
                self.do_evaluation(config, capture_output, executor, is_forced)
                    .await
            }
        };

        output
    }

    async fn make_subtask_future(
        &self,
        config: &DigConfig,
        subtask: &PreparedTaskStep,
        capture_output: bool,
        executor: &DigExecutor<'_>,
        is_forced: bool,
    ) -> Result<Option<Vec<String>>> {
        let subtask_config = config.get_task(&subtask.task)?;
        let mut subtask = subtask_config
            .prepare(
                &subtask.task,
                &subtask.vars,
                StackMode::EmptyLocals,
                executor,
            )
            .await?;

        let forcing = match is_forced {
            true => ForcingContext::ParentIsForced,
            false => ForcingContext::NotForced,
        };
        subtask
            .evaluate(config, capture_output, executor, forcing)
            .await
    }

    fn log(&self, message: &str) {
        let message = format!("TASK:{} -- {}", self.label, message).blue();
        println!("{}", message)
    }
    fn log_bad(&self, message: &str) {
        let message = format!("TASK:{} -- {}", self.label, message).red();
        eprintln!("{}", message)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::bail;
    use serde_json::json;

    use crate::{
        step::{common::SingularStepConfig, task_step::TaskStepConfig},
        testing_block_on,
        vars::RawVariable,
    };

    use super::*;

    fn _make_vars() -> VariableSet {
        let mut output = VariableSet::new();
        output.insert("COUNTRIES".into(), json!(vec!["ITA", "USA", "TRY"]));
        output
    }

    fn _make_task_prepare_country() -> TaskConfig {
        TaskConfig {
            label: Some("prepare_country".into()),
            steps: vec![StepConfig::Single(SingularStepConfig::Simple(
                "echo PREPARING: {{iso3}}".into(),
            ))],
            inputs: None,
            outputs: None,
            silent: true,
            vars: Some(
                vec![("iso3".to_string(), RawVariable::Json("DEU".into()))]
                    .into_iter()
                    .collect(),
            ),
            forcing: ForcingBehaviour::Inherit,
        }
    }

    fn _make_task_analyze_country() -> TaskConfig {
        TaskConfig {
            label: Some("analyze_country".into()),
            steps: vec![
                StepConfig::Single(SingularStepConfig::Task(TaskStepConfig {
                    task: "prepare_country".into(),
                    vars: None,
                    env: None,
                    dir: None,
                    r#if: None,
                    over: None,
                })),
                StepConfig::Single(SingularStepConfig::Simple(
                    "echo ANALYZING: {{iso3}}".into(),
                )),
            ],
            inputs: None,
            outputs: None,
            silent: true,
            vars: Some(
                vec![("iso3".to_string(), RawVariable::Json("GBR".into()))]
                    .into_iter()
                    .collect(),
            ),
            forcing: ForcingBehaviour::Inherit,
        }
    }

    fn _make_task_analyze_all_countries() -> TaskConfig {
        TaskConfig {
            label: Some("analyze_all_countries".into()),
            steps: vec![StepConfig::Single(SingularStepConfig::Task(
                TaskStepConfig {
                    task: "analyze_country".into(),
                    vars: None,
                    env: None,
                    dir: None,
                    r#if: None,
                    over: Some(
                        vec![("iso3".to_string(), "{{COUNTRIES}}".to_string())]
                            .into_iter()
                            .collect(),
                    ),
                },
            ))],
            inputs: None,
            outputs: None,
            silent: true,
            vars: None,
            forcing: ForcingBehaviour::Inherit,
        }
    }

    #[test]
    fn test_task() -> Result<()> {
        let vars = _make_vars();
        let task = _make_task_prepare_country();
        let mut prepared_task =
            testing_block_on!(ex, task.prepare("test", &vars, StackMode::EmptyLocals, &ex))?;

        let config = DigConfig::new();
        let outputs = testing_block_on!(
            ex,
            prepared_task.evaluate(&config, true, &ex, ForcingContext::NotForced)
        )?;

        match outputs {
            None => bail!("Expected outputs not present"),
            Some(outputs) => assert_eq!(outputs, vec!["PREPARING: DEU"]),
        }

        Ok(())
    }

    #[test]
    fn test_overridden_task() -> Result<()> {
        let mut vars = _make_vars();
        vars.insert("iso3".into(), json!("MEX"));
        let task = _make_task_prepare_country();
        let mut prepared_task =
            testing_block_on!(ex, task.prepare("test", &vars, StackMode::EmptyLocals, &ex))?;

        let config = DigConfig::new();
        let outputs = testing_block_on!(
            ex,
            prepared_task.evaluate(&config, true, &ex, ForcingContext::NotForced)
        )?;

        match outputs {
            None => bail!("Expected outputs not present"),
            Some(outputs) => assert_eq!(outputs, vec!["PREPARING: MEX"]),
        }

        Ok(())
    }

    #[test]
    fn test_task_with_subtask() -> Result<()> {
        let vars = _make_vars();
        let mut config = DigConfig::new();
        config
            .tasks
            .insert("prepare_country".into(), _make_task_prepare_country());
        let main_task = _make_task_analyze_country();

        let mut prepared_task = testing_block_on!(
            ex,
            main_task.prepare("test", &vars, StackMode::EmptyLocals, &ex)
        )?;

        let outputs = testing_block_on!(
            ex,
            prepared_task.evaluate(&config, true, &ex, ForcingContext::NotForced)
        )?;

        match outputs {
            None => bail!("Expected outputs not present"),
            Some(outputs) => assert_eq!(outputs, vec!["PREPARING: GBR", "ANALYZING: GBR"]),
        }

        Ok(())
    }

    #[test]
    fn test_task_with_mapped_subtasks() -> Result<()> {
        let vars = _make_vars();
        let mut config = DigConfig::new();
        config
            .tasks
            .insert("prepare_country".into(), _make_task_prepare_country());
        config
            .tasks
            .insert("analyze_country".into(), _make_task_analyze_country());
        let main_task = _make_task_analyze_all_countries();

        let mut prepared_task = testing_block_on!(
            ex,
            main_task.prepare("test", &vars, StackMode::EmptyLocals, &ex)
        )?;

        let outputs = testing_block_on!(
            ex,
            prepared_task.evaluate(&config, true, &ex, ForcingContext::NotForced)
        )?;

        match outputs {
            None => bail!("Expected outputs not present"),
            Some(outputs) => assert_eq!(
                outputs,
                vec![
                    "PREPARING: ITA",
                    "ANALYZING: ITA",
                    "PREPARING: USA",
                    "ANALYZING: USA",
                    "PREPARING: TRY",
                    "ANALYZING: TRY"
                ]
            ),
        }

        Ok(())
    }
}
