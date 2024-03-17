use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::vars::VariableMapStack;

use super::{
    basic_step::{BasicStep, RawCommandEntry},
    common::StepEvaluationResult,
};

fn default_executable() -> String {
    "python3".into()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PythonStep {
    #[serde(default = "default_executable")]
    pub executable: String,
    pub py: String,
    pub env: Option<HashMap<String, String>>,
    pub dir: Option<String>,
    pub r#if: Option<Vec<String>>,
}

impl PythonStep {
    pub fn evaluate(&self, var_stack: &VariableMapStack) -> Result<StepEvaluationResult> {
        // let executable = self.executable.evaluate(vars)?;
        BasicStep {
            entry: format!("{} -c", self.executable).into(),
            cmd: RawCommandEntry::Single(self.py.clone()),
            env: self.env.clone(),
            dir: self.dir.clone(),
            r#if: self.r#if.clone(),
        }
        .evaluate(var_stack)
    }

    pub fn new(command: &str) -> Self {
        PythonStep {
            executable: default_executable(),
            py: command.into(),
            env: None,
            dir: None,
            r#if: None,
        }
    }
}

#[cfg(test)]
mod test {
    use anyhow::bail;
    use serde_json::Value as JsonValue;

    use crate::vars::VariableMap;

    use super::*;

    #[test]
    fn test_usage() -> Result<()> {
        let mut vars = VariableMap::new();
        vars.insert("SOME_NUM".into(), 17.into());

        let command_config = PythonStep {
            executable: "python3".into(),
            py: "\nimport math\nprint(math.sqrt( {{SOME_NUM}} ))".into(),
            env: None,
            dir: None,
            r#if: None,
        };

        let output = command_config.evaluate(&vec![&vars])?;
        match output {
            StepEvaluationResult::CompletedWithOutput(output) => match output {
                JsonValue::Number(val) => {
                    assert!((val.as_f64().unwrap() - 4.123105625617661).abs() < 1e-6)
                }
                other => bail!("We expected a float. Got '{}'", other),
            },
            _ => bail!("Expected an completion with output"),
        };

        Ok(())
    }
}