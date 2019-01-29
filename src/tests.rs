use super::lang;

pub struct Test {
    pub id: lang::ID,
    pub name: String,
    pub subject: TestSubject,
    code: lang::Block,
}

impl Test {
    pub fn new(subject: TestSubject) -> Self {
        Self {
            id: lang::new_id(),
            name: "New test".to_string(),
            subject,
            code: lang::Block::new(),
        }
    }
}

#[derive(PartialEq, Clone, Copy, Hash, Eq)]
pub enum TestSubject {
    Function(lang::ID),
}