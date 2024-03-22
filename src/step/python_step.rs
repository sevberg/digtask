use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{executor::DigExecutor, vars::VariableSet};

use super::{
    basic_step::{BasicStep, RawCommandEntry},
    common::{StepEvaluationResult, StepMethods},
};

fn default_executable() -> String {
    "python3".into()
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PythonStep {
    #[serde(default = "default_executable")]
    pub executable: String,
    pub py: String,
    pub env: Option<HashMap<String, String>>,
    pub dir: Option<String>,
    pub r#if: Option<Vec<String>>,
    pub store: Option<String>,
}

impl PythonStep {
    #[allow(dead_code)]
    pub fn new(command: &str) -> Self {
        PythonStep {
            executable: default_executable(),
            py: command.into(),
            env: None,
            dir: None,
            r#if: None,
            store: None,
        }
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
        executor: &DigExecutor<'_>,
    ) -> Result<StepEvaluationResult> {
        // let executable = self.executable.evaluate(vars)?;
        BasicStep {
            entry: format!("{} -c", self.executable).into(),
            cmd: RawCommandEntry::Single(self.py.clone()),
            env: self.env.clone(),
            dir: self.dir.clone(),
            r#if: self.r#if.clone(),
            store: self.store.clone(),
        }
        .evaluate(step_i, vars, executor)
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
            env: None,
            dir: None,
            r#if: None,
            store: None,
        };

        let output = testing_block_on!(ex, command_config.evaluate(0, &vars, &ex))?;
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
