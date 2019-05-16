use crate::lang::CodeNode;

pub struct ClickBehavior {}

//pub fn handle_click(code_node: &CodeNode) -> ClickBehavior {
//    match code_node {
//        CodeNode::FunctionCall(function_call) => {
//            self.render_function_call(&function_call)
//        },
//        CodeNode::StringLiteral(string_literal) => {
//            self.render_string_literal(&string_literal)
//        }
//        CodeNode::NumberLiteral(number_literal) => {
//            self.render_number_literal(&number_literal)
//        }
//        CodeNode::Assignment(assignment) => self.render_assignment(&assignment),
//        CodeNode::Block(block) => self.render_block(&block),
//        CodeNode::VariableReference(variable_reference) => {
//            self.render_variable_reference(&variable_reference)
//        }
//        CodeNode::FunctionDefinition(_function_definition) => {
//            self.draw_button(&"Function defs are unimplemented", RED_COLOR, || {})
//        }
//        CodeNode::FunctionReference(function_reference) => {
//            self.render_function_reference(&function_reference)
//        }
//        CodeNode::Argument(argument) => self.render_function_call_argument(&argument),
//        CodeNode::Placeholder(placeholder) => self.render_placeholder(&placeholder),
//        CodeNode::NullLiteral => {
//            self.draw_text(&format!(" {} ", lang::NULL_TYPESPEC.symbol))
//        }
//        CodeNode::StructLiteral(struct_literal) => {
//            self.render_struct_literal(&struct_literal)
//        }
//        CodeNode::StructLiteralField(_field) => {
//            panic!("struct literal fields shouldn't be rendered from here");
//            //self.ui_toolkit.draw_all(vec![])
//            // we would, except render_struct_literal_field isn't called from here...
//            //self.render_struct_literal_field(&field)
//        }
//        CodeNode::Conditional(conditional) => self.render_conditional(&conditional),
//        CodeNode::Match(mach) => self.render_match(&mach),
//        CodeNode::ListLiteral(list_literal) => {
//            self.render_list_literal(&list_literal, code_node)
//        }
//        CodeNode::StructFieldGet(sfg) => self.render_struct_field_get(&sfg),
//        CodeNode::ListIndex(list_index) => self.render_list_index(&list_index),
//
//    }
//}
