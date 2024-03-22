use crate::{executor::DigExecutor, token::TokenedJsonValue, vars::VariableSet};
use anyhow::{anyhow, Result};
use async_process::Command;
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{borrow::BorrowMut, collections::HashMap, path::Path, process::ExitStatus};

use super::common::{StepEvaluationResult, StepMethods};

fn default_command_entry() -> String {
    "bash -c".into()
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum RawCommandEntry {
    None,
    Single(String),
    Many(Vec<String>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum CommandEntry {
    None,
    Single(String),
    Many(Vec<String>),
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BasicStep {
    pub cmd: RawCommandEntry,
    #[serde(default = "default_command_entry")]
    pub entry: String,
    pub env: Option<HashMap<String, String>>,
    pub dir: Option<String>,
    pub r#if: Option<Vec<String>>,
    pub store: Option<String>,
}

impl BasicStep {
    fn build_envs(&self, vars: &VariableSet) -> Result<Option<HashMap<String, String>>> {
        let output = match &self.env {
            None => None,
            Some(envmap) => {
                let mut output_envmap: HashMap<String, String> = HashMap::new();
                envmap
                    .iter()
                    .map(|(key, val)| {
                        let key = key.evaluate_tokens_to_string("env-key", vars)?;
                        let val = val.evaluate_tokens_to_string("env-value", vars)?;
                        output_envmap.insert(key, val);
                        Ok(())
                    })
                    .collect::<Result<Vec<()>>>()?;

                Some(output_envmap)
            }
        };

        Ok(output)
    }

    fn build_dir(&self, vars: &VariableSet) -> Result<Option<String>> {
        let output = match &self.dir {
            None => None,
            Some(specified_dir) => {
                let specified_dir = specified_dir.evaluate_tokens_to_string("dir", vars)?;
                let path = Path::new(specified_dir.as_str());

                if !path.is_dir() {
                    return Err(anyhow!("Invalid directory '{}'", specified_dir));
                }

                Some(specified_dir)
            }
        };

        Ok(output)
    }
    fn build_command(&self, vars: &VariableSet) -> Result<(Command, String)> {
        // Parse command entry
        let mut string_rep: Vec<String> = Vec::new();
        let entry = self.entry.evaluate_tokens_to_string("command", vars)?;
        let entry_split = entry.split(" ").collect::<Vec<_>>();
        let (true_entry, initial_cmd) = entry_split
            .split_first()
            .expect("Entrypoint should be splittable");

        let mut command = Command::new(true_entry);
        string_rep.push(true_entry.trim().to_string());

        for cmd in initial_cmd.iter() {
            command.arg(cmd);
            string_rep.push(cmd.trim().to_string());
        }

        // Handle user command elements
        match &self.cmd {
            RawCommandEntry::None => (),
            RawCommandEntry::Single(t) => {
                let user_command = t.evaluate_tokens_to_string("command", vars)?;
                command.arg(user_command.clone());
                string_rep.push(user_command);
            }
            RawCommandEntry::Many(tokens) => {
                let user_command_elements = tokens
                    .iter()
                    .map(|t| t.evaluate_tokens_to_string("command", vars))
                    .collect::<Result<Vec<_>, _>>()?;

                command.args(user_command_elements.clone());
                string_rep.extend(user_command_elements);
            }
        };

        // Return
        let string_rep = string_rep.join(" ");
        Ok((command, string_rep))
    }

    async fn test_if_statement(
        &self,
        statement: &String,
        env: Option<&HashMap<String, String>>,
        dir: Option<&String>,
        executor: &DigExecutor<'_>,
    ) -> Result<ExitStatus> {
        let mut command = Command::new("bash");
        command.arg("-c");
        let _command = command.arg(format!("test {}", statement));
        contextualize_command(_command, env, dir);

        // println!("LOCKING - {:?}", executor.limiter);
        let lock = executor.limiter.acquire().await;
        let output = command.output().await?;
        drop(lock);
        // println!("UNLOCKING");

        Ok(output.status)
    }
}

impl StepMethods for BasicStep {
    fn get_store(&self) -> Option<&String> {
        self.store.as_ref()
    }

    async fn evaluate(
        &self,
        step_i: usize,
        vars: &VariableSet,
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        let env = self.build_envs(vars)?;
        let dir = self.build_dir(vars)?;

        // Test If statements
        let exit_on_if = match &self.r#if {
            None => None,
            Some(statements) => {
                let mut output = None;
                for (i, statement) in statements.iter().enumerate() {
                    let statement = statement.evaluate_tokens_to_string("if-test", vars)?;
                    let result = self
                        .test_if_statement(&statement, env.as_ref(), dir.as_ref(), executor)
                        .await?;
                    if !result.success() {
                        output = Some((i + 1, statement));
                        break;
                    }
                }
                output
            }
        };
        if exit_on_if.is_some() {
            let (if_stmt_id, if_stmt_str) = exit_on_if.unwrap();
            println!(
                "STEP:{} -- Skipped due to if statement #{}, '{}'",
                step_i, if_stmt_id, if_stmt_str
            );
            return Ok(StepEvaluationResult::SkippedDueToIfStatement((
                if_stmt_id,
                if_stmt_str,
            )));
        }

        // Execute Command
        let (mut command, string_rep) = self.build_command(vars)?;
        contextualize_command(command.borrow_mut(), env.as_ref(), dir.as_ref());
        println!("STEP:{} -- {}", step_i, string_rep);

        // println!("LOCKING - {:?}", executor.limiter);
        let lock = executor.limiter.acquire().await;
        let output = command.output().await?;
        drop(lock);
        // println!("UNLOCKING");

        let stdout = std::str::from_utf8(output.stdout.as_ref())
            .expect("Could not convert stdout to a UTF-8 string")
            .trim()
            .to_string();

        if !stdout.is_empty() {
            println!("{}", stdout.truecolor(100, 100, 100));
        }

        let stderr = std::str::from_utf8(output.stderr.as_ref())
            .expect("Could not convert stderr to a UTF-8 string")
            .trim()
            .to_string();

        if !stderr.is_empty() {
            println!("{}", stderr.red());
        }

        // Parse output and return
        match output.status.success() {
            true => {
                let trimmed_data = stdout.trim();
                Ok(StepEvaluationResult::Completed(trimmed_data.to_string()))
            }
            false => Err(anyhow!("{}", stderr)),
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::bail;

    use super::*;
    use crate::test_utils::*;

    #[test]
    fn test_whoami() -> Result<()> {
        let cmdconfig = BasicStep {
            cmd: RawCommandEntry::None,
            entry: "whoami".into(),
            env: None,
            dir: None,
            r#if: None,
            store: None,
        };
        let vars = VariableSet::new();
        let output = testing_block_on!(ex, cmdconfig.evaluate(0, &vars, &ex))?;
        match output {
            StepEvaluationResult::Completed(_) => (), // All good!
            _ => bail!("The step did not complete"),
        };

        Ok(())
    }

    #[test]
    fn test_sadpath() -> Result<()> {
        let cmdconfig = BasicStep {
            cmd: RawCommandEntry::None,
            entry: "whoamiwhoamiwhoami".into(),
            env: None,
            dir: None,
            r#if: None,
            store: None,
        };

        let vars = VariableSet::new();
        let output = testing_block_on!(ex, cmdconfig.evaluate(0, &vars, &ex));
        match output {
            Ok(val) => bail!("We expected a failure, but got '{:?}'", val),
            Err(error) => assert_eq!(error.to_string(), "No such file or directory (os error 2)"),
        };

        Ok(())
    }

    #[test]
    fn test_dir_usage() -> Result<()> {
        let cmdconfig = BasicStep {
            entry: "bash -c".into(),
            cmd: RawCommandEntry::Single("pwd".into()),
            dir: Some("/".into()),
            env: None,
            r#if: None,
            store: None,
        };

        let vars = VariableSet::new();
        let output_dir = testing_block_on!(ex, cmdconfig.evaluate(0, &vars, &ex))?;
        assert_eq!(output_dir, StepEvaluationResult::Completed("/".to_string()));

        Ok(())
    }

    #[test]
    fn test_env_usage() -> Result<()> {
        let mut envmap: HashMap<String, String> = HashMap::new();
        envmap.insert("IM_AN_ENV".into(), "IM_A_VARIABLE".into());
        envmap.insert("IM_A_{{KEY_1}}".into(), "IM_A_{{KEY_2}}".into());

        let mut vars = VariableSet::new();
        vars.insert("KEY_1".into(), "cats".into());
        vars.insert("KEY_2".into(), "dogs".into());

        let cmdconfig = BasicStep {
            entry: "bash -c".into(),
            cmd: RawCommandEntry::Single("echo \"${IM_AN_ENV}, but ${IM_A_{{KEY_1}}}\"".into()),
            dir: None,
            env: Some(envmap),
            r#if: None,
            store: None,
        };

        let message = testing_block_on!(ex, cmdconfig.evaluate(0, &vars, &ex))?;
        assert_eq!(
            message,
            StepEvaluationResult::Completed("IM_A_VARIABLE, but IM_A_dogs".into())
        );

        Ok(())
    }

    #[test]
    fn test_if_usage() -> Result<()> {
        let mut vars = VariableSet::new();
        vars.insert("KEY_1".into(), "cats".into());
        vars.insert("KEY_2".into(), "dogs".into());

        let mut if_statements: Vec<String> = Vec::new();
        if_statements.push("{{KEY_1}} = cats".into());
        if_statements.push("{{KEY_2}} = monkeys".into());

        let cmdconfig = BasicStep {
            entry: "bash -c".into(),
            cmd: RawCommandEntry::Single("badcommand".into()),
            dir: None,
            env: None,
            r#if: Some(if_statements),
            store: None,
        };

        let outcome = testing_block_on!(ex, cmdconfig.evaluate(0, &vars, &ex))?;
        match outcome {
            StepEvaluationResult::SkippedDueToIfStatement((i, statement)) => {
                assert_eq!(i, 2);
                assert_eq!(statement, "dogs = monkeys".to_string());
            }
            _ => bail!("Did not skip as expected"),
        }

        Ok(())
    }

    #[test]
    fn inline_many() -> Result<()> {
        let cmdconfig = BasicStep {
            entry: "bash".into(),
            cmd: RawCommandEntry::Many(vec!["-c".into(), "date +%s".into()]),
            env: None,
            dir: None,
            r#if: None,
            store: None,
        };

        let vars = VariableSet::new();
        let output = testing_block_on!(ex, cmdconfig.evaluate(0, &vars, &ex))?;

        match output {
            StepEvaluationResult::Completed(output) => {
                match serde_json::from_str::<JsonValue>(&output) {
                    Ok(val) => match val {
                        JsonValue::Number(_) => (), // All good, we got a number
                        other => bail!("We expected a number, but got '{:?}'", other),
                    },
                    Err(_) => bail!("Could not convert output to json"),
                }
            }
            _ => bail!("Did not get the correct result"),
        }

        Ok(())
    }

    #[test]
    fn token_vars() -> Result<()> {
        let mut vars = VariableSet::new();
        vars.insert("hats".into(), "date".into());
        vars.insert("entry".into(), "bash".into());

        let cmdconfig = BasicStep {
            entry: "{{entry}}".into(),
            cmd: RawCommandEntry::Many(vec!["-c".into(), "{{hats}} +%s".into()]),
            env: None,
            dir: None,
            r#if: None,
            store: None,
        };

        let output = testing_block_on!(ex, cmdconfig.evaluate(0, &vars, &ex))?;

        match output {
            StepEvaluationResult::Completed(output) => {
                match serde_json::from_str::<JsonValue>(&output) {
                    Ok(val) => match val {
                        JsonValue::Number(_) => (), // All good, we got a number
                        other => bail!("We expected a number, but got '{:?}'", other),
                    },
                    Err(_) => bail!("Could not convert output to json"),
                }
            }
            _ => bail!("Did not get the correct result"),
        }

        Ok(())
    }
}
