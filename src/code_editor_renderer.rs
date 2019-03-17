use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

use itertools::Itertools;

use super::ui_toolkit::UiToolkit;
use super::code_editor;
use super::insert_code_menu::{InsertCodeMenu,InsertCodeMenuOption};
use super::code_editor::InsertionPoint;
use super::editor;
use super::lang;
use super::structs;
use super::lang::CodeNode;
use super::env_genie::EnvGenie;

// TODO: move to colors.rs
pub type Color = [f32; 4];

pub const PLACEHOLDER_ICON: &str = "\u{F071}";
pub const SELECTION_COLOR: Color = [1., 1., 1., 0.3];
pub const YELLOW_COLOR: Color = [253.0 / 255.0, 159.0 / 255.0, 19.0 / 255.0, 1.0];
pub const CLEAR_COLOR: Color = [0.0, 0.0, 0.0, 0.0];
pub const GREY_COLOR: Color = [0.521, 0.521, 0.521, 1.0];
pub const BLUE_COLOR: Color = [100.0 / 255.0, 149.0 / 255.0, 237.0 / 255.0, 1.0];
pub const BLACK_COLOR: Color = [0.0, 0.0, 0.0, 1.0];
pub const RED_COLOR: Color = [0.858, 0.180, 0.180, 1.0];
pub const PURPLE_COLOR: Color = [0.486, 0.353, 0.952, 1.0];
pub const PX_PER_INDENTATION_LEVEL : i16 = 20;

pub struct CodeEditorRenderer<'a, T> {
    ui_toolkit: &'a T,
    arg_nesting_level: Rc<RefCell<u32>>,
    code_editor: &'a code_editor::CodeEditor,
    command_buffer: Rc<RefCell<PerEditorCommandBuffer>>,
    env_genie: &'a EnvGenie<'a>,
}

// ok stupid but all the methods on this take &self instead of &mut self because the ImGui closures
// all take Fn instead of FnMut
impl<'a, T: UiToolkit> CodeEditorRenderer<'a, T> {
    pub fn new(ui_toolkit: &'a T, code_editor: &'a code_editor::CodeEditor,
               command_buffer: Rc<RefCell<editor::CommandBuffer>>, env_genie: &'a EnvGenie) -> Self {
        let command_buffer = PerEditorCommandBuffer::new(
            command_buffer, code_editor.id());
        Self {
            ui_toolkit,
            code_editor,
            command_buffer: Rc::new(RefCell::new(command_buffer)),
            arg_nesting_level: Rc::new(RefCell::new(0)),
            env_genie,
        }
    }

    pub fn render(&self, height_percentage: f32) -> T::DrawResult {
        let code = self.code_editor.get_code();
        let cmd_buffer = Rc::clone(&self.command_buffer);
        self.ui_toolkit.draw_child_region(
            &|| { self.render_code(code) },
            height_percentage,
            Some(move |keypress| {
                cmd_buffer.borrow_mut().add_editor_command(move |code_editor| {
                    code_editor.handle_keypress(keypress)
                })
            })
        )
    }

    fn is_insertion_pointer_immediately_before(&self, id: lang::ID) -> bool {
        let insertion_point = self.code_editor.insertion_point();
        match insertion_point {
            Some(InsertionPoint::Before(code_node_id)) if code_node_id == id => {
                true
            }
            _ => false
        }
    }

    fn draw_code_node_and_insertion_point_if_before_or_after(&self, code_node: &CodeNode,
                                                             draw: &Fn() -> T::DrawResult) -> T::DrawResult {
        let mut drawn: Vec<T::DrawResult> = vec![];
        if self.is_insertion_pointer_immediately_before(code_node.id()) {
            drawn.push(self.render_insert_code_node())
        }
        drawn.push(draw());
        if self.is_insertion_pointer_immediately_after(code_node.id()) {
            drawn.push(self.render_insert_code_node())
        }
        self.ui_toolkit.draw_all(drawn)
    }

    fn is_insertion_pointer_immediately_after(&self, id: lang::ID) -> bool {
        match self.code_editor.insertion_point() {
            Some(InsertionPoint::After(code_node_id)) if code_node_id == id => {
                true
            }
            _ => false
        }
    }

    fn draw_selected(&self, draw: &Fn() -> T::DrawResult) -> T::DrawResult {
        self.ui_toolkit.draw_box_around(SELECTION_COLOR, draw)
    }

    fn render_insertion_option(&self, option: &'a InsertCodeMenuOption,
                               insertion_point: InsertionPoint) -> T::DrawResult {
        let is_selected = option.is_selected;
        let button_color = if is_selected { RED_COLOR } else { BLACK_COLOR };
        let cmd_buffer = Rc::clone(&self.command_buffer);
        let new_code_node = option.new_node.clone();

        let draw = move || {
            let cmdb = cmd_buffer.clone();
            let new_code_node = new_code_node.clone();

            self.draw_small_button(&option.label, button_color, move|| {
                let ncn = new_code_node.clone();
                cmdb.borrow_mut().add_editor_command(move |editor| {
                    editor.hide_insert_code_menu();
                    editor.insert_code(ncn.clone(), insertion_point);
                });
            })
        };

        if is_selected {
            self.draw_selected(&draw)
        } else {
            draw()
        }
    }

    fn render_assignment(&self, assignment: &lang::Assignment) -> T::DrawResult {
        let type_of_assignment = self.code_editor.code_genie
            .guess_type(assignment.expression.as_ref(), self.env_genie);
        self.ui_toolkit.draw_all_on_same_line(&[
            &|| {
                self.render_name_with_type_definition(&assignment.name, PURPLE_COLOR, &type_of_assignment)
                // TODO: this still needs to be an inline editable button, or at least needs to let
                // us click to change the variable name somehow
//                self.render_inline_editable_button(
//                    &assignment.name,
//                    PURPLE_COLOR,
//                    InsertionPoint::Editing(assignment.id)
//                )
            },
            &|| self.draw_text("   \u{f52c}   "),
            &|| self.render_code(assignment.expression.as_ref()),
        ])
    }


    fn render_insert_code_node(&self) -> T::DrawResult {
        // TODO: do we really need this clone?
        let menu = self.code_editor.insert_code_menu.as_ref().unwrap().clone();

        self.ui_toolkit.draw_all(vec![
            self.ui_toolkit.focused(&||{
                let cmdb_1 = Rc::clone(&self.command_buffer);
                let cmdb_2 = Rc::clone(&self.command_buffer);
                let insertion_point = menu.insertion_point.clone();
                let new_code_node = menu.selected_option_code(&self.code_editor.code_genie, self.env_genie);

                self.draw_text_input(
                    menu.input_str(),
                    move |input|{
                        let input = input.to_string();
                        cmdb_1.borrow_mut().add_editor_command(move |editor| {
                            editor.insert_code_menu.as_mut().map(|menu| {
                                menu.set_search_str(&input);
                            });
                        })
                    },
                    move ||{
                        let mut cmdb = cmdb_2.borrow_mut();
                        if let Some(ref new_code_node) = new_code_node {
                            let new_code_node = new_code_node.clone();
                            cmdb.add_editor_command(move |editor| {
                                editor.hide_insert_code_menu();
                                editor.insert_code(new_code_node.clone(), insertion_point);
                            });
                        } else {
                            cmdb.add_editor_command(|editor| {
                                editor.undo();
                                editor.hide_insert_code_menu();
                            });
                        }
                    })
            }),
            self.render_without_nesting(&|| self.render_insertion_options(&menu)),
        ])
    }

    fn render_insertion_options(&self, menu: &InsertCodeMenu) -> T::DrawResult {
        let options = menu.list_options(&self.code_editor.code_genie, self.env_genie);
        let render_insertion_options : Vec<(Box<Fn() -> T::DrawResult>, bool)> = options.iter()
            .map(|option| {
                let c : Box<Fn() -> T::DrawResult> = Box::new(move || {
                    self.render_insertion_option(option, menu.insertion_point)
                });
                (c, option.is_selected)
            })
            .collect();
        let render_insertion_options = render_insertion_options
            .iter()
            .map(|(box_fn, is_selected)| (box_fn.as_ref(), *is_selected));
        self.ui_toolkit.draw_x_scrollable_list(render_insertion_options, 1)
    }

    fn render_list_literal(&self, list_literal: &lang::ListLiteral,
                           code_node: &lang::CodeNode) -> T::DrawResult {
        let t = self.code_editor.code_genie.guess_type(code_node, self.env_genie);

        // TODO: we can use smth better to express the nesting than ascii art, like our nesting scheme
        //       with the black lines (can actually make that generic so we can swap it with something
        //       else
        let type_symbol = self.env_genie.get_symbol_for_type(&t);
        let lhs = &|| self.draw_button(&type_symbol, BLUE_COLOR, &|| {});

        let insert_pos = match self.code_editor.insert_code_menu {
            Some(InsertCodeMenu {
                     insertion_point: InsertionPoint::ListLiteralElement { list_literal_id, pos }, ..
                 }) if list_literal_id == list_literal.id => Some(pos),
            _ => None,
        };

        let mut rhs : Vec<Box<Fn() -> T::DrawResult>> = vec![];
        let mut position_label = 0;
        let mut i = 0;
        while i <= list_literal.elements.len() {
            if insert_pos.map_or(false, |insert_pos| insert_pos == i) {
                let position_string = position_label.to_string();
                rhs.push(Box::new(move || {
                    self.ui_toolkit.draw_all_on_same_line(&[
                        &|| {
                            self.draw_button(&position_string, BLACK_COLOR, &||{})
                        },
                        &|| self.render_nested(&|| self.render_insert_code_node()),
                    ])
                }));
                position_label += 1;
            }

            list_literal.elements.get(i).map(|el| {
                rhs.push(Box::new(move || {
                    self.ui_toolkit.draw_all_on_same_line(&[
                        &|| {
                            self.draw_button(&position_label.to_string(), BLACK_COLOR, &|| {})
                        },
                        &|| self.render_nested(&|| self.render_code(el)),
                    ])
                }));
                position_label += 1;
            });
            i += 1;
        }

        self.ui_toolkit.align(lhs,
                              &rhs.iter()
                                  .map(|c| c.as_ref())
                                  .collect_vec()
        )
    }

    fn render_nested(&self, draw_fn: &Fn() -> T::DrawResult) -> T::DrawResult {
        self.arg_nesting_level.replace_with(|l| *l + 1);
        let drawn = draw_fn();
        self.arg_nesting_level.replace_with(|l| *l - 1);
        drawn
    }

    fn render_without_nesting(&self, draw_fn: &Fn() -> T::DrawResult) -> T::DrawResult {
        let old_nesting_level = self.arg_nesting_level.replace(0);
        let drawn = draw_fn();
        self.arg_nesting_level.replace(old_nesting_level);
        drawn
    }

    fn draw_nested_borders_around(&self, draw_element_fn: &Fn() -> T::DrawResult) -> T::DrawResult {
        let nesting_level = *self.arg_nesting_level.borrow();
        if nesting_level == 0 {
            return draw_element_fn()
        }

        let top_border_thickness = 1 + nesting_level + 1;
        let right_border_thickness = 1;
        let left_border_thickness = 1;
        let bottom_border_thickness = 1;

        let drawn = self.ui_toolkit.draw_top_border_inside(BLACK_COLOR, top_border_thickness as u8, &|| {
            self.ui_toolkit.draw_right_border_inside(BLACK_COLOR, right_border_thickness, &|| {
                self.ui_toolkit.draw_left_border_inside(BLACK_COLOR, left_border_thickness, &|| {
                    self.ui_toolkit.draw_bottom_border_inside(BLACK_COLOR, bottom_border_thickness, draw_element_fn)
                })
            })
        });
        drawn
    }

    fn is_selected(&self, code_node_id: lang::ID) -> bool {
        Some(code_node_id) == *self.code_editor.get_selected_node_id()
    }

    fn is_editing(&self, code_node_id: lang::ID) -> bool {
        self.is_selected(code_node_id) && self.code_editor.editing
    }

    fn render_code(&self, code_node: &CodeNode) -> T::DrawResult {
        if self.is_editing(code_node.id()) {
            return self.draw_inline_editor(code_node)
        }
        let draw = ||{
            match code_node {
                CodeNode::FunctionCall(function_call) => {
                    self.render_function_call(&function_call)
                }
                CodeNode::StringLiteral(string_literal) => {
                    self.render_string_literal(&string_literal)
                }
                CodeNode::NumberLiteral(number_literal) => {
                    self.render_number_literal(&number_literal)
                }
                CodeNode::Assignment(assignment) => {
                    self.render_assignment(&assignment)
                }
                CodeNode::Block(block) => {
                    self.render_block(&block)
                }
                CodeNode::VariableReference(variable_reference) => {
                    self.render_variable_reference(&variable_reference)
                }
                CodeNode::FunctionDefinition(_function_definition) => {
                    self.draw_button(
                        &"Function defs are unimplemented",
                        RED_COLOR,
                        ||{}
                    )
                }
                CodeNode::FunctionReference(function_reference) => {
                    self.render_function_reference(&function_reference)
                }
                CodeNode::Argument(argument) => {
                    self.render_function_call_argument(&argument)
                }
                CodeNode::Placeholder(placeholder) => {
                    self.render_placeholder(&placeholder)
                }
                CodeNode::NullLiteral => {
                    self.draw_text(&format!(" {} ", lang::NULL_TYPESPEC.symbol))
                },
                CodeNode::StructLiteral(struct_literal) => {
                    self.render_struct_literal(&struct_literal)
                },
                CodeNode::StructLiteralField(_field) => {
                    self.ui_toolkit.draw_all(vec![])
                    // we would, except render_struct_literal_field isn't called from here...
                    //self.render_struct_literal_field(&field)
                },
                CodeNode::Conditional(conditional) => {
                    self.render_conditional(&conditional)
                }
                CodeNode::Match(mach) => self.render_match(&mach),
                CodeNode::ListLiteral(list_literal) => {
                    self.render_list_literal(&list_literal, code_node)
                }
                CodeNode::StructFieldGet(sfg) => self.render_struct_field_get(&sfg),
                CodeNode::ListIndex(list_index) => self.render_list_index(&list_index)
            }
        };

        match self.insertion_point() {
            Some(InsertionPoint::Replace(id)) | Some(InsertionPoint::Wrap(id)) if {
                id == code_node.id()
            } => return self.render_insert_code_node(),
            _ => {}
        }

        if self.is_selected(code_node.id()) {
            self.draw_selected(&draw)
        } else {
            self.draw_code_node_and_insertion_point_if_before_or_after(code_node, &draw)
        }
    }

    fn render_placeholder(&self, placeholder: &lang::Placeholder) -> T::DrawResult {
        let mut r = YELLOW_COLOR;
        // LOL: mess around w/ some transparency
        r[3] = 0.4;
        // TODO: maybe use the traffic cone instead of the exclamation triangle,
        // which is kinda hard to see
        self.draw_button(
            &format!("{} {}", PLACEHOLDER_ICON, placeholder.description),
            r,
            &|| {})
    }

    fn render_function_reference(&self, function_reference: &lang::FunctionReference) -> T::DrawResult {
        let function_id = function_reference.function_id;

        // TODO: don't do validation in here. this is just so i can see what this error looks
        // like visually. for realz, i would probably be better off having a separate validation
        // step. and THEN show the errors in here. or maybe overlay something on the codenode that
        // contains the error
        //
        // UPDATE: so i tried that, but figured i still needed to have this code here. i guess maybe
        // there's gonna be no avoiding doing double validation in some situations, and that's ok
        // i think
        let func = self.env_genie.find_function(function_id) ;
        if func.is_none() {
            let error_msg = format!("Error: function ID {} not found", function_id);
            return self.draw_button(&error_msg, RED_COLOR, &||{})
        }
        let func = func.unwrap();
        // TODO: rework this for when we have generics. the function arguments will need to take
        // the actual parameters as parameters
        self.render_function_name(&func.name(), GREY_COLOR, &func.returns())
    }

    fn render_variable_reference(&self, variable_reference: &lang::VariableReference) -> T::DrawResult {
        if let Some(name) = self.lookup_variable_name(variable_reference) {
            let typ = self.code_editor.code_genie.guess_type(
                self.code_editor.code_genie.find_node(variable_reference.id).unwrap(),
                self.env_genie);
                self.render_name_with_type_definition(&name, PURPLE_COLOR, &typ)

        } else {
            self.draw_button("Variable reference not found", RED_COLOR, &|| {})
        }
    }

    fn darken(&self, mut color: Color) -> Color {
        color[0] *= 0.75;
        color[1] *= 0.75;
        color[2] *= 0.75;
        color
    }

    fn render_function_name(&self, name: &str, color: Color, typ: &lang::Type) -> T::DrawResult {
        let sym = self.env_genie.get_symbol_for_type(typ);

        let darker_color = self.darken(self.darken(self.darken(color)));
        self.draw_nested_borders_around(&|| {
            self.ui_toolkit.draw_top_border_inside(darker_color, 2, &|| {
                self.ui_toolkit.draw_right_border_inside(darker_color, 1, &|| {
                    self.ui_toolkit.draw_left_border_inside(darker_color, 1, &|| {
                        self.ui_toolkit.draw_bottom_border_inside(darker_color, 1, &|| {
                            // don't show the return type if the function returns null. there's no
                            // use in looking at it
                            if typ.matches_spec(&lang::NULL_TYPESPEC) {
                                self.ui_toolkit.draw_button(name, color, &|| {})
                            } else {
                                self.ui_toolkit.draw_all_on_same_line(&[
                                    &|| self.ui_toolkit.draw_button(name, color, &|| {}),
                                    &|| self.ui_toolkit.draw_text("  "),
                                    &|| self.ui_toolkit.draw_button(&sym, darker_color, &||{}),
                                ])
                            }
                        })
                    })
                })
            })
        })
    }

    // this is used for rendering variable references and struct field gets. it displays the type of the
    // attribute next to the name of it, so the user can see type information along
    fn render_name_with_type_definition(&self, name: &str, color: Color, typ: &lang::Type) -> T::DrawResult {
            let sym = self.env_genie.get_symbol_for_type(typ);

            let darker_color = self.darken(color);
            self.draw_nested_borders_around(&|| {
                self.ui_toolkit.draw_top_border_inside(darker_color, 2, &|| {
                    self.ui_toolkit.draw_right_border_inside(darker_color, 1, &|| {
                        self.ui_toolkit.draw_left_border_inside(darker_color, 1, &|| {
                            self.ui_toolkit.draw_bottom_border_inside(darker_color, 1, &|| {
                                self.ui_toolkit.buttonize(&|| {
                                    self.ui_toolkit.draw_all_on_same_line(&[
                                        &|| self.ui_toolkit.draw_buttony_text(&sym, darker_color),
                                        &|| self.ui_toolkit.draw_buttony_text(name, color),
                                    ])
                                }, || {})
                            })
                        })
                    })
                })
            })
    }

    fn lookup_variable_name(&self, variable_reference: &lang::VariableReference) -> Option<String> {
        let assignment = self.code_editor.code_genie.find_node(variable_reference.assignment_id);
        if let Some(CodeNode::Assignment(assignment)) = assignment {
            return Some(assignment.name.clone())
        }
        // TODO: this searches all functions, but we could be smarter here because we already know which
        //       function we're inside
        if let Some(arg) = self.env_genie.get_arg_definition(variable_reference.assignment_id) {
            return Some(arg.short_name)
        }
        // variables can also refer to enum variants
        self.code_editor.code_genie.find_enum_variant_preceding_by_assignment_id(variable_reference.id,
                                                                 variable_reference.assignment_id, self.env_genie)
            .map(|match_variant| match_variant.enum_variant.name)
    }

    fn render_block(&self, block: &lang::Block) -> T::DrawResult {
        // TODO: i think i could move the is_insertion_point_before_or_after crapola to here
        let mut to_draw = match self.code_editor.insertion_point() {
            Some(InsertionPoint::BeginningOfBlock(block_id)) if block.id == block_id => vec![self.render_insert_code_node()],
            _ => vec![],
        };
        to_draw.extend(block.expressions.iter().map(|code| self.render_code(code)));
        self.ui_toolkit.draw_all(to_draw)
    }

    fn render_function_call(&self, function_call: &lang::FunctionCall) -> T::DrawResult {
        // XXX: we've gotta have this conditional because of a quirk with the way the imgui
        // toolkit works. if render_function_call_arguments doesn't actually draw anything, it
        // will cause the next drawn thing to appear on the same line. weird i know, maybe we can
        // one day fix this jumbledness
        if function_call.args.is_empty() {
            return self.render_code(&function_call.function_reference)
        }

        let rhs = self.render_function_call_arguments(
            function_call.function_reference().function_id,
            function_call.args());
        let rhs : Vec<Box<Fn() -> T::DrawResult>> = rhs
            .iter()
            .map(|cl| {
                let b : Box<Fn() -> T::DrawResult> = Box::new(move || cl(&self));
                b
            })
            .collect_vec();

        self.ui_toolkit.align(
            &|| { self.render_code(&function_call.function_reference) },
            &rhs.iter().map(|b| b.as_ref()).collect_vec()
        )
    }

    fn render_function_call_argument(&self, argument: &lang::Argument) -> T::DrawResult {
        let arg_display = {
            match self.env_genie.get_arg_definition(argument.argument_definition_id) {
                Some(arg_def) => {
                    let type_symbol = self.env_genie.get_symbol_for_type(&arg_def.arg_type);
                    format!("{} {}", type_symbol, arg_def.short_name)
                },
                None => "\u{f059}".to_string(),
            }
        };


        self.render_nested(&|| {
            self.ui_toolkit.draw_all_on_same_line(&[
                &|| {
                    self.render_inline_editable_button(&arg_display, BLACK_COLOR,
                                                       InsertionPoint::Replace(argument.expr.id()))
                },
                &|| {
                    self.render_code(argument.expr.as_ref())
                },
            ])
        })
    }

    fn render_args_for_found_function(&self, function: &lang::Function,
                                      args: Vec<&lang::Argument>) -> Vec<Box<Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> {
        let provided_arg_by_definition_id : HashMap<lang::ID,lang::Argument> = args.into_iter()
            .map(|arg| (arg.argument_definition_id, arg.clone())).collect();
        let expected_args = function.takes_args();

        let mut draw_fns : Vec<Box<Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> = vec![];

        for expected_arg in expected_args.into_iter() {
            if let Some(provided_arg) = provided_arg_by_definition_id.get(&expected_arg.id).clone() {
                let provided_arg = provided_arg.clone();
                draw_fns.push(Box::new(move |s: &CodeEditorRenderer<T>| s.render_code(&CodeNode::Argument(provided_arg.clone()))))
            } else {
                draw_fns.push(Box::new(move |s: &CodeEditorRenderer<T>| s.render_missing_function_argument(&expected_arg)))
            }
        }
        draw_fns
    }

    fn render_missing_function_argument(&self, _arg: &lang::ArgumentDefinition) -> T::DrawResult {
        self.draw_button(
            "this shouldn't have happened, you've got a missing function arg somehow",
            RED_COLOR,
            &|| {})
    }

    fn render_function_call_arguments(&self, function_id: lang::ID, args: Vec<&lang::Argument>) -> Vec<Box<Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> {
        let function = self.env_genie.find_function(function_id)
            .map(|func| func.clone());
        let args = args.clone();
        match function {
            Some(function) => {
                return self.render_args_for_found_function(&*function, args)
            },
            None => {
                return self.render_args_for_missing_function(args)
            }
        }
    }

    fn render_args_for_missing_function(&self, _args: Vec<&lang::Argument>) -> Vec<Box<Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> {
        vec![Box::new(|s: &CodeEditorRenderer<T>| s.ui_toolkit.draw_all(vec![]))]
    }

    fn render_struct_literal_field(&self, field: &structs::StructField,
                                   literal_field: &lang::StructLiteralField) -> T::DrawResult {
        let field_text = format!("{} {}", self.env_genie.get_symbol_for_type(&field.field_type),
                                 field.name);
        self.ui_toolkit.draw_all_on_same_line(&[
            &|| {
                if self.is_editing(literal_field.id) {
                    self.render_insert_code_node()
                } else {
                    self.render_inline_editable_button(&field_text, BLACK_COLOR,
                                                       InsertionPoint::StructLiteralField(literal_field.id))
                }
            },
            &|| self.render_nested(&|| self.render_code(&literal_field.expr))
        ])
    }

    fn render_struct_literal_fields(&self, strukt: &'a structs::Struct,
                                    fields: impl Iterator<Item = &'a lang::StructLiteralField>) -> Vec<Box<Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> {
        // TODO: should this map just go inside the struct????
        let struct_field_by_id = strukt.field_by_id();

        let mut to_draw : Vec<Box<Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> = vec![];
        for literal_field in fields {
            // this is where the bug is
            let strukt_field = struct_field_by_id.get(&literal_field.struct_field_id).unwrap();
            let strukt_field = (*strukt_field).clone();
            let literal_feeld = literal_field.clone();
            to_draw.push(Box::new(move |s: &CodeEditorRenderer<T>| {
                s.render_struct_literal_field(&strukt_field, &literal_feeld)
            }));
        }
        to_draw
    }

    fn render_struct_literal(&self, struct_literal: &lang::StructLiteral) -> T::DrawResult {
        // XXX: we've gotta have this conditional because of a quirk with the way the imgui
        // toolkit works. if render_function_call_arguments doesn't actually draw anything, it
        // will cause the next drawn thing to appear on the same line. weird i know, maybe we can
        // one day fix this jumbledness
        let strukt = self.env_genie.find_struct(struct_literal.struct_id).unwrap();

        if struct_literal.fields.is_empty() {
            return self.render_struct_identifier(&strukt, struct_literal)
        }
        let rhs = self.render_struct_literal_fields(&strukt,
                                                    struct_literal.fields());
        let rhs : Vec<Box<Fn() -> T::DrawResult>> = rhs.into_iter()
            .map(|draw_fn| {
                let b : Box<Fn() -> T::DrawResult> = Box::new(move || draw_fn(&self));
                b
            }).collect_vec();
        self.ui_toolkit.align(
            &|| { self.render_struct_identifier(&strukt, struct_literal) },
            &rhs.iter().map(|b| b.as_ref()).collect_vec()
        )
    }

    fn render_list_index(&self, list_index: &lang::ListIndex) -> T::DrawResult {
        self.draw_nested_borders_around(&|| {
            self.ui_toolkit.draw_all_on_same_line(&[
                &|| self.render_without_nesting(&|| self.render_code(&list_index.list_expr)),
                &|| self.render_without_nesting(&|| self.render_nested(&|| self.render_code(&list_index.index_expr))),
            ])
        })
    }

    fn render_struct_field_get(&self, sfg: &lang::StructFieldGet) -> T::DrawResult {
        let struct_field = self.env_genie.find_struct_field(sfg.struct_field_id).unwrap();

        self.draw_nested_borders_around(&|| {
            self.ui_toolkit.draw_all_on_same_line(&[
                &|| self.render_code(&sfg.struct_expr),
                &|| {
                    self.render_nested(&|| {
                        self.render_name_with_type_definition(&struct_field.name, BLUE_COLOR, &struct_field.field_type)
                    })
                }
            ])
        })
    }

    fn render_struct_identifier(&self, strukt: &structs::Struct,
                                _struct_literal: &lang::StructLiteral) -> T::DrawResult {
        // TODO: handle when the typespec ain't available
        self.draw_button(&strukt.name, BLUE_COLOR, &|| {})
    }

    fn render_conditional(&self, conditional: &lang::Conditional) -> T::DrawResult {
        self.ui_toolkit.draw_all(vec![
            self.ui_toolkit.draw_all_on_same_line(&[
                &|| { self.draw_button("If", GREY_COLOR, &||{}) },
                &|| { self.render_code(&conditional.condition) },
            ]),
            self.render_indented(&|| { self.render_code(&conditional.true_branch) }),
        ])
    }

    fn render_match(&self, mach: &lang::Match) -> T::DrawResult {
        let mut drawn = vec![
            self.ui_toolkit.draw_all_on_same_line(&[
                &|| { self.draw_button("Match", GREY_COLOR, &||{}) },
                &|| { self.render_code(&mach.match_expression) },
            ])];

        let type_and_enum_by_variant_id =
            self.code_editor.code_genie.match_variant_by_variant_id(mach, self.env_genie);

        drawn.extend(mach.branch_by_variant_id.iter().map(|(variant_id, branch)| {
            let match_variant = type_and_enum_by_variant_id.get(variant_id).unwrap();
            self.render_indented(&|| {
                self.ui_toolkit.draw_all(vec![
                    self.ui_toolkit.draw_all_on_same_line(&[
                        &|| {
                            let type_symbol = self.env_genie.get_symbol_for_type(&match_variant.typ);
                            self.ui_toolkit.draw_button(&type_symbol, BLACK_COLOR, &|| {})
                        },
                        &|| {
                            self.ui_toolkit.draw_button(&match_variant.enum_variant.name, PURPLE_COLOR,
                                                        &|| {})
                        },
                    ]),
                    self.render_indented(&|| self.render_code(branch))
                ])
            })
        }));
        self.ui_toolkit.draw_all(drawn)
    }

    fn render_indented(&self, draw_fn: &Fn() -> T::DrawResult) -> T::DrawResult {
        self.ui_toolkit.indent(PX_PER_INDENTATION_LEVEL, draw_fn)
    }

    fn render_inline_editable_button(&self, label: &str, color: Color, insertion_point: InsertionPoint) -> T::DrawResult {
        let cmd_buffer = Rc::clone(&self.command_buffer);
        self.draw_button(label, color, move || {
            cmd_buffer.borrow_mut().add_editor_command(move |editor| {
                editor.mark_as_editing(insertion_point);
            })
        })
    }

    fn render_string_literal(&self, string_literal: &lang::StringLiteral) -> T::DrawResult {
        self.render_inline_editable_button(
            &format!("\u{F10D} {} \u{F10E}", string_literal.value),
            CLEAR_COLOR,
            InsertionPoint::Editing(string_literal.id))
    }

    fn render_number_literal(&self, number_literal: &lang::NumberLiteral) -> T::DrawResult {
        // TODO: for now lettttttt's not implement the editor for number literals. i think we
        // don't need it just yet. the insert code menu can insert number literals. perhaps we
        // can implement an InsertionPoint::Replace(node_id) that will suffice for number literals
        self.render_inline_editable_button(&number_literal.value.to_string(),
                                           CLEAR_COLOR,
                                           InsertionPoint::Editing(number_literal.id))
    }

    fn draw_inline_editor(&self, code_node: &CodeNode) -> T::DrawResult {
        // this is kind of a mess. render_insert_code_node() does `focus` inside of
        // it. the other parts of the branch need to be wrapped in focus() but not
        // render_insert_code_node()
        match code_node {
            CodeNode::StringLiteral(string_literal) => {
                self.ui_toolkit.focused(&move ||{
                    let new_literal = string_literal.clone();
                    self.draw_inline_text_editor(
                        &string_literal.value,
                        move |new_value| {
                            let mut sl = new_literal.clone();
                            sl.value = new_value.to_string();
                            CodeNode::StringLiteral(sl)
                        })
                })
            },
            CodeNode::Assignment(assignment) => {
                self.ui_toolkit.focused(&|| {
                    let a = assignment.clone();
                    self.draw_inline_text_editor(
                        &assignment.name,
                        move |new_value| {
                            let mut new_assignment = a.clone();
                            new_assignment.name = new_value.to_string();
                            CodeNode::Assignment(new_assignment)
                        })
                })
            },
            CodeNode::Argument(_) | CodeNode::StructLiteralField(_) => {
                // TODO: need to show the argument name or struct literal field, i think we can
                // copy the render_list_literal approach here
                self.render_insert_code_node()
            }
            // the list literal renders its own editor inline
            CodeNode::ListLiteral(list_literal) => {
                self.render_list_literal(list_literal, code_node)
            }
            _ => {
                // TODO: this is super hacks. the editor just reaches in and makes something not
                // editing while rendering lol
                self.command_buffer.borrow_mut().add_editor_command(|e| e.mark_as_not_editing());
                self.draw_button(&format!("Not possible to edit {:?}", code_node), RED_COLOR, &||{})
            }
        }
    }

    fn draw_inline_text_editor<F: Fn(&str) -> CodeNode + 'static>(&self, initial_value: &str, new_node_fn: F) -> T::DrawResult {
        let cmd_buffer = Rc::clone(&self.command_buffer);
        let cmd_buffer2 = Rc::clone(&self.command_buffer);

        let new_node_fn = Rc::new(new_node_fn);

        self.draw_text_input(
            initial_value,
            move |new_value| {
                let new_node_fn = Rc::clone(&new_node_fn);

                let new_value = new_value.to_string();
                cmd_buffer.borrow_mut().add_editor_command(move |editor| {
                    editor.replace_code(new_node_fn(&new_value))
                })
            },
            move || {
                cmd_buffer2.borrow_mut().add_editor_command(|editor| {
                    editor.mark_as_not_editing();
                })
            },
            // TODO: i think we need another callback for what happens when you CANCEL
        )
    }

    fn draw_button<F: Fn() + 'static>(&self, label: &str, color: Color, onclick: F) -> T::DrawResult {
        let onclick_rc = Rc::new(RefCell::new(onclick));
        self.draw_nested_borders_around(& || {
            let onclick_rc = Rc::clone(&onclick_rc);
            self.ui_toolkit.draw_button(label, color, move|| onclick_rc.borrow()())
        })
    }

    fn draw_small_button<F: Fn() + 'static>(&self, label: &str, color: Color, onclick: F) -> T::DrawResult {
        let onclick_rc = Rc::new(RefCell::new(onclick));
        self.draw_nested_borders_around(& || {
            let onclick_rc = Rc::clone(&onclick_rc);
            self.ui_toolkit.draw_small_button(label, color, move|| onclick_rc.borrow()())
        })
    }

    fn draw_text(&self, text: &str) -> T::DrawResult {
        self.draw_nested_borders_around(&|| self.ui_toolkit.draw_text(text))
    }

    fn draw_text_input<F: Fn(&str) + 'static, D: Fn() + 'static>(&self, existing_value: &str, onchange: F,
                                                                 ondone: D) -> T::DrawResult {
        let onchange_rc = Rc::new(RefCell::new(onchange));
        let ondone_rc = Rc::new(RefCell::new(ondone));
        self.draw_nested_borders_around(&|| {
            let onchange_rc = Rc::clone(&onchange_rc);
            let ondone_rc = Rc::clone(&ondone_rc);
            self.ui_toolkit.draw_text_input(existing_value,
                                            move |v| onchange_rc.borrow()(v),
                                            move || ondone_rc.borrow()())
        })
    }

    fn insertion_point(&self) -> Option<InsertionPoint> {
        Some(self.code_editor.insert_code_menu.as_ref()?.insertion_point)
    }
}

struct PerEditorCommandBuffer {
    actual_command_buffer: Rc<RefCell<editor::CommandBuffer>>,
    editor_id: lang::ID,
}

impl PerEditorCommandBuffer {
    pub fn new(actual_command_buffer: Rc<RefCell<editor::CommandBuffer>>, editor_id: lang::ID) -> Self {
        Self { actual_command_buffer, editor_id }
    }

    pub fn add_editor_command<F: FnOnce(&mut code_editor::CodeEditor) + 'static>(&mut self, f: F) {
        let editor_id = self.editor_id;
        self.actual_command_buffer.borrow_mut().add_controller_command(move |controller| {
            controller.get_editor_mut(editor_id).map(f);
        });

        // update the function that the code being edited belongs to
        self.actual_command_buffer.borrow_mut().add_integrating_command(move |cont, interpreter, _, _| {
            let mut env = interpreter.env.borrow_mut();

            let editor = cont.get_editor_mut(editor_id).unwrap();
            let code = editor.get_code().clone();
            code_editor::update_code_in_env(editor.location, code, cont, &mut env)
        });
    }
}