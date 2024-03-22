use anyhow::{anyhow, bail, Result};

use serde_json::Value as JsonValue;
use winnow::combinator::{alt, delimited};
use winnow::token::{any, take_till, take_until, take_while};
use winnow::{PResult, Parser};

use crate::vars::VariableSet;

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

fn evaluate_tokens(input: &str, vars: &VariableSet) -> Result<JsonValue> {
    // Begin Parsing
    let mut elements = match parse_all_elements(input) {
        Ok(val) => val,
        Err(error) => return Err(anyhow!("{:?}", error)),
    };

    // Check for lone token or literal
    let output = match elements.len() {
        0 => JsonValue::Null,
        1 => match elements.pop().unwrap() {
            ParsedElement::Token(key) => vars.get(key)?.clone(),
            ParsedElement::Literal(value) => JsonValue::String(value.to_string()),
        },
        _ => {
            // Build output string stack
            let mut string_stack = Vec::new();
            for element in elements.into_iter() {
                match element {
                    ParsedElement::Literal(val) => string_stack.push(val.to_string()),
                    ParsedElement::Token(key) => {
                        let value = match vars.get(key)? {
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
    fn evaluate_tokens(&self, vars: &VariableSet) -> Result<JsonValue>;
    fn evaluate_tokens_to_string(&self, token_type: &str, vars: &VariableSet) -> Result<String> {
        let output = match self.evaluate_tokens(vars)? {
            JsonValue::String(val) => Ok(val),
            other => Err(anyhow!(
                "A {} must evaluate to a String. Got '{}'",
                token_type,
                other
            )),
        }?;
        Ok(output)
    }
}

impl TokenedJsonValue for String {
    fn evaluate_tokens(&self, vars: &VariableSet) -> Result<JsonValue> {
        return evaluate_tokens(self, vars);
    }
}
impl TokenedJsonValue for &str {
    fn evaluate_tokens(&self, vars: &VariableSet) -> Result<JsonValue> {
        return evaluate_tokens(self, vars);
    }
}
impl TokenedJsonValue for JsonValue {
    fn evaluate_tokens(&self, vars: &VariableSet) -> Result<JsonValue> {
        let output = match self {
            JsonValue::Object(valmap) => {
                let mut output = serde_json::Map::new();
                for (key, val) in valmap.iter() {
                    let detokened_key = match evaluate_tokens(key, vars)? {
                        JsonValue::String(val) => val,
                        other => bail!(
                            "Map keys should always map to strings: '{}' became '{}'",
                            key,
                            other
                        ),
                    };
                    let detokened_value = val.evaluate_tokens(vars)?;
                    output.insert(detokened_key, detokened_value);
                }
                JsonValue::Object(output)
            }
            JsonValue::Array(valarr) => {
                let mut output = Vec::new();
                for val in valarr.iter() {
                    let detokened_value = val.evaluate_tokens(vars)?;
                    output.push(detokened_value);
                }
                JsonValue::Array(output)
            }
            JsonValue::String(valstr) => valstr.evaluate_tokens(vars)?,
            other => other.clone(),
        };
        Ok(output)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rstest::*;
    use serde_json::json;

    use crate::test_utils::*;

    #[test]
    fn test_multiline() -> Result<()> {
        let vars = variable_set_bob();

        let raw = json!(
            "import math
import json
print(json.dumps({ \"{{NAME}}\": math.sqrt( {{AGE}} )}))"
        );

        let output = raw.evaluate_tokens(&vars)?;

        let expected = json!(
            "import math
import json
print(json.dumps({ \"bob\": math.sqrt( 43.7 )}))"
        );
        assert_eq!(output, expected);

        Ok(())
    }

    #[rstest]
    // Happy path :)
    #[case("just some regular string", "just some regular string")]
    #[case("{NAME}", "{NAME}")]
    #[case("{{NAME}}", "bob")]
    #[case("{{NAME}}  ", "bob  ")]
    #[case("  {{NAME}}  ", "  bob  ")]
    #[case("  {{NAME}}", "  bob")]
    #[case("{{  NAME}}", "bob")]
    #[case("{{  NAME  }}", "bob")]
    #[case("{{NAME  }}", "bob")]
    #[case("{{{NAME}}}", "{bob}")]
    #[case("{{{{NAME}}}}", "{{bob}}")]
    #[case("/*{{NAME}}*/", "{{NAME}}")]
    #[case(
        "{{NAME}}'s number are {{FAVORITE_NUMBERS}}",
        "bob's number are [7,13,99]"
    )]
    // Sad path :(
    #[should_panic(expected = "A string must evaluate to a String. Got '[7,13,99]'")]
    #[case("{{FAVORITE_NUMBERS}}", "")]
    #[trace] //This attribute enable tracing
    fn string_tokens(#[case] token: &str, #[case] expected: &str) {
        let vars = variable_set_bob();
        let parsed = token.evaluate_tokens_to_string("string", &vars).unwrap();
        assert_eq!(parsed, expected);
    }

    #[test]
    fn object_token() -> Result<()> {
        let vars = variable_set_bob();
        let output = "{{CHILDREN_AGES}}".evaluate_tokens(&vars)?;

        let expected = vars.get("CHILDREN_AGES")?;
        assert_eq!(&output, expected);

        Ok(())
    }
}
