use std::collections::BTreeMap;

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_yaml;

use crate::{
    // task::{ForcingContext, Task, TaskEvaluation},
    task::TaskConfig,
    vars::{RawVariable, RawVariableMap},
};

fn default_version() -> String {
    "1".into()
}

#[derive(Deserialize, Debug)]
pub struct DigConfig {
    #[serde(default = "default_version")]
    pub version: String,
    pub vars: Option<RawVariableMap>,
    pub tasks: BTreeMap<String, TaskConfig>,
}

impl DigConfig {
    #[allow(dead_code)]
    pub fn new() -> DigConfig {
        DigConfig {
            version: default_version(),
            vars: None,
            tasks: BTreeMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn insert_raw_variable(&mut self, key: String, value: RawVariable) {
        match &mut self.vars {
            Some(vars) => {
                vars.insert(key, value);
            }
            None => {
                let mut vars = RawVariableMap::new();
                vars.insert(key, value);
                self.vars = Some(vars);
            }
        }
    }

    pub fn load_yaml(source: &String) -> Result<Self> {
        let f = std::fs::File::open(source)?;
        let config: DigConfig = serde_yaml::from_reader(f)?;
        Ok(config)
    }

    pub fn get_task(&self, key: &str) -> Result<&TaskConfig> {
        match self.tasks.get(key) {
            Some(val) => Ok(val),
            None => Err(anyhow!("Unknown task '{}'", key)),
        }
    }

    // pub fn build_task_evaluation<'a>(
    //     &'a self,
    //     key: &str,
    //     global_vars: &ProcessedVarValueMap,
    //     forcing_context: ForcingContext,
    // ) -> Result<TaskEvaluation<'a>> {
    //     let task = self.get_task(key)?;
    //     task.build_task_evaluation(
    //         key,
    //         Some(global_vars),
    //         VarPriority::DefinedByConfig,
    //         0,
    //         forcing_context,
    //     )
    // }
}
