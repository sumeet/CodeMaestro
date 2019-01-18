use super::lang;
use super::external_func;
use super::function;
use super::env;
use std::collections::HashMap;

#[derive(Clone)]
pub struct CodeFunction {
    id: lang::ID,
    name: String,
    args: Vec<lang::ArgumentDefinition>,
    return_type: lang::Type,
    block: lang::Block,
}

impl CodeFunction {
    pub fn new() -> Self {
        Self {
            id: lang::new_id(),
            name: "New function".to_string(),
            block: lang::Block::new(),
            args: vec![],
            return_type: lang::Type::from_spec(&*lang::NULL_TYPESPEC),
        }
    }
}

impl external_func::ModifyableFunc for CodeFunction {
    fn set_return_type(&mut self, return_type: lang::Type) {
        self.return_type = return_type
    }
}

impl function::SettableArgs for CodeFunction {
    fn set_args(&mut self, args: Vec<lang::ArgumentDefinition>) {
        self.args = args
    }
}

impl lang::Function for CodeFunction {
    fn call(&self, env: &mut env::ExecutionEnvironment, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        unimplemented!()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        self.args.clone()
    }

    fn returns(&self) -> lang::Type {
        self.return_type.clone()
    }
}