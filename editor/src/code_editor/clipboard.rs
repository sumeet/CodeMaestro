use cs::lang;

use druid_shell::clipboard::platform::Clipboard;
use druid_shell::ClipboardFormat;
use gtk;
use lazy_static::lazy_static;
use serde_derive::{Deserialize, Serialize};

use crate::code_editor::find_assignment_ids_referenced_in_codes;
use crate::code_editor::locals::Variable;
use std::collections::HashMap;
use std::sync::Mutex;

lazy_static! {
    static ref DRUID_CLIPBOARD: Mutex<Clipboard> = {
        gtk::init().unwrap();
        Mutex::new(Clipboard {})
    };
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClipboardContents {
    referenced_variable_by_locals_id: HashMap<lang::ID, Variable>,
    pub copied_code: Vec<lang::CodeNode>,
}

impl ClipboardContents {
    pub fn new(copied_code: Vec<lang::CodeNode>,
               referenced_variables: impl Iterator<Item = Variable>)
               -> Self {
        Self { copied_code,
               referenced_variable_by_locals_id: referenced_variables.map(|var| {
                                                                         (var.locals_id, var)
                                                                     })
                                                                     .collect() }
    }

    pub fn variables_referenced_in_code(&self) -> impl Iterator<Item = &Variable> + '_ {
        find_assignment_ids_referenced_in_codes(self.copied_code.iter()).filter_map(move |assignment_id| {
            self.referenced_variable_by_locals_id.get(&assignment_id)
        })
    }
}

const OUR_CLIPBOARD_FORMAT: &str = "application/cs-lang";

pub fn add_code_to_clipboard(clipboard_contents: &ClipboardContents) {
    let mut clipboard = DRUID_CLIPBOARD.lock().unwrap();
    let contents = serde_json::to_vec(&clipboard_contents).unwrap();
    clipboard.put_formats(&[ClipboardFormat::new(OUR_CLIPBOARD_FORMAT, contents)])
}

pub fn get_code_from_clipboard() -> Option<ClipboardContents> {
    let clipboard = DRUID_CLIPBOARD.lock().unwrap();
    let clipboard_data = clipboard.get_format(OUR_CLIPBOARD_FORMAT)?;
    serde_json::from_slice(&clipboard_data).ok()
}
