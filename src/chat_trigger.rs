use serde_derive::{Serialize,Deserialize};
use std::collections::HashMap;

use super::lang;
use super::builtins::MESSAGE_STRUCT_ID;
use super::env;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChatTrigger {
    pub id: lang::ID,
    pub prefix: String,
    pub name: String,
    pub code: lang::Block,
}

impl ChatTrigger {
    pub fn new() -> Self {
        Self {
            id: lang::new_id(),
            prefix: "".to_string(),
            name: "New chat trigger".to_string(),
            code: lang::Block::new(),
        }
    }
}

impl lang::Function for ChatTrigger {
    fn call(&self, mut interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        // XXX: shouldn't the caller do this???? duped with CodeFunction
        for (id, value) in args {
            interpreter.env.borrow_mut().set_local_variable(id, value);
        }
        lang::Value::new_future(
            interpreter.evaluate(&lang::CodeNode::Block(self.code.clone()))
        )
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition {
                id: uuid::Uuid::parse_str("159dc4f3-3f37-44da-b979-d4a41a9273cf").unwrap(),
                arg_type: lang::Type::from_spec_id(*MESSAGE_STRUCT_ID, vec![]),
                short_name: "Message".to_string()
            }
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NULL_TYPESPEC)
    }
}
