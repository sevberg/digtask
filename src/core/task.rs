use std::{fs, path::Path, time::SystemTime};

use anyhow::{anyhow, Result};
use async_recursion::async_recursion;
use futures::future::join_all;
use serde::Deserialize;
use serde_json::{json, Value as JsonValue};

use crate::core::{
    common::default_false,
    config::{DigConfig, DirConfig, EnvConfig},
    executor::DigExecutor,
    gate::RunGates,
    run_context::{ForcingBehaviour, RunContext},
    step::{
        common::{StepConfig, StepEvaluationResult, StepMethods},
        task_step::PreparedTaskStep,
    },
    token::TokenedJsonValue,
    vars::{RawVariableMap, StackMode, VariableSet},
};

use colored::Colorize;

use super::gate::test_run_gates;

fn default_forcing() -> ForcingBehaviour {
    ForcingBehaviour::Inherit
}

fn task_log(label: &str, message: &str) {
    let message = format!("TASK:{} -- {}", label, message).blue();
    println!("{}", message)
}

fn task_log_bad(label: &str, message: &str) {
    let message = format!("TASK:{} -- {}", label, message).red();
    eprintln!("{}", message)
}

#[derive(Deserialize, Debug)]
pub struct TaskConfig {
    pub label: Option<String>,
    pub presteps: Option<Vec<StepConfig>>,
    pub steps: Vec<StepConfig>,
    pub poststeps: Option<Vec<StepConfig>>,
    pub inputs: Option<Vec<String>>,
    pub outputs: Option<Vec<String>>,
    pub r#if: Option<RunGates>,
    pub unless: Option<RunGates>,
    #[serde(default = "default_false")]
    pub silent: bool,
    pub vars: Option<RawVariableMap>,
    #[serde(default = "default_forcing")]
    pub forcing: ForcingBehaviour,
    pub env: EnvConfig,
    pub dir: DirConfig,
}

impl TaskConfig {
    pub async fn prepare(
        &self,
        default_label: &str,
        vars: &VariableSet,
        stack_mode: StackMode,
        parent_context: &RunContext,
        executor: &DigExecutor<'_>,
    ) -> Result<TaskEvaluationData> {
        let mut context = parent_context.child_context(self.forcing);
        let vars = match &self.vars {
            None => vars.stack(stack_mode),
            Some(raw_vars) => {
                vars.stack_raw_variables(raw_vars, stack_mode, &context, executor)
                    .await?
            }
        };
        context.update(self.env.as_ref(), self.dir.as_ref(), self.silent, &vars)?;

        let label = match &self.label {
            Some(val) => val.evaluate_tokens_to_string("label", &vars)?,
            None => default_label.to_string(),
        };

        Ok(TaskEvaluationData {
            label,
            vars,
            context,
        })
    }

    async fn test_cancel(
        &self,
        data: &TaskEvaluationData,
        executor: &DigExecutor<'_>,
    ) -> Result<Option<CanceledTask>> {
        // Handle cancling
        if self.unless.is_none() {
            return Ok(None);
        }

        let run_gate_outcome =
            test_run_gates(self.unless.as_ref(), &data.vars, &data.context, executor).await?;
        match run_gate_outcome {
            // Some((id, r#if_exit)) => Ok(Some(CanceledTask {
            //     label: data.label.clone(),
            //     reason: format!(
            //         "cancel-if statement {} returned false: '{}'",
            //         id, r#if_exit.statement
            //     ),
            // })),
            // None => Ok(None),
            Some(_) => Ok(None),
            None => Ok(Some(CanceledTask {
                label: data.label.clone(),
                reason: "all unless-statements returned true".to_string(),
            })),
        }
    }

    async fn test_skip(
        &self,
        data: &TaskEvaluationData,
        executor: &DigExecutor<'_>,
    ) -> Result<Option<SkippedTask>> {
        // Handle skipping
        let skip_reason = self
            .check_skip_state(&data.vars, &data.context, executor)
            .await?;
        match skip_reason {
            None => (),
            Some(reason) => match data.context.is_forced() {
                false => {
                    return Ok(Some(SkippedTask {
                        label: data.label.clone(),
                        reason,
                    }))
                }
                true => (),
            },
        }

        // Done
        Ok(None)
    }

    async fn check_skip_state(
        &self,
        vars: &VariableSet,
        context: &RunContext,
        executor: &DigExecutor<'_>,
    ) -> Result<Option<String>> {
        // Test Run-If statemennts
        let run_gate_outcome = test_run_gates(self.r#if.as_ref(), vars, context, executor).await?;
        if run_gate_outcome.is_some() {
            let (id, r#if_exit) = run_gate_outcome.unwrap();
            return Ok(Some(format!(
                "run-if statement {} returned false: '{}'",
                id, r#if_exit.statement
            )));
        }

        // Test inputs/outputs
        if self.inputs.is_some() {
            let latest_input = self.get_latest_input(vars)?;
            let earliest_output = self.get_earliest_output(vars)?;
            if earliest_output > latest_input {
                return Ok(Some("all outputs are up to date'".to_string()));
            }
        }

        // done
        Ok(None)
    }

    fn get_latest_input(&self, vars: &VariableSet) -> Result<SystemTime> {
        let mut last_modification = SystemTime::UNIX_EPOCH;
        match &self.inputs {
            None => (),
            Some(inputs) => {
                for raw_path in inputs.iter() {
                    let path = raw_path.evaluate_tokens_to_string("input path", vars)?;
                    let file_modified = match fs::metadata(&path) {
                        Ok(meta) => meta.modified()?,
                        Err(error) => {
                            // self.log_bad(format!("Couldn't access input file '{}'", path).as_str());
                            return Err(error.into());
                        }
                    };
                    last_modification = last_modification.max(file_modified);
                }
            }
        }

        Ok(last_modification)
    }

    fn get_earliest_output(&self, vars: &VariableSet) -> Result<SystemTime> {
        let mut first_modification = SystemTime::now();
        match &self.outputs {
            None => (),
            Some(outputs) => {
                for raw_path in outputs.iter() {
                    let path = raw_path.evaluate_tokens_to_string("output path", vars)?;
                    if Path::new(&path).exists() {
                        let file_modified = fs::metadata(&path)?.modified()?;
                        first_modification = first_modification.min(file_modified);
                    }
                }
            }
        }

        Ok(first_modification)
    }

    #[async_recursion(?Send)]
    pub async fn evaluate(
        &self,
        mut data: TaskEvaluationData,
        config: &DigConfig,
        capture_output: bool,
        executor: &DigExecutor<'_>,
    ) -> Result<Option<Vec<String>>> {
        // Check for Canceling
        if let Some(t) = self.test_cancel(&data, executor).await? {
            task_log(
                &data.label,
                format!("Canceled because {}", t.reason).as_ref(),
            );
            return Err(anyhow!("Task {} canceled", data.label));
        }

        // Evaluate Dependencies
        let pretest_outputs = match &self.presteps {
            Some(presteps) => {
                task_log(&data.label, "Evaluating Dependencies");

                self.evaluate_steps(presteps, &mut data, config, capture_output, executor)
                    .await?
            }
            None => Vec::new(),
        };

        // Check for Skipping
        if let Some(t) = self.test_skip(&data, executor).await? {
            match &data.context.is_forced() {
                true => task_log(&data.label, "Forced"),
                false => {
                    task_log(
                        &data.label,
                        format!("Skipped because {}", t.reason).as_ref(),
                    );
                    return Ok(None);
                }
            }
        }

        // Do evaluation
        task_log(&data.label, "Begin");
        let step_outputs = self
            .evaluate_steps(&self.steps, &mut data, config, capture_output, executor)
            .await;

        match step_outputs {
            Ok(_) => data.vars.insert("SUCCESS".to_string(), json!(true)),
            Err(_) => data.vars.insert("SUCCESS".to_string(), json!(false)),
        }

        // Evaluate post-steps
        let poststep_outputs = match &self.poststeps {
            Some(poststeps) => {
                task_log(&data.label, "Evaluating post-steps");

                self.evaluate_steps(poststeps, &mut data, config, capture_output, executor)
                    .await
            }
            None => Ok(Vec::new()),
        };

        // Handle errors
        let (step_outputs, poststep_outputs) = match step_outputs {
            Ok(step_outputs) => match poststep_outputs {
                Ok(poststep_outputs) => (step_outputs, poststep_outputs),
                Err(poststep_error) => {
                    task_log_bad(&data.label, "Task succeeded, but post-steps failed");
                    return Err(poststep_error);
                }
            },
            Err(step_error) => match poststep_outputs {
                Ok(_) => {
                    task_log_bad(&data.label, "Task failed");
                    return Err(step_error);
                }
                Err(poststep_error) => {
                    task_log_bad(
                        &data.label,
                        format!(
                            "Task failed:\n{}\n\nAnd then post-steps failed as well",
                            step_error
                        )
                        .as_str(),
                    );
                    return Err(poststep_error);
                }
            },
        };

        task_log(&data.label, "Finished");

        // Finalize
        match capture_output {
            true => {
                let outputs = [pretest_outputs, step_outputs, poststep_outputs].concat();
                Ok(Some(outputs))
            }
            false => Ok(None),
        }
    }

    async fn evaluate_steps(
        &self,
        steps: &[StepConfig],
        data: &mut TaskEvaluationData,
        config: &DigConfig,
        capture_output: bool,
        executor: &DigExecutor<'_>,
    ) -> Result<Vec<String>> {
        let mut outputs = Vec::new();

        for (step_i, step) in steps.iter().enumerate() {
            let step_output = step
                .evaluate(step_i, &data.vars, &data.context, executor)
                .await?;

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
                            data.vars.insert(key.clone(), step_output_value);
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
                        subtask_futures.push(self.evaluate_subtask(
                            data,
                            config,
                            subtask,
                            capture_output,
                            executor,
                        ));
                    }
                    let subtask_results = join_all(subtask_futures.into_iter()).await;

                    let mut output = Vec::new();
                    for outcome in subtask_results {
                        match outcome {
                            Ok(possible_subtask_output) => {
                                if let Some(subtask_output) = possible_subtask_output {
                                    output.extend(subtask_output)
                                }
                            }
                            Err(error) => return Err(error),
                        }
                    }

                    Some(output)
                }
            };

            if let Some(all_subtask_outputs) = all_subtask_outputs {
                outputs.extend(all_subtask_outputs)
            }
        }

        Ok(outputs)
    }

    async fn evaluate_subtask(
        &self,
        data: &TaskEvaluationData,
        config: &DigConfig,
        subtask: &PreparedTaskStep,
        capture_output: bool,
        executor: &DigExecutor<'_>,
    ) -> Result<Option<Vec<String>>> {
        let subtask_config = config.get_task(&subtask.task)?;
        // let subtask_context = self.context.child_context(subtask_config.forcing);
        let subtask_data = subtask_config
            .prepare(
                &subtask.task,
                &subtask.vars,
                StackMode::EmptyLocals,
                &data.context,
                executor,
            )
            .await?;

        subtask_config
            .evaluate(subtask_data, config, capture_output, executor)
            .await
    }
}

#[derive(Debug)]
pub struct SkippedTask {
    pub label: String,
    pub reason: String,
}

#[derive(Debug)]
pub struct CanceledTask {
    pub label: String,
    pub reason: String,
}

#[derive(Debug)]
pub struct TaskEvaluationData {
    pub label: String,
    pub vars: VariableSet,
    pub context: RunContext,
}

#[cfg(test)]
mod tests {
    use anyhow::bail;
    use serde_json::json;

    use crate::core::{
        step::{common::SingularStepConfig, task_step::TaskStepConfig},
        vars::RawVariable,
    };
    use crate::testing_block_on;

    use super::*;

    fn _make_vars() -> VariableSet {
        let mut output = VariableSet::new();
        output.insert("COUNTRIES".into(), json!(vec!["ITA", "USA", "TRY"]));
        output.insert("NAME".into(), json!("batman"));
        output
    }

    fn _make_task_prepare_country() -> TaskConfig {
        TaskConfig {
            label: Some("prepare_country".into()),
            presteps: None,
            steps: vec!["echo PREPARING: {{iso3}}".into()],
            poststeps: None,
            inputs: None,
            outputs: None,
            r#if: None,
            unless: None,
            silent: false,
            vars: Some(
                vec![("iso3".to_string(), RawVariable::Json("DEU".into()))]
                    .into_iter()
                    .collect(),
            ),
            forcing: ForcingBehaviour::Inherit,
            env: None,
            dir: None,
        }
    }

    fn _make_task_analyze_country() -> TaskConfig {
        TaskConfig {
            label: Some("analyze_country".into()),
            presteps: None,
            steps: vec![
                StepConfig::Single(SingularStepConfig::Task(TaskStepConfig {
                    task: "prepare_country".into(),
                    vars: None,
                    env: None,
                    dir: None,
                    r#if: None,
                    over: None,
                    silent: false,
                })),
                StepConfig::Single(SingularStepConfig::Simple(
                    "echo ANALYZING: {{iso3}}".into(),
                )),
            ],
            poststeps: None,
            inputs: None,
            outputs: None,
            r#if: None,
            unless: None,
            silent: true,
            vars: Some(
                vec![("iso3".to_string(), RawVariable::Json("GBR".into()))]
                    .into_iter()
                    .collect(),
            ),
            forcing: ForcingBehaviour::Inherit,
            env: None,
            dir: None,
        }
    }

    fn _make_task_analyze_all_countries() -> TaskConfig {
        TaskConfig {
            label: Some("analyze_all_countries".into()),
            presteps: None,
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
                    silent: false,
                },
            ))],
            poststeps: None,
            inputs: None,
            outputs: None,
            r#if: None,
            unless: None,
            silent: true,
            vars: None,
            forcing: ForcingBehaviour::Inherit,
            env: None,
            dir: None,
        }
    }

    #[test]
    fn test_task() -> Result<()> {
        let vars = _make_vars();
        let task = _make_task_prepare_country();
        let context = RunContext::default();
        let task_data = testing_block_on!(
            ex,
            task.prepare("test", &vars, StackMode::EmptyLocals, &context, &ex)
        )?;

        let config = DigConfig::new();
        let outputs = testing_block_on!(ex, task.evaluate(task_data, &config, true, &ex))?;

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
        let context = RunContext::default();
        let task_data = testing_block_on!(
            ex,
            task.prepare("test", &vars, StackMode::EmptyLocals, &context, &ex)
        )?;

        let config = DigConfig::new();
        let outputs = testing_block_on!(ex, task.evaluate(task_data, &config, true, &ex))?;

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
        let task = _make_task_analyze_country();

        let context = RunContext::default();
        let task_data = testing_block_on!(
            ex,
            task.prepare("test", &vars, StackMode::EmptyLocals, &context, &ex)
        )?;

        let outputs = testing_block_on!(ex, task.evaluate(task_data, &config, true, &ex))?;

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
        let task = _make_task_analyze_all_countries();

        let context = RunContext::default();
        let task_data = testing_block_on!(
            ex,
            task.prepare("test", &vars, StackMode::EmptyLocals, &context, &ex)
        )?;

        let outputs = testing_block_on!(ex, task.evaluate(task_data, &config, true, &ex))?;

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

    #[test]
    fn test_task_with_dir_env() -> Result<()> {
        let vars = _make_vars();

        let task = TaskConfig {
            label: Some("dir_env".into()),
            presteps: None,
            steps: vec!["echo \"I am the ${SOME_ENV}\"".into(), "pwd".into()],
            poststeps: None,
            inputs: None,
            outputs: None,
            r#if: None,
            unless: None,
            silent: true,
            vars: Some(
                vec![("iso3".to_string(), RawVariable::Json("DEU".into()))]
                    .into_iter()
                    .collect(),
            ),
            forcing: ForcingBehaviour::Inherit,
            env: Some(
                vec![("SOME_ENV".to_string(), "{{NAME}}".into())]
                    .into_iter()
                    .collect(),
            ),
            dir: Some("/".into()),
        };

        let context = RunContext::default();
        let task_data = testing_block_on!(
            ex,
            task.prepare("test", &vars, StackMode::EmptyLocals, &context, &ex)
        )?;

        let config = DigConfig::new();
        let outputs = testing_block_on!(ex, task.evaluate(task_data, &config, true, &ex))?;

        match outputs {
            None => bail!("Expected outputs not present"),
            Some(outputs) => assert_eq!(outputs, vec!["I am the batman", "/"]),
        }

        Ok(())
    }
}
