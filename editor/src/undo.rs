use cs::lang;

#[derive(Clone, Debug)]
pub struct UndoHistoryCell {
    pub root: lang::CodeNode,
    pub cursor_position: Option<lang::ID>,
}

#[derive(Debug)]
pub struct UndoHistory {
    undo_stack: Vec<UndoHistoryCell>,
    redo_stack: Vec<UndoHistoryCell>,
}

impl UndoHistory {
    pub fn new() -> Self {
        Self { undo_stack: vec![],
               redo_stack: vec![] }
    }

    pub fn record_previous_state(&mut self,
                                 root: &lang::CodeNode,
                                 cursor_position: Option<lang::ID>) {
        let root = root.clone();
        self.undo_stack.push(UndoHistoryCell { root,
                                               cursor_position });
        self.redo_stack.clear();
    }

    pub fn undo(&mut self,
                current_root: &lang::CodeNode,
                cursor_position: Option<lang::ID>)
                -> Option<UndoHistoryCell> {
        self.redo_stack
            .push(UndoHistoryCell { root: current_root.clone(),
                                    cursor_position });
        self.undo_stack.pop()
    }

    pub fn redo(&mut self,
                current_root: &lang::CodeNode,
                cursor_position: Option<lang::ID>)
                -> Option<UndoHistoryCell> {
        let redone_state = self.redo_stack.pop()?;
        self.undo_stack
            .push(UndoHistoryCell { root: current_root.clone(),
                                    cursor_position });
        Some(redone_state)
    }
}
