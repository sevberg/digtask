use std::collections::HashMap;

use anyhow::{anyhow, bail, Result};

use serde_json::Value as JsonValue;
use winnow::combinator::{alt, delimited};
use winnow::token::{any, take_till, take_until, take_while};
use winnow::{PResult, Parser};

use crate::vars::{VariableMapStack, VariableMapStackTrait};

#[derive(Debug)]
enum ParsedElement<'s> {
    Token(&'s str),
    Literal(&'s str),
}

fn is_control_char(c: char) -> bool {
    let output = c == '{' || c == '/';
    output
}

fn parse_token<'s>(input: &mut &'s str) -> PResult<ParsedElement<'s>> {
    let output = delimited(
        "{{",
        take_while(1.., ('a'..='z', 'A'..='Z', '0'..='9', '.', ' ', '_', '-')),
        "}}",
    )
    .parse_next(input)?;
    Ok(ParsedElement::Token(output.trim()))
}
fn parse_comment<'s>(input: &mut &'s str) -> PResult<ParsedElement<'s>> {
    let output = delimited("/*", take_until(0.., "*/"), "*/").parse_next(input)?;
    Ok(ParsedElement::Literal(output))
}

fn parse_literal<'s>(input: &mut &'s str) -> PResult<ParsedElement<'s>> {
    let stored_input = *input;
    let (_, remainder) = (any, take_till(0.., is_control_char)).parse_next(input)?;

    let total_length = 1 + remainder.len();
    let output = &stored_input[..total_length];
    Ok(ParsedElement::Literal(output))
}

fn parse_element<'s>(input: &mut &'s str) -> PResult<ParsedElement<'s>> {
    let output = alt((parse_token, parse_comment, parse_literal)).parse_next(input);
    output
}
fn parse_all_elements<'s>(input: &'s str) -> PResult<Vec<ParsedElement<'s>>> {
    let mut input = input;
    let mut output = Vec::new();
    while !input.is_empty() {
        let (remainder, element) = parse_element.parse_peek(input)?;
        output.push(element);
        input = remainder;
    }
    Ok(output)
}

fn evaluate_tokens(input: &str, var_stack: &Vec<&HashMap<String, JsonValue>>) -> Result<JsonValue> {
    // Begin Parsing
    let mut elements = match parse_all_elements(input) {
        Ok(val) => val,
        Err(error) => return Err(anyhow!("{:?}", error)),
    };

    // Check for lone token or literal
    let output = match elements.len() {
        0 => JsonValue::Null,
        1 => match elements.pop().unwrap() {
            ParsedElement::Token(key) => var_stack.get_key(key)?.clone(),
            ParsedElement::Literal(value) => JsonValue::String(value.to_string()),
        },
        _ => {
            // Build output string stack
            let mut string_stack = Vec::new();
            for element in elements.into_iter() {
                match element {
                    ParsedElement::Literal(val) => string_stack.push(val.to_string()),
                    ParsedElement::Token(key) => {
                        let value = match var_stack.get_key(key)? {
                            JsonValue::String(str_value) => str_value.clone(),
                            non_str_value => serde_json::to_string(non_str_value)?,
                        };
                        string_stack.push(value)
                    }
                }
            }

            // Finalize
            JsonValue::String(string_stack.join(""))
        }
    };

    Ok(output)
}

pub trait TokenedJsonValue {
    fn evaluate_tokens(&self, var_stack: &VariableMapStack) -> Result<JsonValue>;
}

impl TokenedJsonValue for String {
    fn evaluate_tokens(&self, var_stack: &VariableMapStack) -> Result<JsonValue> {
        return evaluate_tokens(self, var_stack);
    }
}
impl TokenedJsonValue for JsonValue {
    fn evaluate_tokens(&self, var_stack: &VariableMapStack) -> Result<JsonValue> {
        let output = match self {
            JsonValue::Object(valmap) => {
                let mut output = serde_json::Map::new();
                for (key, val) in valmap.iter() {
                    let detokened_key = match evaluate_tokens(key, var_stack)? {
                        JsonValue::String(val) => val,
                        other => bail!(
                            "Map keys should always map to strings: '{}' became '{}'",
                            key,
                            other
                        ),
                    };
                    let detokened_value = val.evaluate_tokens(var_stack)?;
                    output.insert(detokened_key, detokened_value);
                }
                JsonValue::Object(output)
            }
            JsonValue::Array(valarr) => {
                let mut output = Vec::new();
                for val in valarr.iter() {
                    let detokened_value = val.evaluate_tokens(var_stack)?;
                    output.push(detokened_value);
                }
                JsonValue::Array(output)
            }
            JsonValue::String(valstr) => valstr.evaluate_tokens(var_stack)?,
            other => other.clone(),
        };
        Ok(output)
    }
}

#[cfg(test)]
mod test {
    // use crate::vars::{ReferenceVariableMap, VariableMap, VariableMapLike, NO_VARS};

    use serde_json::json;

    use crate::vars::{no_vars, VariableMap};

    use super::*;

    #[test]
    fn test_multiline() -> Result<()> {
        let vars = HashMap::from([
            ("NAME".to_string(), json!("bob")),
            ("NUM".to_string(), json!(3.0)),
        ]);
        let var_stack = vec![&vars];

        let raw = json!(
            "import math
import json
print(json.dumps({ \"{{NAME}}\": math.sqrt( {{NUM}} )}))"
        );

        let output = raw.evaluate_tokens(&var_stack)?;

        let expected = json!(
            "import math
import json
print(json.dumps({ \"bob\": math.sqrt( 3.0 )}))"
        );
        assert_eq!(output, expected);

        Ok(())
    }

    #[test]
    fn test_token_parsing() -> Result<()> {
        // Build testing variable containers
        let namespace_1_vars = HashMap::from([
            ("key_1".to_string(), JsonValue::String("val_1".to_string())),
            ("key_2".to_string(), JsonValue::String("val_2".to_string())),
        ]);
        let namespace_2_vars = HashMap::from([
            (
                "key_1".to_string(),
                JsonValue::String("val_1_updated".to_string()),
            ),
            ("key_3".to_string(), JsonValue::String("val_3".to_string())),
            ("key_4".to_string(), JsonValue::String("val_4".to_string())),
        ]);
        let task_vars = HashMap::from([
            ("key_5".to_string(), JsonValue::String("val_5".to_string())),
            (
                "key_2".to_string(),
                JsonValue::String("val_2_updated".to_string()),
            ),
        ]);
        let var_stack = vec![&namespace_1_vars, &namespace_2_vars, &task_vars];

        // Begin Parsing
        let mut input =
            "{{key_1}} meep \"{{key_2 }}\" or {{  key_3}} but/*{{key_4}}*/ {{key_5}} okay?";

        let output = evaluate_tokens(&mut input, &var_stack)?;
        assert_eq!(
            output,
            JsonValue::String(
                "val_1_updated meep \"val_2_updated\" or val_3 but{{key_4}} val_5 okay?"
                    .to_string()
            )
        );
        println!("{}", output);

        Ok(())
    }

    #[test]
    fn evaluate_without_vars() -> Result<()> {
        let input = json!["just a string"];
        let output = input.evaluate_tokens(&no_vars())?;

        assert_eq!(output, input);
        Ok(())
    }

    #[test]
    fn evaluate_with_explicit_vars() -> Result<()> {
        let mut varmap = VariableMap::new();
        varmap.insert("whatami".into(), "tea pot".into());

        let token = json!["I am a {{whatami}}"];
        let output = token.evaluate_tokens(&vec![&varmap])?;

        assert_eq!(output, "I am a tea pot".to_string());
        Ok(())
    }
}
