use cs::lang;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

const INITIAL_WINDOW_SIZE: (usize, usize) = (400, 500);

#[derive(Deserialize, Serialize)]
pub struct WindowPositions {
    size: (usize, usize),
    open_windows: HashMap<lang::ID, Window>,
}

impl Default for WindowPositions {
    fn default() -> Self {
        Self { size: (4000, 3000),
               open_windows: HashMap::new() }
    }
}

impl WindowPositions {
    pub fn add_window(&mut self, window_id: lang::ID) {
        if self.open_windows.contains_key(&window_id) {
            return;
        }
        let initial_pos = self.get_next_window_position();
        self.open_windows.insert(window_id,
                                 Window { id: window_id,
                                          x: initial_pos.0 as isize,
                                          y: initial_pos.1 as isize,
                                          size: INITIAL_WINDOW_SIZE });
    }

    pub fn set_window(&mut self, window_id: lang::ID, pos: (isize, isize), size: (usize, usize)) {
        let mut win = self.open_windows.get_mut(&window_id).unwrap();
        win.size = size;
        win.x = pos.0;
        win.y = pos.1;
    }

    fn get_next_window_position(&self) -> (usize, usize) {
        (25, 50)
    }

    pub fn get_open_window(&self, id: &lang::ID) -> Option<Window> {
        self.open_windows.get(id).map(Clone::clone)
    }

    pub fn get_open_windows<'a>(&'a self,
                                ids: impl Iterator<Item = &'a lang::ID> + 'a)
                                -> impl Iterator<Item = Window> + 'a {
        ids.filter_map(move |id| self.get_open_window(id))
    }

    pub fn close_window(&mut self, id: lang::ID) {
        self.open_windows.remove(&id);
    }
}

// TODO: could mutate this window if we were to implement scrolling of the background ourselves
#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Window {
    pub id: lang::ID,
    pub x: isize,
    pub y: isize,
    pub size: (usize, usize),
}

impl Window {
    pub fn pos(&self) -> (isize, isize) {
        (self.x, self.y)
    }
}
