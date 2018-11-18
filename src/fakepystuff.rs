use std::collections::HashMap;

use super::lang;
use super::env;
use std::rc::Rc;

#[derive(Clone)]
pub struct PyFunc {
    pub prelude: String,
    pub eval: String,
    pub return_type: lang::Type,
    pub name: String,
    pub id: lang::ID,
}

impl lang::Function for PyFunc {
    fn call(&self, _env: &mut env::ExecutionEnvironment, _args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        lang::Value::Null
    }

    fn name(&self) -> &str {
        "Not implemented"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&lang::NULL_TYPESPEC)
    }
}
