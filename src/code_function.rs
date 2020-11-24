use super::env;
use super::external_func;
use super::function;
use super::lang;

use std::collections::HashMap;

use crate::lang::FunctionRenderingStyle;
use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct CodeFunction {
    id: lang::ID,
    pub name: String,
    pub description: String,
    args: Vec<lang::ArgumentDefinition>,
    return_type: lang::Type,
    pub block: lang::Block,
    pub rendering_style: FunctionRenderingStyle,
}

impl CodeFunction {
    pub fn new() -> Self {
        Self { id: lang::new_id(),
               name: "New function".to_string(),
               description: "".into(),
               block: lang::Block::new(),
               args: vec![],
               return_type: lang::Type::from_spec(&*lang::NULL_TYPESPEC),
               rendering_style: FunctionRenderingStyle::Default }
    }

    pub fn code_id(&self) -> lang::ID {
        self.block.id
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

#[typetag::serde]
impl lang::Function for CodeFunction {
    fn call(&self,
            mut interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        // XXX: shouldn't the caller do this???? duped with ChatProgram
        for (id, value) in args {
            interpreter.set_local_variable(id, value);
        }
        lang::Value::new_future(interpreter.evaluate(&self.code()))
    }

    fn style(&self) -> &FunctionRenderingStyle {
        &self.rendering_style
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
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

    fn cs_code(&self) -> Box<dyn Iterator<Item = &lang::Block> + '_> {
        let x: Box<dyn Iterator<Item = &lang::Block>> = Box::new(std::iter::once(&self.block));
        x
    }
}
