use cs::lang;
use lazy_static::lazy_static;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid;

lazy_static! {
    pub static ref QUICK_START_GUIDE_WINDOW_ID: lang::ID =
        uuid::Uuid::parse_str("8d12c907-3129-40b2-af67-a6f727a1e888").unwrap();
    pub static ref CHAT_TEST_WINDOW_ID: lang::ID =
        uuid::Uuid::parse_str("1f5dbdf2-c8b7-4594-bc3e-9a4ca4c6184b").unwrap();
}

// go under the title bar
// TODO: something else should handle going under the title bar... this should just start
// at 0, 0 pry
const INITIAL_WINDOW_POSITION: (isize, isize) = (5, 30);
const INITIAL_WINDOW_SIZE: (usize, usize) = (550, 650);

const QUICK_START_WINDOW_SIZE: (usize, usize) = (300, 200);
const CHAT_TEST_WINDOW_SIZE: (usize, usize) = (300, 500);

#[derive(Deserialize, Serialize)]
pub struct WindowPositions {
    size: (usize, usize),
    open_windows: HashMap<lang::ID, Window>,
}

impl Default for WindowPositions {
    fn default() -> Self {
        let mut open_windows = HashMap::new();
        open_windows.insert(*QUICK_START_GUIDE_WINDOW_ID,
                            Window { id: *QUICK_START_GUIDE_WINDOW_ID,
                                     size: QUICK_START_WINDOW_SIZE,
                                     x: INITIAL_WINDOW_POSITION.0,
                                     y: INITIAL_WINDOW_POSITION.1 });
        open_windows.insert(*CHAT_TEST_WINDOW_ID,
                            Window { id: *CHAT_TEST_WINDOW_ID,
                                     size: CHAT_TEST_WINDOW_SIZE,
                                     x: INITIAL_WINDOW_POSITION.0,
                                     y: INITIAL_WINDOW_POSITION.1
                                        + QUICK_START_WINDOW_SIZE.1 as isize
                                        + 5 });
        Self { size: (4000, 3000),
               open_windows }
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
                                          x: initial_pos.0,
                                          y: initial_pos.1,
                                          size: INITIAL_WINDOW_SIZE });
    }

    pub fn set_window(&mut self, window_id: lang::ID, pos: (isize, isize), size: (usize, usize)) {
        let mut win = self.open_windows.get_mut(&window_id).unwrap();
        win.size = size;
        win.x = pos.0;
        win.y = pos.1;
    }

    // TODO: i don't think this is used except for getting the first window pos
    fn get_next_window_position(&self) -> (isize, isize) {
        // TODO: some calculationz
        // for now, just open up anything to the right of the quick start window
        (INITIAL_WINDOW_POSITION.0 + QUICK_START_WINDOW_SIZE.0 as isize + 5,
         INITIAL_WINDOW_POSITION.1)
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
