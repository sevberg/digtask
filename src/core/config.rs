use std::collections::{BTreeMap, HashMap};

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_yaml;

use crate::core::{
    task::TaskConfig,
    vars::{RawVariable, RawVariableMap},
};

pub type EnvConfig = Option<HashMap<String, String>>;
pub type EnvConfigRef<'a> = Option<&'a HashMap<String, String>>;
pub type DirConfig = Option<String>;
pub type DirConfigRef<'a> = Option<&'a String>;

fn default_version() -> String {
    "1".into()
}

#[derive(Deserialize, Debug)]
pub struct DigConfig {
    #[serde(default = "default_version")]
    pub version: String,
    pub vars: Option<RawVariableMap>,
    pub tasks: BTreeMap<String, TaskConfig>,
    pub env: EnvConfig,
    pub dir: DirConfig,
}

impl DigConfig {
    #[allow(dead_code)]
    pub fn new() -> DigConfig {
        DigConfig {
            version: default_version(),
            vars: None,
            tasks: BTreeMap::new(),
            env: None,
            dir: None,
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
}
