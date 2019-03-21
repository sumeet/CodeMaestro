use super::lang;
use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Test {
    pub id: lang::ID,
    pub name: String,
    pub subject: TestSubject,
    code: lang::Block,
}

impl Test {
    pub fn new(subject: TestSubject) -> Self {
        Self { id: lang::new_id(),
               name: "New test".to_string(),
               subject,
               code: lang::Block::new() }
    }

    pub fn code_id(&self) -> lang::ID {
        self.code.id
    }

    pub fn code(&self) -> lang::CodeNode {
        lang::CodeNode::Block(self.code.clone())
    }

    pub fn set_code(&mut self, code: lang::Block) {
        self.code = code
    }
}

#[derive(PartialEq, Clone, Copy, Hash, Eq, Serialize, Deserialize, Debug)]
pub enum TestSubject {
    Function(lang::ID),
}
