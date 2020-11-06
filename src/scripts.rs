use super::lang;
use serde_derive::{Deserialize, Serialize};

// doesn't take any arguments, and doesn't return anything
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Script {
    pub name: String,
    pub code: lang::Block,
}

impl Script {
    pub fn new() -> Self {
        Self { name: "New script".into(),
               code: lang::Block::new() }
    }

    pub fn code(&self) -> lang::CodeNode {
        lang::CodeNode::Block(self.code.clone())
    }

    pub fn id(&self) -> lang::ID {
        self.code.id
    }
}
