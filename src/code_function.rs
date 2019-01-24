use super::lang;
use super::external_func;
use super::function;
use super::env;

use std::collections::HashMap;

use serde_derive::{Serialize,Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct CodeFunction {
    id: lang::ID,
    pub name: String,
    args: Vec<lang::ArgumentDefinition>,
    return_type: lang::Type,
    pub block: lang::Block,
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

    pub fn code(&self) -> lang::CodeNode {
        lang::CodeNode::Block(self.block.clone())
    }

    pub fn set_code(&mut self, block: lang::Block) {
        self.block = block
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
    fn call(&self, mut interpreter: env::Interpreter, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        for (id, value) in args {
            interpreter.env.borrow_mut().set_local_variable(id, value);
        }
        lang::Value::new_future(interpreter.evaluate(&self.code()))
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