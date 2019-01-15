use super::code_editor;
use super::editor;

struct CodeEditorRenderer<'a, T> {
    ui_toolkit: &'a mut T,
}

impl<'a, T: editor::UiToolkit> CodeEditorRenderer<'a, T> {
    pub fn new(ui_toolkit: &'a mut T, code_editor: )
}