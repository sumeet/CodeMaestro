use super::{lang};

use std::collections::HashMap;

pub struct ExecutionEnvironment {
    pub console: String,
    pub locals: HashMap<lang::ID, lang::Value>
}

impl ExecutionEnvironment {
    pub fn new() -> ExecutionEnvironment {
        return ExecutionEnvironment {
            console: String::new(),
            locals: HashMap::new(),
        }
    }

    pub fn set_local_variable(&mut self, id: lang::ID, value: lang::Value) {
        self.locals.insert(id, value);
    }

    pub fn get_local_variable(&self, id: lang::ID) -> Option<&lang::Value> {
        if self.locals.contains_key(&id) {
            Some(&self.locals[&id])
        } else {
            None
        }

    }

    pub fn println(&mut self, ln: &str) {
        self.console.push_str(ln);
        self.console.push_str("\n")
    }

    pub fn read_console(&self) -> &str {
        &self.console
    }
}

