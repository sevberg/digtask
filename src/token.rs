use anyhow::Result;
use handlebars::Handlebars;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, PartialOrd, Ord, Eq, Hash)]
pub struct Token(pub String);

impl From<String> for Token {
    fn from(item: String) -> Self {
        Token(item)
    }
}

impl From<&str> for Token {
    fn from(item: &str) -> Self {
        Token(item.to_string())
    }
}

impl Token {
    pub fn evaluate<T>(&self, vars: Option<&T>) -> Result<String>
    where
        T: Serialize,
    {
        let output = match vars {
            Some(vars) => {
                let mut handlebars = Handlebars::new();
                handlebars.set_strict_mode(true);
                handlebars.render_template(self.0.as_str(), vars)?
            }

            None => self.0.clone(),
        };

        Ok(output)
    }
}

#[cfg(test)]
mod test {
    use crate::vars::{ReferenceVariableMap, VariableMap, VariableMapLike, NO_VARS};

    use super::*;

    #[test]
    fn evaluate_without_vars() -> Result<()> {
        let token = Token("just a string".to_string());
        let output = token.evaluate(NO_VARS)?;

        assert_eq!(output, token.0);
        Ok(())
    }

    #[test]
    fn evaluate_with_explicit_vars() -> Result<()> {
        let mut varmap = VariableMap::new();
        varmap.insert("whatami".into(), "tea pot".into());

        let token: Token = "I am a {{whatami}}".into();
        let output = token.evaluate(varmap.as_option())?;

        assert_eq!(output, "I am a tea pot".to_string());
        Ok(())
    }

    #[test]
    fn evaluate_with_referenced_vars() -> Result<()> {
        let var_value = "tea pot".into();
        let mut varmap = ReferenceVariableMap::new();
        varmap.insert("whatami".into(), &var_value);

        let token: Token = "I am a {{whatami}}".into();
        let output = token.evaluate(varmap.as_option())?;

        assert_eq!(output, "I am a tea pot".to_string());
        Ok(())
    }
}
