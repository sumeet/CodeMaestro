use cs::lang;

use druid_shell::clipboard::platform::Clipboard;
use druid_shell::ClipboardFormat;
use gtk;
use lazy_static::lazy_static;
use serde_derive::{Deserialize, Serialize};

use std::sync::Mutex;

lazy_static! {
    static ref DRUID_CLIPBOARD: Mutex<Clipboard> = {
        gtk::init().unwrap();
        Mutex::new(Clipboard {})
    };
}

#[derive(Serialize, Deserialize)]
pub struct ClipboardContents {
    source_root: lang::CodeNode,
    copied_ids: Vec<lang::ID>,
}

impl ClipboardContents {
    pub fn new(source_root: lang::CodeNode, copied_ids: Vec<lang::ID>) -> Self {
        Self { source_root,
               copied_ids }
    }
}

const OUR_CLIPBOARD_FORMAT: &str = "application/cs-lang";

pub fn add_code_to_clipboard(codes: &ClipboardContents) {
    let mut clipboard = DRUID_CLIPBOARD.lock().unwrap();
    let contents = serde_json::to_vec(&codes).unwrap();
    clipboard.put_formats(&[ClipboardFormat::new(OUR_CLIPBOARD_FORMAT, contents)])
}

pub fn get_code_from_clipboard() -> Option<ClipboardContents> {
    let clipboard = DRUID_CLIPBOARD.lock().unwrap();
    let clipboard_data = clipboard.get_format(OUR_CLIPBOARD_FORMAT)?;
    serde_json::from_slice(&clipboard_data).ok()
}
