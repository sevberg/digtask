use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::vars::VariableMapStack;

use super::{
    basic_step::{BasicStep, RawCommandEntry},
    common::StepEvaluationResult,
};

fn default_executable() -> String {
    "/bin/bash".into()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BashStep {
    #[serde(default = "default_executable")]
    pub executable: String,
    pub bash: String,
    pub env: Option<HashMap<String, String>>,
    pub dir: Option<String>,
    pub r#if: Option<Vec<String>>,
}

impl BashStep {
    pub fn new(command: &String) -> Self {
        BashStep {
            executable: default_executable(),
            bash: command.clone(),
            env: None,
            dir: None,
            r#if: None,
        }
    }
    pub fn evaluate(&self, var_stack: &VariableMapStack) -> Result<StepEvaluationResult> {
        // let executable = self.executable.evaluate(vars)?;
        BasicStep {
            entry: format!("{} -c", self.executable).into(),
            cmd: RawCommandEntry::Single(self.bash.clone()),
            env: self.env.clone(),
            dir: self.dir.clone(),
            r#if: self.r#if.clone(),
        }
        .evaluate(var_stack)
    }
}

#[cfg(test)]
mod test {
    use anyhow::bail;
    use serde_json::Value as JsonValue;

    use crate::vars::no_vars;

    use super::*;

    #[test]
    fn test_usage() -> Result<()> {
        let bash_command_config = BashStep {
            executable: "/bin/bash".into(),
            bash: "whoami".into(),
            env: None,
            dir: None,
            r#if: None,
        };

        let output = bash_command_config.evaluate(&no_vars())?;
        match output {
            StepEvaluationResult::CompletedWithOutput(output) => match output {
                JsonValue::String(_) => (), // All good!
                other => bail!("We expected a string. Got '{}'", other),
            },
            _ => bail!("Expected an completion with output"),
        };

        Ok(())
    }
}
