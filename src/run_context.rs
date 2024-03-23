use crate::{
    config::{DirConfig, EnvConfig},
    task::ForcingContext,
    token::TokenedJsonValue,
    vars::VariableSet,
};
use anyhow::{anyhow, Result};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Clone, PartialEq)]
pub struct RunContext {
    pub forcing: ForcingContext,
    pub env: EnvConfig,
    pub dir: DirConfig,
}

impl RunContext {
    #[allow(dead_code)]
    pub fn default() -> Self {
        RunContext {
            forcing: ForcingContext::NotForced,
            env: None,
            dir: None,
        }
    }

    pub fn new(
        forcing: &ForcingContext,
        env: &EnvConfig,
        dir: &DirConfig,
        vars: &VariableSet,
    ) -> Result<Self> {
        let mut context = RunContext::default();
        context.forcing = forcing.clone();
        context.update_dir(dir, vars)?;
        context.update_env(env, vars)?;
        Ok(context)
    }

    pub fn child_context(&self) -> Self {
        let forcing = match &self.forcing {
            ForcingContext::EverythingForced => ForcingContext::EverythingForced,
            ForcingContext::ExplicitlyForced => ForcingContext::ParentIsForced,
            ForcingContext::ParentIsForced => ForcingContext::NotForced,
            ForcingContext::NotForced => ForcingContext::NotForced,
        };

        RunContext {
            forcing,
            env: self.env.clone(),
            dir: self.dir.clone(),
        }
    }

    pub fn update_env(&mut self, env: &EnvConfig, vars: &VariableSet) -> Result<()> {
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

    pub fn update_dir(&mut self, dir: &DirConfig, vars: &VariableSet) -> Result<()> {
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
