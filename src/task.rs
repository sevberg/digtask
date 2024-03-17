use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    fs,
    time::SystemTime,
};

use crate::{
    config::RequeueConfig,
    step::common::StepConfig,
    token::TokenedJsonValue,
    vars::{RawVariableMap, VariableMap}, // vars::{process_raw_vars, ProcessedVarValueMap, RawVarValueMap, VarPriority},
};

use colored::Colorize;

fn default_false() -> bool {
    true
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum ForcingContext {
    NotForced,
    ExplicitlyForced,
    ParentIsForced,
}

#[derive(Deserialize, Debug)]
pub enum ForcingBehaviour {
    Never,
    Always,
    Inherit,
}
fn default_forcing() -> ForcingBehaviour {
    ForcingBehaviour::Inherit
}

#[derive(Deserialize, Debug)]
pub struct TaskConfig {
    pub label: Option<String>,
    pub steps: Vec<StepConfig>, // Vec<TaskStep>,
    pub inputs: Option<Vec<String>>,
    pub outputs: Option<Vec<String>>,
    #[serde(default = "default_false")]
    pub silent: bool,
    pub vars: Option<RawVariableMap>,
    #[serde(default = "default_forcing")]
    pub forcing: ForcingBehaviour,
}

// impl TaskConfig {
//     pub fn build_vars(&self, parent_vars: Option<Vec<ReferenceVariableMap>>) -> Result<(VariableMap, ReferenceVariableMap)>
//     {
//         let parent_vars = match parent_vars {
//             None => ReferenceVariableMap::new(),
//             Some(parent_vars) => parent_vars.stack_references()
//         };

//         let task_vars = match &self.vars {
//             None => VariableMap::new(),
//             Some(rawvars) => rawvars.evaluate(parent_vars.as_option())?
//         };

//         Ok((task_vars, parent_vars ))
//     }

//     pub fn evaluate(&self, parent_vars: Option<Vec<ReferenceVariableMap>>) -> Result<PreparedTask>
//     {
//         let (task_vars, parent_vars) = self.build_vars(parent_vars)?

//         // let name

//         let output = PreparedTask{
//             label: None,
//             steps: None,
//             inputs: None,
//             outputs: None,
//             silent: None,
//             vars: None,
//             forcing: None,
//         }
//         Ok(())
//     }
// }

#[derive(Deserialize, Debug)]
pub struct PreparedTask {
    pub label: String,
    pub steps: Vec<StepConfig>, // Vec<TaskStep>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    #[serde(default = "default_false")]
    pub silent: bool,
    pub task_vars: VariableMap,
    pub parent_vars: VariableMap,
    #[serde(default = "default_forcing")]
    pub forcing: ForcingBehaviour,
}
