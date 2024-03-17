// use crate::step::common::{StepConfig, StepEvaluationResult};
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

impl<'s> VariableMapStackTrait for VariableMapStack<'s> {
    // blloooo
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum RawVariable {
    // Executable(StepConfig),
    Json(JsonValue),
}

impl RawVariable {
    pub fn evaluate(&self, var_stack: &VariableMapStack) -> Result<JsonValue> {
        let output = match &self {
            RawVariable::Json(json_value) => json_value.evaluate_tokens(var_stack),
            // RawVariable::Executable(command) => match command.evaluate(var_stack)? {
            //     StepEvaluationResult::CompletedWithOutput(val) => val,
            //     _ => bail!("Command did not result in an output"),
            // },
        }?;

        Ok(output)
    }
}

// type RawVariableKeyValueSet<'a> = (&'a Token, &'a RawVariable);
// trait RawVariableKeyValueSetTrait {
//     fn evaluate<T>(&self, vars: Option<&T>) -> Result<(String, Variable)>
//     where
//         T: Serialize;
// }
// impl RawVariableKeyValueSetTrait for RawVariableKeyValueSet<'_> {
//     fn evaluate<T>(&self, vars: Option<&T>) -> Result<(String, Variable)>
//     where
//         T: Serialize,
//     {
//         let key = self.0.evaluate(vars)?;
//         let value = self.1.evaluate(vars)?;
//         Ok((key, value))
//     }
// }

pub type RawVariableMap = IndexMap<String, RawVariable>;

pub trait RawVariableMapTrait {
    fn evaluate(&self, var_stack: &VariableMapStack) -> Result<VariableMap>;
    fn as_option(&self) -> Option<&RawVariableMap>;
}
impl RawVariableMapTrait for RawVariableMap {
    fn evaluate(&self, var_stack: &VariableMapStack) -> Result<VariableMap> {
        let mut output = VariableMap::new();

        for (keytoken, rawvalue) in self.iter() {
            let (key, value) = {
                let mut _var_stack = var_stack.clone(); // TODO: Does this clone the underlying data, or just the pointers?
                _var_stack.push(&output);
                let key = match keytoken.evaluate_tokens(&_var_stack)? {
                    JsonValue::String(val) => val,
                    other => bail!("We expected a string, not '{}'", other),
                };
                let value = rawvalue.evaluate(&_var_stack)?;
                Ok::<(String, JsonValue), Error>((key, value))
            }?;

            output.insert(key.clone(), value);
        }

        Ok(output)
    }

    fn as_option(&self) -> Option<&RawVariableMap> {
        Some(self)
    }
}

// #[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
// #[serde(untagged)]
// pub enum Variable {
//     SingleString(String),
//     SingleInt(i32),
//     SingleFloat(f64),
//     ManyStrings(Vec<String>),
//     ManyInts(Vec<i32>),
//     ManyFloats(Vec<f64>),
//     Map(Map<String, Variable>), // Does this work???
// }

// impl From<String> for Variable {
//     fn from(item: String) -> Self {
//         Variable::SingleString(item)
//     }
// }

// impl From<&str> for Variable {
//     fn from(item: &str) -> Self {
//         Variable::SingleString(item.to_string())
//     }
// }

// impl From<i32> for Variable {
//     fn from(item: i32) -> Self {
//         Variable::SingleInt(item)
//     }
// }

// impl From<f64> for Variable {
//     fn from(item: f64) -> Self {
//         Variable::SingleFloat(item)
//     }
// }

// impl From<Vec<String>> for Variable {
//     fn from(item: Vec<String>) -> Self {
//         Variable::ManyStrings(item)
//     }
// }

// impl From<Vec<i32>> for Variable {
//     fn from(item: Vec<i32>) -> Self {
//         Variable::ManyInts(item)
//     }
// }

// impl From<Vec<f64>> for Variable {
//     fn from(item: Vec<f64>) -> Self {
//         Variable::ManyFloats(item)
//     }
// }

// impl From<Map<String, Variable>> for Variable {
//     fn from(item: Map<String, Variable>) -> Self {
//         Variable::Map(item)
//     }
// }

// pub type VariableMap = Map<String, Variable>;
// pub const NO_VARS: Option<&VariableMap> = None;

// pub trait VariableMapLike {
//     fn clone_as_reference_map(&self) -> ReferenceVariableMap;
//     fn clone_as_map(&self) -> VariableMap;
//     fn as_option(&self) -> Option<&Self>;
// }
// impl VariableMapLike for VariableMap {
//     fn clone_as_reference_map(&self) -> ReferenceVariableMap {
//         let mut ref_var_map: ReferenceVariableMap = Map::new();
//         for (key, value) in self.iter() {
//             ref_var_map.insert(key.clone(), value);
//         }

//         ref_var_map
//     }
//     fn clone_as_map(&self) -> VariableMap {
//         self.clone()
//     }
//     fn as_option(&self) -> Option<&VariableMap> {
//         Some(self)
//     }
// }

// pub type ReferenceVariableMap<'a> = Map<String, &'a Variable>;

// impl<'a> VariableMapLike for ReferenceVariableMap<'a> {
//     fn clone_as_reference_map(&self) -> ReferenceVariableMap {
//         self.clone()
//     }

//     fn clone_as_map(&self) -> VariableMap {
//         let mut var_map: VariableMap = Map::new();
//         for (key, value) in self.iter() {
//             var_map.insert(key.clone(), (*value).clone());
//         }

//         var_map
//     }

//     fn as_option(&self) -> Option<&ReferenceVariableMap<'a>> {
//         Some(self)
//     }
// }

// pub trait VariableMapVectorOperations {
//     fn stack_references(&self) -> ReferenceVariableMap;
// }
// impl VariableMapVectorOperations for Vec<&VariableMap> {
//     fn stack_references(&self) -> ReferenceVariableMap {
//         let mut flat_var_map: ReferenceVariableMap = Map::new();
//         for varmap in self.iter() {
//             for (key, value) in varmap.iter() {
//                 flat_var_map.insert(key.clone(), value);
//             }
//         }

//         flat_var_map
//     }
// }
// impl<'a> VariableMapVectorOperations for Vec<ReferenceVariableMap<'a>> {
//     fn stack_references(&self) -> ReferenceVariableMap<'a> {
//         let mut flat_var_map: ReferenceVariableMap = Map::new();
//         for varmap in self.iter() {
//             for (key, value) in varmap.iter() {
//                 flat_var_map.insert(key.clone(), value);
//             }
//         }

//         flat_var_map
//     }
// }
// impl<'a> VariableMapVectorOperations for Vec<&ReferenceVariableMap<'a>> {
//     fn stack_references(&self) -> ReferenceVariableMap<'a> {
//         let mut flat_var_map: ReferenceVariableMap = Map::new();
//         for varmap in self.iter() {
//             for (key, value) in varmap.iter() {
//                 flat_var_map.insert(key.clone(), value);
//             }
//         }

//         flat_var_map
//     }
// }

#[cfg(test)]
mod test {
    // use crate::step::python_step::PythonStep;

    use super::*;
    use anyhow::anyhow;

    fn type_of<T>(_: &T) -> &'static str {
        std::any::type_name::<T>()
    } //

    // #[test]
    // fn evaluate_raw_variable_map() -> Result<()> {
    //     // Build Raw variable map
    //     let mut raw_var_map = RawVariableMap::new();
    //     raw_var_map.insert("fixed_int".into(), 22.into());
    //     raw_var_map.insert("fixed_str".into(), RawVariable::SingleToken("mama".into()));
    //     raw_var_map.insert(
    //         "token_str".into(),
    //         RawVariable::SingleToken("papa loves {{fixed_str}}".into()),
    //     );
    //     raw_var_map.insert("fixed_array".into(), vec![1, 2, 3].into());
    //     raw_var_map.insert("token_key_{{fixed_int}}".into(), 5.into());

    //     let mut nested_token_map = RawVariableMap::new();
    //     nested_token_map.insert("nested_key_{{fixed_str}}".into(), "{{fixed_array}}".into());
    //     raw_var_map.insert(
    //         "nested_token_map".into(),
    //         RawVariable::Map(nested_token_map),
    //     );

    //     // Evaluate
    //     let evaluated = raw_var_map.evaluate(NO_VARS)?;

    //     let result = evaluated
    //         .get("fixed_int")
    //         .ok_or(anyhow!("bad 'fixed_int'"))?;
    //     assert_eq!(result, &Variable::SingleInt(22));

    //     let result = evaluated
    //         .get("token_str")
    //         .ok_or(anyhow!("bad 'token_str'"))?;
    //     assert_eq!(result, &Variable::SingleString("papa loves mama".into()));

    //     let result = evaluated
    //         .get("token_key_22")
    //         .ok_or(anyhow!("bad 'token_str'"))?;
    //     assert_eq!(result, &Variable::SingleInt(5));

    //     let result = match evaluated
    //         .get("nested_token_map")
    //         .ok_or(anyhow!("bad 'nested_key'"))?
    //     {
    //         Variable::Map(nested_result) => nested_result
    //             .get("nested_key_mama")
    //             .ok_or(anyhow!("bad 'nested_key'"))?,
    //         other => {
    //             return Err(anyhow!(
    //                 "Nested map is not a map. It's a '{}'",
    //                 type_of(other)
    //             ))
    //         }
    //     };
    //     assert_eq!(result, &Variable::SingleString("[1, 2, 3]".into()));

    //     Ok(())
    // }

    // #[test]
    // fn consolidate_var_maps() -> Result<()> {
    //     let mut namespace_varmap = VariableMap::new();
    //     namespace_varmap.insert("key_1".into(), "namespace_val".into());
    //     namespace_varmap.insert("key_2".into(), 17.into());
    //     namespace_varmap.insert("key_3".into(), vec![13.0, 15.7].into());

    //     let mut task_varmap = VariableMap::new();
    //     task_varmap.insert("key_1".into(), "task_val".into());

    //     let varmap_vec = vec![&namespace_varmap, &task_varmap];
    //     let stacked_varmap = varmap_vec.stack_references();

    //     // Test basic Variable access
    //     let key_1_val = stacked_varmap
    //         .get("key_1".into())
    //         .ok_or(anyhow!("Missing 'key_1'"))?;
    //     let key_1_expected: Variable = "task_val".into();
    //     assert_eq!(*key_1_val, &key_1_expected);

    //     let key_2_val = stacked_varmap
    //         .get("key_2".into())
    //         .ok_or(anyhow!("Missing 'key_2'"))?;
    //     let key_2_expected: Variable = 17.into();
    //     assert_eq!(*key_2_val, &key_2_expected);

    //     let key_3_val = stacked_varmap
    //         .get("key_3".into())
    //         .ok_or(anyhow!("Missing 'key_3'"))?;
    //     let key_3_expected: Variable = vec![13.0, 15.7].into();
    //     assert_eq!(*key_3_val, &key_3_expected);

    //     // Test Usage in a Token resolution
    //     let token: Token =
    //         "{{key_1}} is {{lookup key_3 1}} years old and has {{key_2}} toys".into();
    //     let token_result = token.evaluate(stacked_varmap.as_option())?;
    //     let token_expected = "task_val is 15.7 years old and has 17 toys".to_string();
    //     assert_eq!(token_result, token_expected);

    //     // Done!
    //     Ok(())
    // }

    // #[test]
    // fn test_command_var() -> Result<()> {
    //     let mut rawvars = RawVariableMap::new();
    //     rawvars.insert("fixed_key".into(), "dyn_key".into());
    //     rawvars.insert(
    //         "{{fixed_key}}".into(),
    //         RawVariable::Executable(StepConfig::Python(PythonStep::new("print(\"19\")"))),
    //     );

    //     let output = rawvars.evaluate(NO_VARS)?;

    //     let value = output
    //         .get("dyn_key")
    //         .ok_or(anyhow!("Expected 'dyn_key' to be available"))?;

    //     assert_eq!(value, &Variable::SingleInt(19));

    //     Ok(())
    // }
}
