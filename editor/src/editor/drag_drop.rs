use cs::lang;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct DragDropPayload {
    code_editor_id: lang::ID,
    code_nodes: Vec<lang::CodeNode>,
}

impl DragDropPayload {
    pub fn new(code_editor_id: lang::ID, code_nodes: Vec<lang::CodeNode>) -> Self {
        Self { code_editor_id,
               code_nodes }
    }
}
