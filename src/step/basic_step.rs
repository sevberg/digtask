use crate::{token::TokenedJsonValue, vars::VariableMapStack};
use anyhow::{anyhow, Result};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::{
    borrow::BorrowMut,
    collections::HashMap,
    path::Path,
    process::{Command, ExitStatus},
};

use super::common::StepEvaluationResult;

fn default_command_entry() -> String {
    "bash -c".into()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BasicStep {
    pub cmd: RawCommandEntry,
    #[serde(default = "default_command_entry")]
    pub entry: String,
    pub env: Option<HashMap<String, String>>,
    pub dir: Option<String>,
    pub r#if: Option<Vec<String>>,
}

impl BasicStep {
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
    fn build_command(&self, var_stack: &VariableMapStack) -> Result<(Command, String)> {
        // Parse command entry
        let mut string_rep: Vec<String> = Vec::new();
        let entry = self.entry.evaluate_tokens_to_string("command", var_stack)?;
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
                let user_command = t.evaluate_tokens_to_string("command", var_stack)?;
                command.arg(user_command.clone());
                string_rep.push(user_command);
            }
            RawCommandEntry::Many(tokens) => {
                let user_command_elements = tokens
                    .iter()
                    .map(|t| t.evaluate_tokens_to_string("command", var_stack))
                    .collect::<Result<Vec<_>, _>>()?;

                command.args(user_command_elements.clone());
                string_rep.extend(user_command_elements);
            }
        };

        // Return
        let string_rep = string_rep.join(" ");
        Ok((command, string_rep))
    }

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

    pub fn evaluate(&self, var_stack: &VariableMapStack) -> Result<StepEvaluationResult> {
        let env = self.build_envs(var_stack)?;
        let dir = self.build_dir(var_stack)?;

        // Test If statements
        match &self.r#if {
            None => (),
            Some(statements) => {
                for (i, statement) in statements.iter().enumerate() {
                    let statement = statement.evaluate_tokens_to_string("if-test", var_stack)?;
                    let result = self.test_if_statement(&statement, env.as_ref(), dir.as_ref())?;
                    if !result.success() {
                        return Ok(StepEvaluationResult::SkippedDueToIfStatement((
                            i + 1,
                            statement,
                        )));
                    }
                }
            }
        }

        // Execute Command
        let (mut command, string_rep) = self.build_command(var_stack)?;
        contextualize_command(command.borrow_mut(), env.as_ref(), dir.as_ref());
        println!("{}", string_rep);

        let output = command.output()?;

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
        let trimmed_data = stdout.trim();

        let output = match serde_json::from_str::<JsonValue>(trimmed_data) {
            Ok(val) => val,
            Err(_) => trimmed_data.into(),
            // Err(err)=>panic!("{}", err.to_string())
        };

        Ok(StepEvaluationResult::CompletedWithOutput(output))
    }
}

#[cfg(test)]
mod test {
    use anyhow::bail;
    use serde_json::json;

    // use crate::vars::{VariableMap, VariableMapLike, NO_VARS};

    use crate::vars::{no_vars, VariableMap};

    use super::*;

    #[test]
    fn test_whoami() -> Result<()> {
        let cmdconfig = BasicStep {
            cmd: RawCommandEntry::None,
            entry: "whoami".into(),
            env: None,
            dir: None,
            r#if: None,
        };

        let output = cmdconfig.evaluate(&no_vars())?;
        match output {
            StepEvaluationResult::CompletedWithOutput(output) => match output {
                JsonValue::String(_) => (), // All good!
                other => bail!("We expected a string, not '{}'", other),
            },
            _ => bail!("The step did not complete"),
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
        };

        let output_dir = cmdconfig.evaluate(&no_vars())?;
        assert_eq!(
            output_dir,
            StepEvaluationResult::CompletedWithOutput(json!["/"])
        );

        Ok(())
    }

    #[test]
    fn test_env_usage() -> Result<()> {
        let mut envmap: HashMap<String, String> = HashMap::new();
        envmap.insert("IM_AN_ENV".into(), "IM_A_VARIABLE".into());
        envmap.insert("IM_A_{{KEY_1}}".into(), "IM_A_{{KEY_2}}".into());

        let mut vars = VariableMap::new();
        vars.insert("KEY_1".into(), "cats".into());
        vars.insert("KEY_2".into(), "dogs".into());

        let cmdconfig = BasicStep {
            entry: "bash -c".into(),
            cmd: RawCommandEntry::Single("echo \"${IM_AN_ENV}, but ${IM_A_{{KEY_1}}}\"".into()),
            dir: None,
            env: Some(envmap),
            r#if: None,
        };

        let message = cmdconfig.evaluate(&vec![&vars])?;
        assert_eq!(
            message,
            StepEvaluationResult::CompletedWithOutput("IM_A_VARIABLE, but IM_A_dogs".into())
        );

        Ok(())
    }

    #[test]
    fn test_if_usage() -> Result<()> {
        let mut vars = VariableMap::new();
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
        };

        let outcome = cmdconfig.evaluate(&vec![&vars])?;
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
        };

        let output = cmdconfig.evaluate(&no_vars())?;

        match output {
            StepEvaluationResult::CompletedWithOutput(output) => match output {
                JsonValue::Number(_) => (), // We got an int, everything's fine
                other => bail!("Expected an integer, got '{}'", other),
            },
            _ => bail!("Did not get the correct result"),
        }

        Ok(())
    }

    #[test]
    fn token_vars() -> Result<()> {
        let mut varmap = VariableMap::new();
        varmap.insert("hats".into(), "date".into());
        varmap.insert("entry".into(), "bash".into());

        let cmdconfig = BasicStep {
            entry: "{{entry}}".into(),
            cmd: RawCommandEntry::Many(vec!["-c".into(), "{{hats}} +%s".into()]),
            env: None,
            dir: None,
            r#if: None,
        };

        let output = cmdconfig.evaluate(&vec![&varmap])?;

        match output {
            StepEvaluationResult::CompletedWithOutput(output) => match output {
                JsonValue::Number(_) => (), // We got an int, everything's fine
                other => bail!("Expected an integer, got '{}'", other),
            },
            _ => bail!("Did not get the correct result"),
        }

        Ok(())
    }
}
