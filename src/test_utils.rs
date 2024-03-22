use serde_json::json;
use std::collections::HashMap;

use crate::vars::VariableSet;

pub fn variable_set_bob() -> VariableSet {
    let mut vars = VariableSet::new();
    vars.insert("NAME".to_string(), json!("bob"));
    vars.insert("AGE".to_string(), json!(43.7));
    vars.insert("FAVORITE_NUMBERS".to_string(), json!(vec![7, 13, 99]));
    vars.insert("FEARS".to_string(), json!(()));

    let mut children = HashMap::new();
    children.insert("timmy".to_string(), 3);
    children.insert("sarah".to_string(), 8);
    vars.insert("CHILDREN_AGES".to_string(), json!(children));

    vars
}

#[macro_export]
macro_rules! testing_block_on {
    ( $executor:ident, $func:expr) => {{
        let $executor = DigExecutor::new(2);
        let future = $func;

        smol::block_on($executor.executor.run(future))
    }};
}
pub(crate) use testing_block_on;
