use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::{
    common::default_false, executor::DigExecutor, run_context::RunContext, vars::VariableSet,
};

use super::{
    basic_step::{BasicStep, RawCommandEntry},
    common::{StepEvaluationResult, StepMethods},
};

fn default_executable() -> String {
    "/bin/bash".into()
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BashStep {
    #[serde(default = "default_executable")]
    pub executable: String,
    pub bash: String,
    pub env: Option<HashMap<String, String>>,
    pub dir: Option<String>,
    pub r#if: Option<Vec<String>>,
    pub store: Option<String>,
    #[serde(default = "default_false")]
    pub silent: bool,
}

impl BashStep {
    pub fn new(command: &str) -> Self {
        BashStep {
            executable: default_executable(),
            bash: command.to_string(),
            env: None,
            dir: None,
            r#if: None,
            store: None,
            silent: false,
        }
    }
}

impl StepMethods for BashStep {
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
        // let executable = self.executable.evaluate(vars)?;
        BasicStep {
            entry: format!("{} -c", self.executable),
            cmd: RawCommandEntry::Single(self.bash.clone()),
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

    use crate::testing_block_on;

    use super::*;

    #[test]
    fn test_usage() -> Result<()> {
        let bash_command_config = BashStep {
            executable: "/bin/bash".into(),
            bash: "whoami".into(),
            env: None,
            dir: None,
            r#if: None,
            store: None,
            silent: false,
        };

        let vars = VariableSet::new();
        let context = RunContext::default();
        let output = testing_block_on!(ex, bash_command_config.evaluate(0, &vars, &context, &ex))?;
        match output {
            StepEvaluationResult::Completed(_) => (), // All good!
            _ => bail!("Expected an completion with output"),
        };

        Ok(())
    }
}
