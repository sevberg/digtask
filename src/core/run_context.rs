use crate::core::{
    config::{DirConfig, DirConfigRef, EnvConfig, EnvConfigRef},
    token::TokenedJsonValue,
    vars::VariableSet,
};
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::{collections::HashMap, path::Path};

#[derive(Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum ForcingContext {
    NotForced,
    ParentIsForced,
    ExplicitlyForced,
    ForcedAsMainTask,
    EverythingForced,
}

#[derive(Deserialize, Debug, Clone, Copy)]
pub enum ForcingBehaviour {
    Never,
    Always,
    Inherit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RunContext {
    pub forcing: ForcingContext,
    pub env: EnvConfig,
    pub dir: DirConfig,
    pub silent: bool,
}

impl RunContext {
    #[allow(dead_code)]
    pub fn default() -> Self {
        RunContext {
            forcing: ForcingContext::NotForced,
            env: None,
            dir: None,
            silent: false,
        }
    }

    pub fn new(
        forcing: &ForcingContext,
        env: EnvConfigRef,
        dir: DirConfigRef,
        vars: &VariableSet,
    ) -> Result<Self> {
        let mut context = RunContext::default();
        context.forcing = *forcing;
        context.update_dir(dir, vars)?;
        context.update_env(env, vars)?;
        Ok(context)
    }

    pub fn child_context(&self, child_behavior: ForcingBehaviour) -> Self {
        let forcing = match &self.forcing {
            ForcingContext::EverythingForced => ForcingContext::EverythingForced,
            ForcingContext::ForcedAsMainTask => ForcingContext::ExplicitlyForced,
            ForcingContext::ExplicitlyForced => match child_behavior {
                ForcingBehaviour::Always => ForcingContext::ExplicitlyForced,
                ForcingBehaviour::Inherit => ForcingContext::ExplicitlyForced,
                ForcingBehaviour::Never => ForcingContext::NotForced,
            },
            ForcingContext::ParentIsForced => match child_behavior {
                ForcingBehaviour::Always => ForcingContext::ExplicitlyForced,
                ForcingBehaviour::Inherit => ForcingContext::ParentIsForced,
                ForcingBehaviour::Never => ForcingContext::NotForced,
            },
            ForcingContext::NotForced => match child_behavior {
                ForcingBehaviour::Always => ForcingContext::ExplicitlyForced,
                ForcingBehaviour::Inherit => ForcingContext::NotForced,
                ForcingBehaviour::Never => ForcingContext::NotForced,
            },
        };

        RunContext {
            forcing,
            env: self.env.clone(),
            dir: self.dir.clone(),
            silent: self.silent,
        }
    }

    pub fn is_forced(&self) -> bool {
        match self.forcing {
            ForcingContext::EverythingForced => true,
            ForcingContext::ForcedAsMainTask => true,
            ForcingContext::ExplicitlyForced => true,
            ForcingContext::ParentIsForced => false,
            ForcingContext::NotForced => false,
        }
    }

    pub fn update(
        &mut self,
        env: EnvConfigRef,
        dir: DirConfigRef,
        silent: bool,
        vars: &VariableSet,
    ) -> Result<()> {
        self.update_env(env, vars)?;
        self.update_dir(dir, vars)?;
        self.silent = self.silent || silent;

        Ok(())
    }

    fn update_env(&mut self, env: EnvConfigRef, vars: &VariableSet) -> Result<()> {
        let env = match env {
            None => None,
            Some(envmap) => {
                let mut output_envmap: HashMap<String, String> = HashMap::new();
                envmap
                    .iter()
                    .map(|(key, val)| {
                        let key = key.evaluate_tokens_to_string("env-key", vars)?;
                        let val = val.evaluate_tokens_to_string("env-value", vars)?;
                        output_envmap.insert(key, val);
                        Ok(())
                    })
                    .collect::<Result<Vec<()>>>()?;

                Some(output_envmap)
            }
        };

        match &mut self.env {
            None => self.env = env,
            Some(self_env) => match env {
                None => (),
                Some(env) => env.into_iter().for_each(|(k, v)| {
                    self_env.insert(k, v);
                }),
            },
        }

        Ok(())
    }

    fn update_dir(&mut self, dir: DirConfigRef, vars: &VariableSet) -> Result<()> {
        match dir {
            None => (),
            Some(specified_dir) => {
                let specified_dir = specified_dir.evaluate_tokens_to_string("dir", vars)?;
                let path = Path::new(specified_dir.as_str());

                if !path.is_dir() {
                    return Err(anyhow!("Invalid directory '{}'", specified_dir));
                }

                self.dir = Some(specified_dir);
            }
        };

        Ok(())
    }
}
