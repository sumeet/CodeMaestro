use crate::ui_toolkit::{UiToolkit, DrawFnRef};
use cs::lang::CodeNode;

pub fn render_context_menu<T: UiToolkit>(code_node: &CodeNode, ui_toolkit: &T, draw_code_fn: DrawFnRef<T>) -> T::DrawResult {
    match code_node {
        CodeNode::FunctionReference(_) => {
            ui_toolkit.context_menu(draw_code_fn, &|| {
                ui_toolkit.draw_menu_item("Delete", move || ())
            })
        }
        CodeNode::FunctionCall(_) => draw_code_fn(),
        CodeNode::Argument(_) => draw_code_fn(),
        CodeNode::StringLiteral(_) => draw_code_fn(),
        CodeNode::NullLiteral(_) => draw_code_fn(),
        CodeNode::Assignment(_) => draw_code_fn(),
        CodeNode::Block(_) => draw_code_fn(),
        CodeNode::VariableReference(_) => draw_code_fn(),
        CodeNode::Placeholder(_) => draw_code_fn(),
        CodeNode::StructLiteral(_) => draw_code_fn(),
        CodeNode::StructLiteralField(_) => draw_code_fn(),
        CodeNode::Conditional(_) => draw_code_fn(),
        CodeNode::Match(_) => draw_code_fn(),
        CodeNode::ListLiteral(_) => draw_code_fn(),
        CodeNode::StructFieldGet(_) => draw_code_fn(),
        CodeNode::NumberLiteral(_) => draw_code_fn(),
        CodeNode::ListIndex(_) => draw_code_fn(),
    }
}

