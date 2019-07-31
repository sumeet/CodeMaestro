use super::window_positions::WindowPositions;
use crate::code_editor::CodeLocation;
use cfg_if::cfg_if;
use serde::{Deserialize, Serialize};

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
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
    open_code_editors: &'a [CodeLocation],
}

#[derive(Serialize, Deserialize, Default)]
pub struct StateDeserialize {
    pub window_positions: WindowPositions,
    pub open_code_editors: Vec<CodeLocation>,
}

pub fn save(window_positions: &WindowPositions, open_code_editors: &[CodeLocation]) {
    save_state(&StateSerialize { window_positions,
                                 open_code_editors })
}

#[cfg(target_arch = "wasm32")]
mod js {
    use super::{StateDeserialize, StateSerialize};
    use lazy_static::lazy_static;
    use stdweb::web::{window, Storage};

    lazy_static! {
        static ref STORAGE: Storage = window().local_storage();
    }

    pub fn load() -> StateDeserialize {
        let deserialized_state : Result<_, Box<dyn std::error::Error>> = try {
            let stored = STORAGE.get("state").ok_or("no such key state")?;
            serde_json::from_str(&stored)?
        };
        if let Ok(ds) = deserialized_state {
            return ds
        }
       let default = StateDeserialize::default();
       STORAGE.insert("state", &serde_json::to_string(&default).unwrap()).unwrap();
       default
    }

    pub fn save_state(state_serialize: &StateSerialize) {
        STORAGE.insert("state", &serde_json::to_string(state_serialize).unwrap())
               .unwrap()
    }

}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::save_state::{StateDeserialize, StateSerialize};
    use directories::ProjectDirs;
    use lazy_static::lazy_static;
    use std::fs::{create_dir_all, File};
    use std::path::PathBuf;

    lazy_static! {
        static ref CONFIG_DIR : PathBuf = {
            ProjectDirs::from("org", "sumeet", "cs").unwrap()
                .config_dir().into()
        };
        // HAXXXXXXXXXXXXX
        static ref STATE_FILE_NAME : PathBuf = {
            CONFIG_DIR.with_file_name("state.json").into()
        };
    }

    pub fn load() -> StateDeserialize {
        let deserialized_state : Result<_, Box<dyn std::error::Error>> = try {
            let file = File::open(&*STATE_FILE_NAME)?;
            serde_json::from_reader(file)?
        };
        if let Ok(ds) = deserialized_state {
            return ds
        }
        let default = StateDeserialize::default();
        create_dir_all(CONFIG_DIR.as_path()).unwrap();
        let f = File::create(&*STATE_FILE_NAME).unwrap();
        serde_json::to_writer_pretty(f, &default).unwrap();
        default
    }

    // JANK
    pub fn save_state(state_serialize: &StateSerialize) {
        let f = File::create(&*STATE_FILE_NAME).unwrap();
        serde_json::to_writer_pretty(f, state_serialize).unwrap()
    }
}
