use super::lang;

#[derive(Clone, Debug)]
pub struct UndoHistoryCell {
    pub root: lang::CodeNode,
    pub cursor_position: Option<lang::ID>,
}

#[derive(Debug)]
pub struct UndoHistory {
    history: Vec<UndoHistoryCell>,
    previous_state_index: Option<usize>,
    undo_stack: Vec<UndoHistoryCell>,
    redo_stack: Vec<UndoHistoryCell>,
}

impl UndoHistory {
    pub fn new() -> Self {
        Self {
            history: vec![],
            previous_state_index: None,
            undo_stack: vec![],
            redo_stack: vec![],
        }
    }

    pub fn record_previous_state(&mut self, root: &lang::CodeNode, cursor_position: Option<lang::ID>) {
        let root = root.clone();
        // keep all elements up to the current index
        if let Some(previous_state_index) = self.previous_state_index {
            println!("truncating history to len {}", previous_state_index + 1);
            self.history.truncate(previous_state_index + 1);
        }
        // and then push the new previous state onto the stack
        self.history.push(UndoHistoryCell { root, cursor_position });
        self.previous_state_index = Some(self.history.len() - 1);
        println!("previous_state_index is now {:?}", self.previous_state_index);
        self.print_state();
    }

    pub fn undo(&mut self, current_root: &lang::CodeNode,
                cursor_position: Option<lang::ID>) -> Option<UndoHistoryCell> {
        println!("undoing...");
        let previous_state_index = self.previous_state_index?;
        println!("undo will go, previous_state_index is at {:?}", previous_state_index);
        let previous_state = self.history.get(previous_state_index).cloned();

        // if the next state in the undo buffer isn't the current state, then we need to save the current
        // state. so that we can redo and get back to the current state
        println!("seeing if we should save the current state, so we can redo and get back to it");
        if let Some(next_previous_state) = self.history.get(previous_state_index + 1) {
            println!("well there's a next previous state at index {}", previous_state_index + 1);
            if *current_root != next_previous_state.root {
                println!("well the next previous state isn't the same as what we currently have, so we're gonna write to it");
                self.record_previous_state(current_root, cursor_position);
                self.decr_previous_state_index();
            }
        } else {
            println!("well the there's no next previous state, so we're gonna save our state");
            self.record_previous_state(current_root, cursor_position);
            self.decr_previous_state_index();
        }

        self.decr_previous_state_index();
        println!("aaaand finishing the undo. now we're at state {:?}", self.previous_state_index);
        self.print_state();
        previous_state
    }

    // TODO: i think we may need to accept the current root as an argument here...  because we don't
    // want to redo if someone actually changed something after undoing. play around with it first
    // first before trying that though, because it's possible the logic in record_previous_state
    // would already take care of that
    pub fn redo(&mut self) -> Option<UndoHistoryCell> {
        println!("redoing...");
        if self.history.is_empty() {
            println!("history is empty, can't redo anything");
            return None
        }

        let next_previous_state_index = match self.previous_state_index {
            Some(previous_state_index) => previous_state_index + 2,
            None => 1,
        };

        println!("next previous state is {}, gonna pop it off and incr by 1", next_previous_state_index);

        match self.history.get(next_previous_state_index) {
            Some(_) => {
                println!("popping off {:?}", next_previous_state_index);
                let hoohaw = self.history.remove(next_previous_state_index);
                self.incr_previous_state_index();
                println!("so now we're at {:?}", self.previous_state_index);
                self.print_state();
                Some(hoohaw)
            }
            None => {
                println!("that state doesn't exist, not gonna do anything");
                println!("so we're still at {:?}", self.previous_state_index);
                self.print_state();
                None
            },
        }
    }

    fn decr_previous_state_index(&mut self) {
        self.previous_state_index = Self::previous_state_index(self.previous_state_index);
    }

    fn previous_state_index(index: Option<usize>) -> Option<usize> {
        let index = index?;
        if index == 0 {
            None
        } else {
            Some(index - 1)
        }
    }

    fn incr_previous_state_index(&mut self) {
        self.previous_state_index = Self::next_state_index(self.previous_state_index);
    }

    fn next_state_index(index: Option<usize>) -> Option<usize> {
        match index {
            Some(i) => Some(i + 1),
            None => Some(0)
        }
    }

    fn print_state(&self) {
        use itertools::Itertools;
        println!("{:?} {:?}", (0..self.history.len()).collect_vec(), self.previous_state_index);
    }
}