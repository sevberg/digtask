use crate::executor::DigExecutor;
use crate::step::common::{CommandConfig, StepEvaluationResult, StepMethods};
use crate::token::TokenedJsonValue;
use anyhow::{anyhow, bail, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap as Map;
use std::rc::Rc;

pub type VariableMap = Map<String, JsonValue>;
pub type VariableMapStack = Vec<Rc<VariableMap>>;

#[derive(Debug, Clone, PartialEq)]
pub struct VariableSet {
    pub stacked_vars: VariableMapStack,
    pub local_vars: VariableMap,
}

#[derive(Clone, Copy)]
pub enum StackMode {
    EmptyLocals,
    CopyLocals,
}

impl VariableSet {
    pub fn new() -> Self {
        VariableSet {
            stacked_vars: Vec::new(),
            local_vars: VariableMap::new(),
        }
    }

    pub fn get(&self, key: &str) -> Result<&JsonValue> {
        match self.get_from_locals(key) {
            None => (),
            Some(value) => return Ok(value),
        }
        for vars in self.stacked_vars.iter().rev() {
            match vars.get(key) {
                Some(value) => {
                    return Ok(value);
                }
                None => (),
            }
        }
        Err(anyhow!("Failed to get key '{}'", key))
    }

    pub fn get_from_locals(&self, key: &str) -> Option<&JsonValue> {
        match self.local_vars.get(key) {
            Some(value) => {
                return Some(value);
            }
            None => None,
        }
    }

    pub fn get_from_parent(&self, key: &str) -> Option<&JsonValue> {
        match self.stacked_vars.last() {
            Some(parent) => parent.get(key),
            None => None,
        }
    }

    pub fn parent(&self) -> Result<&VariableMap> {
        let parent_ref = match self.stacked_vars.last() {
            Some(val) => val,
            None => return Err(anyhow!("Variable stack is empty")),
        };

        Ok(parent_ref.as_ref())
    }

    pub fn stack(&self, mode: StackMode) -> Self {
        let local_vars = match mode {
            StackMode::EmptyLocals => VariableMap::new(),
            StackMode::CopyLocals => self.local_vars.clone(),
        };

        let mut stacked_vars = self.stacked_vars.clone();
        stacked_vars.push(Rc::new(self.local_vars.clone()));

        VariableSet {
            stacked_vars,
            local_vars,
        }
    }

    pub fn insert_local(&mut self, key: String, value: JsonValue) {
        self.local_vars.insert(key, value);
    }

    pub async fn stack_raw_variables(
        &self,
        raw_vars: &RawVariableMap,
        stack_mode: StackMode,
        executor: &DigExecutor<'_>,
    ) -> Result<Self> {
        let mut output_vars = self.stack(stack_mode);

        for (keytoken, rawvalue) in raw_vars.iter() {
            let keyvalue: Option<(String, JsonValue)> = {
                match output_vars.get_from_parent(keytoken) {
                    Some(value) => match &stack_mode {
                        StackMode::EmptyLocals => Some((keytoken.clone(), value.clone())),
                        StackMode::CopyLocals => None, // Should already be copied
                    },
                    None => {
                        let key =
                            keytoken.evaluate_tokens_to_string("variable key", &output_vars)?;
                        let value = rawvalue.evaluate(&output_vars, executor).await?;
                        Some((key, value))
                    }
                }
            };

            match keyvalue {
                None => (),
                Some((key, value)) => {
                    output_vars.insert_local(key, value);
                }
            }
        }

        Ok(output_vars)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum RawVariable {
    Executable(CommandConfig),
    Json(JsonValue),
}

impl RawVariable {
    pub async fn evaluate(
        &self,
        vars: &VariableSet,
        executor: &DigExecutor<'_>,
    ) -> Result<JsonValue> {
        let output = match &self {
            RawVariable::Json(json_value) => json_value.evaluate_tokens(vars)?,
            RawVariable::Executable(command) => match command.evaluate(0, vars, executor).await? {
                StepEvaluationResult::CompletedWithOutput(val) => val,
                _ => bail!("Command did not result in an output"),
            },
        };

        Ok(output)
    }
}

impl From<JsonValue> for RawVariable {
    fn from(value: JsonValue) -> Self {
        RawVariable::Json(value)
    }
}

pub type RawVariableMap = IndexMap<String, RawVariable>;

#[cfg(test)]
mod test {
    use super::*;
    use crate::step::python_step::PythonStep;

    use anyhow::anyhow;
    use serde_json::json;

    fn type_of<T>(_: &T) -> &'static str {
        std::any::type_name::<T>()
    } //

    #[test]
    fn raw_string_map() -> Result<()> {
        // Build Raw variable map
        let mut raw_var_map = RawVariableMap::new();
        raw_var_map.insert("fixed_int".into(), RawVariable::Json(json![22]));
        raw_var_map.insert("fixed_str".into(), RawVariable::Json(json!["mama"]));
        raw_var_map.insert(
            "token_str".into(),
            RawVariable::Json(json!["papa loves {{fixed_str}}"]),
        );
        raw_var_map.insert(
            "fixed_array".into(),
            RawVariable::Json(json![vec![1, 2, 3]]),
        );
        raw_var_map.insert(
            "token_key_{{fixed_int}}".into(),
            RawVariable::Json(json![5]),
        );

        let mut nested_token_map = RawVariableMap::new();
        nested_token_map.insert(
            "nested_key_{{fixed_str}}".into(),
            RawVariable::Json(json!["{{fixed_array}}"]),
        );
        raw_var_map.insert(
            "nested_token_map".into(),
            RawVariable::Json(json![nested_token_map]),
        );

        // Stack raw variables
        let mut vars = VariableSet::new();
        let executor = DigExecutor::new(1);
        let future = vars.stack_raw_variables(&raw_var_map, StackMode::EmptyLocals, &executor);
        let evaluated = smol::block_on(executor.executor.run(future))?;

        // Assert outputs
        let result = evaluated.get("fixed_int")?;
        assert_eq!(result, &json![22]);

        let result = evaluated.get("token_str")?;
        assert_eq!(result, &json!["papa loves mama"]);

        let result = evaluated.get("token_key_22")?;
        assert_eq!(result, &json![5]);

        let result = match evaluated.get("nested_token_map")? {
            JsonValue::Object(nested_result) => nested_result
                .get("nested_key_mama")
                .ok_or(anyhow!("bad 'nested_key'"))?,
            other => {
                return Err(anyhow!(
                    "Nested map is not a map. It's a '{}'",
                    type_of(other)
                ))
            }
        };
        assert_eq!(result, &json![vec![1, 2, 3]]);

        Ok(())
    }

    #[test]
    fn raw_command_map() -> Result<()> {
        let mut rawvars = RawVariableMap::new();
        rawvars.insert("fixed_key".into(), RawVariable::Json(json!["dyn_key"]));
        rawvars.insert(
            "{{fixed_key}}".into(),
            RawVariable::Executable(CommandConfig::Python(PythonStep::new("print(\"19\")"))),
        );

        // Stack raw variables
        let mut vars = VariableSet::new();
        let executor = DigExecutor::new(1);
        let future = vars.stack_raw_variables(&rawvars, StackMode::EmptyLocals, &executor);
        let evaluated = smol::block_on(executor.executor.run(future))?;

        // Assert outputs
        let value = evaluated.get("dyn_key")?;

        assert_eq!(value, &json![19]);

        Ok(())
    }
}
