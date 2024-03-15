use anyhow::Result;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap as Map;

use crate::token::Token;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum RawVariable {
    SingleToken(Token),
    SingleInt(i32),
    SingleFloat(f64),
    ManyTokens(Vec<Token>),
    ManyInts(Vec<i32>),
    ManyFloats(Vec<f64>),
    Map(IndexMap<Token, RawVariable>), // Does this work???
}

impl From<Token> for RawVariable {
    fn from(item: Token) -> Self {
        RawVariable::SingleToken(item)
    }
}

impl From<&str> for RawVariable {
    fn from(item: &str) -> Self {
        RawVariable::SingleToken(Token(item.to_string()))
    }
}

impl From<i32> for RawVariable {
    fn from(item: i32) -> Self {
        RawVariable::SingleInt(item)
    }
}

impl From<f64> for RawVariable {
    fn from(item: f64) -> Self {
        RawVariable::SingleFloat(item)
    }
}

impl From<Vec<Token>> for RawVariable {
    fn from(item: Vec<Token>) -> Self {
        RawVariable::ManyTokens(item)
    }
}

impl From<Vec<i32>> for RawVariable {
    fn from(item: Vec<i32>) -> Self {
        RawVariable::ManyInts(item)
    }
}

impl From<Vec<f64>> for RawVariable {
    fn from(item: Vec<f64>) -> Self {
        RawVariable::ManyFloats(item)
    }
}

impl From<IndexMap<Token, RawVariable>> for RawVariable {
    fn from(item: IndexMap<Token, RawVariable>) -> Self {
        RawVariable::Map(item)
    }
}

impl RawVariable {
    pub fn evaluate<T>(&self, vars: Option<&T>) -> Result<Variable>
    where
        T: Serialize,
    {
        let output = match &self {
            RawVariable::SingleToken(obj) => Variable::SingleString(obj.evaluate(vars)?),
            RawVariable::SingleInt(obj) => Variable::SingleInt(obj.clone()),
            RawVariable::SingleFloat(obj) => Variable::SingleFloat(obj.clone()),
            RawVariable::ManyTokens(obj) => Variable::ManyStrings(
                obj.iter()
                    .map(|rawvar| rawvar.evaluate(vars))
                    .collect::<Result<Vec<_>>>()?,
            ),
            RawVariable::ManyInts(obj) => Variable::ManyInts(obj.clone()),
            RawVariable::ManyFloats(obj) => Variable::ManyFloats(obj.clone()),
            &RawVariable::Map(obj) => Variable::Map(
                obj.iter()
                    .map(|(rawkey, rawvar)| Ok((rawkey.evaluate(vars)?, rawvar.evaluate(vars)?)))
                    .collect::<Result<Map<_, _>>>()?,
            ),
        };

        Ok(output)
    }
}

type RawVariableKeyValueSet<'a> = (&'a Token, &'a RawVariable);
trait RawVariableKeyValueSetTrait {
    fn evaluate<T>(&self, vars: Option<&T>) -> Result<(String, Variable)>
    where
        T: Serialize;
}
impl RawVariableKeyValueSetTrait for RawVariableKeyValueSet<'_> {
    fn evaluate<T>(&self, vars: Option<&T>) -> Result<(String, Variable)>
    where
        T: Serialize,
    {
        let key = self.0.evaluate(vars)?;
        let value = self.1.evaluate(vars)?;
        Ok((key, value))
    }
}

pub type RawVariableMap = IndexMap<Token, RawVariable>;

trait RawVariableMapTrait {
    fn evaluate<T>(&self, vars: Option<&T>) -> Result<VariableMap>
    where
        T: Serialize + VariableMapLike;

    fn as_option(&self) -> Option<&RawVariableMap>;
}
impl RawVariableMapTrait for RawVariableMap {
    fn evaluate<T>(&self, vars: Option<&T>) -> Result<VariableMap>
    where
        T: Serialize + VariableMapLike,
    {
        let mut output = VariableMap::new();

        let full_var_context = match vars {
            None => ReferenceVariableMap::new(),
            Some(vars) => vars.clone_as_reference_map(),
        };

        for (keytoken, rawvalue) in self.iter() {
            let _full_var_context_unstacked = vec![
                full_var_context.clone_as_reference_map(),
                output.clone_as_reference_map(),
            ];
            let _full_var_context = _full_var_context_unstacked.stack_references();

            let key = keytoken.evaluate(_full_var_context.as_option())?;
            let value = rawvalue.evaluate(_full_var_context.as_option())?;

            output.insert(key.clone(), value);
        }

        Ok(output)
    }

    fn as_option(&self) -> Option<&RawVariableMap> {
        Some(self)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum Variable {
    SingleString(String),
    SingleInt(i32),
    SingleFloat(f64),
    ManyStrings(Vec<String>),
    ManyInts(Vec<i32>),
    ManyFloats(Vec<f64>),
    Map(Map<String, Variable>), // Does this work???
}

impl From<String> for Variable {
    fn from(item: String) -> Self {
        Variable::SingleString(item)
    }
}

impl From<&str> for Variable {
    fn from(item: &str) -> Self {
        Variable::SingleString(item.to_string())
    }
}

impl From<i32> for Variable {
    fn from(item: i32) -> Self {
        Variable::SingleInt(item)
    }
}

impl From<f64> for Variable {
    fn from(item: f64) -> Self {
        Variable::SingleFloat(item)
    }
}

impl From<Vec<String>> for Variable {
    fn from(item: Vec<String>) -> Self {
        Variable::ManyStrings(item)
    }
}

impl From<Vec<i32>> for Variable {
    fn from(item: Vec<i32>) -> Self {
        Variable::ManyInts(item)
    }
}

impl From<Vec<f64>> for Variable {
    fn from(item: Vec<f64>) -> Self {
        Variable::ManyFloats(item)
    }
}

impl From<Map<String, Variable>> for Variable {
    fn from(item: Map<String, Variable>) -> Self {
        Variable::Map(item)
    }
}

pub type VariableMap = Map<String, Variable>;
pub const NO_VARS: Option<&VariableMap> = None;

pub trait VariableMapLike {
    fn clone_as_reference_map(&self) -> ReferenceVariableMap;
    fn clone_as_map(&self) -> VariableMap;
    fn as_option(&self) -> Option<&Self>;
}
impl VariableMapLike for VariableMap {
    fn clone_as_reference_map(&self) -> ReferenceVariableMap {
        let mut ref_var_map: ReferenceVariableMap = Map::new();
        for (key, value) in self.iter() {
            ref_var_map.insert(key.clone(), value);
        }

        ref_var_map
    }
    fn clone_as_map(&self) -> VariableMap {
        self.clone()
    }
    fn as_option(&self) -> Option<&VariableMap> {
        Some(self)
    }
}

pub type ReferenceVariableMap<'a> = Map<String, &'a Variable>;

impl<'a> VariableMapLike for ReferenceVariableMap<'a> {
    fn clone_as_reference_map(&self) -> ReferenceVariableMap {
        self.clone()
    }

    fn clone_as_map(&self) -> VariableMap {
        let mut var_map: VariableMap = Map::new();
        for (key, value) in self.iter() {
            var_map.insert(key.clone(), (*value).clone());
        }

        var_map
    }

    fn as_option(&self) -> Option<&ReferenceVariableMap<'a>> {
        Some(self)
    }
}

pub trait VariableMapVectorOperations {
    fn stack_references(&self) -> ReferenceVariableMap;
}
impl VariableMapVectorOperations for Vec<&VariableMap> {
    fn stack_references(&self) -> ReferenceVariableMap {
        let mut flat_var_map: ReferenceVariableMap = Map::new();
        for varmap in self.iter() {
            for (key, value) in varmap.iter() {
                flat_var_map.insert(key.clone(), value);
            }
        }

        flat_var_map
    }
}
impl<'a> VariableMapVectorOperations for Vec<ReferenceVariableMap<'a>> {
    fn stack_references(&self) -> ReferenceVariableMap<'a> {
        let mut flat_var_map: ReferenceVariableMap = Map::new();
        for varmap in self.iter() {
            for (key, value) in varmap.iter() {
                flat_var_map.insert(key.clone(), value);
            }
        }

        flat_var_map
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use anyhow::anyhow;

    fn type_of<T>(_: &T) -> &'static str {
        std::any::type_name::<T>()
    } //

    #[test]
    fn evaluate_raw_variable_map() -> Result<()> {
        // Build Raw variable map
        let mut raw_var_map = RawVariableMap::new();
        raw_var_map.insert("fixed_int".into(), 22.into());
        raw_var_map.insert("fixed_str".into(), RawVariable::SingleToken("mama".into()));
        raw_var_map.insert(
            "token_str".into(),
            RawVariable::SingleToken("papa loves {{fixed_str}}".into()),
        );
        raw_var_map.insert("fixed_array".into(), vec![1, 2, 3].into());
        raw_var_map.insert("token_key_{{fixed_int}}".into(), 5.into());

        let mut nested_token_map = RawVariableMap::new();
        nested_token_map.insert("nested_key_{{fixed_str}}".into(), "{{fixed_array}}".into());
        raw_var_map.insert(
            "nested_token_map".into(),
            RawVariable::Map(nested_token_map),
        );

        // Evaluate
        let evaluated = raw_var_map.evaluate(NO_VARS)?;

        let result = evaluated
            .get("fixed_int")
            .ok_or(anyhow!("bad 'fixed_int'"))?;
        assert_eq!(result, &Variable::SingleInt(22));

        let result = evaluated
            .get("token_str")
            .ok_or(anyhow!("bad 'token_str'"))?;
        assert_eq!(result, &Variable::SingleString("papa loves mama".into()));

        let result = evaluated
            .get("token_key_22")
            .ok_or(anyhow!("bad 'token_str'"))?;
        assert_eq!(result, &Variable::SingleInt(5));

        let result = match evaluated
            .get("nested_token_map")
            .ok_or(anyhow!("bad 'nested_key'"))?
        {
            Variable::Map(nested_result) => nested_result
                .get("nested_key_mama")
                .ok_or(anyhow!("bad 'nested_key'"))?,
            other => {
                return Err(anyhow!(
                    "Nested map is not a map. It's a '{}'",
                    type_of(other)
                ))
            }
        };
        assert_eq!(result, &Variable::SingleString("[1, 2, 3]".into()));

        Ok(())
    }

    #[test]
    fn consolidate_var_maps() -> Result<()> {
        let mut namespace_varmap = VariableMap::new();
        namespace_varmap.insert("key_1".into(), "namespace_val".into());
        namespace_varmap.insert("key_2".into(), 17.into());
        namespace_varmap.insert("key_3".into(), vec![13.0, 15.7].into());

        let mut task_varmap = VariableMap::new();
        task_varmap.insert("key_1".into(), "task_val".into());

        let varmap_vec = vec![&namespace_varmap, &task_varmap];
        let stacked_varmap = varmap_vec.stack_references();

        // Test basic Variable access
        let key_1_val = stacked_varmap
            .get("key_1".into())
            .ok_or(anyhow!("Missing 'key_1'"))?;
        let key_1_expected: Variable = "task_val".into();
        assert_eq!(*key_1_val, &key_1_expected);

        let key_2_val = stacked_varmap
            .get("key_2".into())
            .ok_or(anyhow!("Missing 'key_2'"))?;
        let key_2_expected: Variable = 17.into();
        assert_eq!(*key_2_val, &key_2_expected);

        let key_3_val = stacked_varmap
            .get("key_3".into())
            .ok_or(anyhow!("Missing 'key_3'"))?;
        let key_3_expected: Variable = vec![13.0, 15.7].into();
        assert_eq!(*key_3_val, &key_3_expected);

        // Test Usage in a Token resolution
        let token: Token =
            "{{key_1}} is {{lookup key_3 1}} years old and has {{key_2}} toys".into();
        let token_result = token.evaluate(stacked_varmap.as_option())?;
        let token_expected = "task_val is 15.7 years old and has 17 toys".to_string();
        assert_eq!(token_result, token_expected);

        // Done!
        Ok(())
    }
}
