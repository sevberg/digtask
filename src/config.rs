use std::collections::BTreeMap;

use anyhow::Result;
use serde::Deserialize;
use serde_yaml;

use crate::{
    // task::{ForcingContext, Task, TaskEvaluation},
    task::TaskConfig,
    vars::RawVariableMap,
};

fn default_version() -> String {
    "1".into()
}

#[derive(Deserialize, Debug)]
pub struct RequeueConfig {
    #[serde(default = "default_version")]
    pub version: String,
    pub vars: Option<RawVariableMap>,
    pub tasks: BTreeMap<String, TaskConfig>,
}

impl RequeueConfig {
    pub fn load_yaml(source: &String) -> Result<Self> {
        let f = std::fs::File::open(source)?;
        let config: RequeueConfig = serde_yaml::from_reader(f)?;
        Ok(config)
    }

    // pub fn get_task(&self, key: &str) -> Result<&Task> {
    //     match self.tasks.get(key) {
    //         Some(val) => Ok(val),
    //         None => Err(anyhow!("Unknown task '{}'", key)),
    //     }
    // }

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
