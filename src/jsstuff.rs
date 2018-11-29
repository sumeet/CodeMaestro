use super::env;
use super::lang;
use super::external_func;

use std::collections::HashMap;

#[derive(Clone)]
pub struct JSFunc {
    pub eval: String,
    pub return_type: lang::Type,
    pub name: String,
    pub id: lang::ID,
}

impl JSFunc {
    pub fn new() -> Self {
        Self {
            eval: "".to_string(),
            return_type: lang::Type::from_spec(&lang::NULL_TYPESPEC),
            name: "New JSFunc".to_string(),
            id: lang::new_id(),
        }
    }
}

impl lang::Function for JSFunc {
    fn call(&self, _env: &mut env::ExecutionEnvironment, _args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        lang::Value::Null
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![]
    }

    fn returns(&self) -> lang::Type {
        self.return_type.clone()
    }
}

impl external_func::ModifyableFunc for JSFunc {
    fn set_return_type(&mut self, return_type: lang::Type) {
        self.return_type = return_type
    }

    fn clone(&self) -> Self {
        JSFunc {
            eval: self.eval.clone(),
            return_type: self.return_type.clone(),
            name: self.name.clone(),
            id: self.id.clone(),
        }
    }
}
