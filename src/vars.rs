use crate::step::common::{CommandConfig, StepEvaluationResult, StepMethods};
use crate::token::TokenedJsonValue;
use anyhow::{anyhow, bail, Error, Result};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap as Map;

pub type VariableMap = Map<String, JsonValue>;
pub type VariableMapStack<'a> = Vec<&'a VariableMap>;
pub trait VariableMapStackTrait {
    // blahhhh
    fn get_key(&self, key: &str) -> Result<&JsonValue>;
}
pub fn no_vars<'a>() -> VariableMapStack<'a> {
    Vec::new()
}
pub fn no_overrides() -> VariableMap {
    VariableMap::new()
}

impl<'s> VariableMapStackTrait for VariableMapStack<'s> {
    fn get_key(&self, key: &str) -> Result<&'s JsonValue> {
        for vars in self.iter().rev() {
            match vars.get(key) {
                Some(value) => {
                    return Ok(value);
                }
                None => (),
            }
        }
        Err(anyhow!("Failed to insert key '{}'", key))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum RawVariable {
    Executable(CommandConfig),
    Json(JsonValue),
}

impl RawVariable {
    pub fn evaluate(&self, var_stack: &VariableMapStack) -> Result<JsonValue> {
        let output = match &self {
            RawVariable::Json(json_value) => json_value.evaluate_tokens(var_stack)?,
            RawVariable::Executable(command) => match command.evaluate(0, var_stack)? {
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

pub trait RawVariableMapTrait {
    fn evaluate(
        &self,
        var_stack: &VariableMapStack,
        var_overrides: &VariableMap,
        copy_all_overrides: bool, // existing_vars: Option<VariableMap>,
    ) -> Result<VariableMap>;
    fn as_option(&self) -> Option<&RawVariableMap>;
}
impl RawVariableMapTrait for RawVariableMap {
    fn evaluate(
        &self,
        var_stack: &VariableMapStack,
        var_overrides: &VariableMap,
        copy_all_overrides: bool, // existing_vars: Option<VariableMap>,
    ) -> Result<VariableMap> {
        let mut output = match copy_all_overrides {
            false => VariableMap::new(),
            true => var_overrides.clone(),
        };

        for (keytoken, rawvalue) in self.iter() {
            let keyvalue = {
                match var_overrides.get(keytoken) {
                    Some(val) => match copy_all_overrides {
                        true => Ok::<Option<(String, JsonValue)>, Error>(None),
                        false => Ok(Some((keytoken.clone(), val.clone()))),
                    },
                    None => {
                        let mut _var_stack = var_stack.clone(); // TODO: Does this clone the underlying data, or just the pointers?
                        _var_stack.push(&output);
                        let key = match keytoken.evaluate_tokens(&_var_stack)? {
                            JsonValue::String(val) => val,
                            other => bail!("We expected a string, not '{}'", other),
                        };
                        let value = rawvalue.evaluate(&_var_stack)?;
                        Ok(Some((key, value)))
                    }
                }
            }?;

            match keyvalue {
                None => (),
                Some((key, value)) => {
                    output.insert(key.clone(), value);
                }
            }
        }

        Ok(output)
    }

    fn as_option(&self) -> Option<&RawVariableMap> {
        Some(self)
    }
}

#[cfg(test)]
mod test {
    use crate::step::python_step::PythonStep;

    use super::*;
    use anyhow::anyhow;
    use serde_json::json;

    fn type_of<T>(_: &T) -> &'static str {
        std::any::type_name::<T>()
    } //

    #[test]
    fn evaluate_raw_variable_map() -> Result<()> {
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

        // Evaluate
        let evaluated = raw_var_map.evaluate(&no_vars(), &no_overrides(), false)?;

        let result = evaluated
            .get("fixed_int")
            .ok_or(anyhow!("bad 'fixed_int'"))?;
        assert_eq!(result, &json![22]);

        let result = evaluated
            .get("token_str")
            .ok_or(anyhow!("bad 'token_str'"))?;
        assert_eq!(result, &json!["papa loves mama"]);

        let result = evaluated
            .get("token_key_22")
            .ok_or(anyhow!("bad 'token_str'"))?;
        assert_eq!(result, &json![5]);

        let result = match evaluated
            .get("nested_token_map")
            .ok_or(anyhow!("bad 'nested_key'"))?
        {
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
    fn test_command_var() -> Result<()> {
        let mut rawvars = RawVariableMap::new();
        rawvars.insert("fixed_key".into(), RawVariable::Json(json!["dyn_key"]));
        rawvars.insert(
            "{{fixed_key}}".into(),
            RawVariable::Executable(CommandConfig::Python(PythonStep::new("print(\"19\")"))),
        );

        let output = rawvars.evaluate(&no_vars(), &no_overrides(), false)?;

        let value = output
            .get("dyn_key")
            .ok_or(anyhow!("Expected 'dyn_key' to be available"))?;

        assert_eq!(value, &json![19]);

        Ok(())
    }
}
