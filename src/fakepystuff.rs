use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

use super::env;
use super::external_func;
use super::function;
use super::lang;

#[derive(Clone, Serialize, Deserialize)]
pub struct PyFunc {
    pub prelude: String,
    pub eval: String,
    pub return_type: lang::Type,
    pub name: String,
    pub description: String,
    pub id: lang::ID,
}

#[typetag::serde]
impl lang::Function for PyFunc {
    fn call(&self,
            _interpreter: env::Interpreter,
            _args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        lang::Value::Null
    }

    fn name(&self) -> &str {
        "Not implemented"
    }

    fn description(&self) -> &str {
        "Not implemented"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NULL_TYPESPEC)
    }
}

impl external_func::ModifyableFunc for PyFunc {
    fn set_return_type(&mut self, return_type: lang::Type) {
        self.return_type = return_type
    }
}

impl function::SettableArgs for PyFunc {
    fn set_args(&mut self, _args: Vec<lang::ArgumentDefinition>) {}
}
