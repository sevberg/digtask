use serde_json::json;
use std::collections::HashMap;

use crate::vars::VariableSet;

pub fn variable_set_bob() -> VariableSet {
    let mut vars = VariableSet::new();
    vars.insert_local("NAME".to_string(), json!("bob"));
    vars.insert_local("AGE".to_string(), json!(43.7));
    vars.insert_local("FAVORITE_NUMBERS".to_string(), json!(vec![7, 13, 99]));
    vars.insert_local("FEARS".to_string(), json!(()));

    let mut children = HashMap::new();
    children.insert("timmy".to_string(), 3);
    children.insert("sarah".to_string(), 8);
    vars.insert_local("CHILDREN_AGES".to_string(), json!(children));

    vars
}
