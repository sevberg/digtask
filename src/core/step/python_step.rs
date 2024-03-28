use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::{
    common::default_false,
    executor::DigExecutor,
    run_context::RunContext,
    step::{
        basic_step::{BasicStep, RawCommandEntry},
        common::{StepEvaluationResult, StepMethods},
    },
    vars::VariableSet,
};

use super::common::CommandConfigMethods;

fn default_executable() -> String {
    "python3".into()
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PythonStepType {
    Inline,
    Script,
}

impl PythonStepType {
    fn default() -> Self {
        PythonStepType::Script
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PythonStepTypeCondaConfig {
    conda: String,
    #[serde(default = "PythonStepType::default")]
    pub r#type: PythonStepType,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PythonStepTypeVenvConfig {
    venv: String,
    #[serde(default = "PythonStepType::default")]
    pub r#type: PythonStepType,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum PythonStepTypeConfig {
    Native(PythonStepType),
    Conda(PythonStepTypeCondaConfig),
    Venv(PythonStepTypeVenvConfig),
}

impl PythonStepTypeConfig {
    fn default() -> Self {
        PythonStepTypeConfig::Native(PythonStepType::default())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct PythonStep {
    #[serde(default = "default_executable")]
    pub executable: String,
    pub py: String,
    pub env: Option<HashMap<String, String>>,
    pub dir: Option<String>,
    pub r#if: Option<Vec<String>>,
    pub store: Option<String>,
    #[serde(default = "PythonStepTypeConfig::default")]
    pub r#type: PythonStepTypeConfig,
    #[serde(default = "default_false")]
    pub silent: bool,
}

impl PythonStep {
    pub fn new(command: &str) -> Self {
        PythonStep {
            executable: default_executable(),
            py: command.into(),
            r#type: PythonStepTypeConfig::Native(PythonStepType::Inline),
            env: None,
            dir: None,
            r#if: None,
            store: None,
            silent: false,
        }
    }

    #[allow(dead_code)]
    pub fn default() -> Self {
        PythonStep::new("print(\"Hello World\")")
    }
}

impl CommandConfigMethods for PythonStep {
    fn ensure_not_a_command(obj: &serde_json::Value) -> Result<()> {
        if let serde_json::Value::Object(data) = &obj {
            if data.contains_key("py") {
                let error = match serde_json::from_str::<PythonStep>(
                    serde_json::to_string(obj)?.as_ref(),
                ) {
                    Ok(_) => panic!("We expected the object to fail casting as a PythonStepConfig. Why did it succeed??"),
                    Err(error) => Err(anyhow!(
                        "Expected '{}' to be a PythonStepConfig, but encountered the error '{}'",
                        obj.to_string(),
                        error.to_string()
                    ))
                };

                return error;
            }
        }
        Ok(())
    }
}

impl StepMethods for PythonStep {
    fn get_store(&self) -> Option<&String> {
        self.store.as_ref()
    }
    async fn evaluate(
        &self,
        step_i: usize,
        vars: &VariableSet,
        context: &RunContext,
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        // println!("{}", format!("PY TYPE: {:?}", &self.r#type).red());

        let (executable, cmd) = match &self.r#type {
            PythonStepTypeConfig::Native(type_config) => {
                let executable = match type_config {
                    PythonStepType::Inline => format!("{} -c", self.executable),
                    PythonStepType::Script => self.executable.clone(),
                };
                let cmd = self.py.clone();
                (executable, RawCommandEntry::Single(cmd))
            }
            PythonStepTypeConfig::Conda(type_config) => {
                let executable = "conda".to_string();
                let mut cmd = vec![
                    "run".to_string(),
                    "-n".to_string(),
                    type_config.conda.clone(),
                    self.executable.clone(),
                ];

                match type_config.r#type {
                    PythonStepType::Inline => {
                        cmd.push("-c".to_string());
                        cmd.push(self.py.clone());
                    }
                    PythonStepType::Script => cmd.push(self.py.clone()),
                };
                (executable, RawCommandEntry::Many(cmd))
            }
            PythonStepTypeConfig::Venv(type_config) => {
                let executable = "bash -c".to_string();
                let cmd_head = format!(
                    "source {}/bin/activate && {}",
                    type_config.venv, self.executable
                );
                let cmd = match type_config.r#type {
                    PythonStepType::Inline => format!("{} -c {}", cmd_head, self.py),
                    PythonStepType::Script => format!("{} {}", cmd_head, self.py),
                };
                (executable, RawCommandEntry::Single(cmd))
            }
        };

        BasicStep {
            entry: executable,
            cmd,
            env: self.env.clone(),
            dir: self.dir.clone(),
            r#if: self.r#if.clone(),
            store: self.store.clone(),
            silent: self.silent,
        }
        .evaluate(step_i, vars, context, executor)
        .await
    }
}

#[cfg(test)]
mod test {
    use anyhow::bail;
    use serde_json::Value as JsonValue;

    use crate::testing_block_on;

    use super::*;

    #[test]
    fn test_usage() -> Result<()> {
        let mut vars = VariableSet::new();
        vars.insert("SOME_NUM".into(), 17.into());

        let command_config = PythonStep {
            executable: "python3".into(),
            py: "\nimport math\nprint(math.sqrt( {{SOME_NUM}} ))".into(),
            r#type: PythonStepTypeConfig::Native(PythonStepType::Inline),
            ..PythonStep::default()
        };
        let context = RunContext::default();

        let output = testing_block_on!(ex, command_config.evaluate(0, &vars, &context, &ex))?;
        match output {
            StepEvaluationResult::Completed(output) => {
                match serde_json::from_str::<JsonValue>(&output) {
                    Ok(val) => match val {
                        JsonValue::Number(val) => {
                            assert!((val.as_f64().unwrap() - 4.123105625617661).abs() < 1e-6)
                        }
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
