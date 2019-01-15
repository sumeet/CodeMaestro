use std::cell::RefCell;
use std::rc::Rc;

use super::lang;
use super::editor;

pub struct CodeEditor {
    code: lang::CodeNode,
    editing: bool,
    selected_node_id: Option<lang::ID>,
    insert_code_menu: Option<editor::InsertCodeMenu>
}

impl CodeEditor {
    pub fn new(code: &lang::CodeNode) -> Self {
        Self {
            code: code.clone(),
            editing: false,
            selected_node_id: None,
            insert_code_menu: None
        }
    }
}