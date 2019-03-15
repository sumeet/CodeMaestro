use serde::{Deserialize, Serialize};
use cfg_if::cfg_if;
use super::window_positions::{WindowPositions};
use crate::code_editor::CodeLocation;

cfg_if! {
    if #[cfg(feature = "javascript")] {
        pub use js::*;
    } else {
        pub use native::*;
    }
}

// separated into two classes so we can save without
// allocating
#[derive(Serialize)]
pub struct StateSerialize<'a> {
    window_positions: &'a WindowPositions,
    open_code_editors: &'a[CodeLocation],
}

#[derive(Serialize, Deserialize, Default)]
pub struct StateDeserialize {
    pub window_positions: WindowPositions,
    pub open_code_editors: Vec<CodeLocation>,
}

pub fn save(window_positions: &WindowPositions, open_code_editors: &[CodeLocation]) {
    save_state(&StateSerialize { window_positions, open_code_editors })
}

#[cfg(feature = "javascript")]
mod js {
    // TODO: use local storage
}

#[cfg(feature = "default")]
mod native {
    use directories::{ProjectDirs};
    use lazy_static::lazy_static;
    use std::path::PathBuf;
    use std::fs::File;
    use crate::save_state::{StateSerialize, StateDeserialize};

    lazy_static! {
        static ref PROJECT_DIRS : PathBuf = {
            ProjectDirs::from("org", "sumeet", "cs").unwrap()
                .config_dir().into()
        };
        // HAXXXXXXXXXXXXX
        static ref STATE_FILE_NAME : PathBuf = {
            PROJECT_DIRS.with_file_name("state.json").into()
        };
    }

    pub fn load() -> StateDeserialize {
        File::open(&*STATE_FILE_NAME)
            .map(|file| serde_json::from_reader(file))
            .unwrap_or_else(|_| {
                let default = StateDeserialize::default();
                let f = File::create(&*STATE_FILE_NAME).unwrap();
                serde_json::to_writer_pretty(f, &default).unwrap();
                Ok(default)
            }).unwrap()
    }

    // JANK
    pub fn save_state(state_serialize: &StateSerialize) {
        let f = File::create(&*STATE_FILE_NAME).unwrap();
        serde_json::to_writer_pretty(f, state_serialize).unwrap()
    }
}
