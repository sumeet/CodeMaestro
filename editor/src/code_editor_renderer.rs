use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use itertools::Itertools;

use super::code_editor;
use super::code_editor::InsertionPoint;
use super::editor;
use super::insert_code_menu::{InsertCodeMenu, InsertCodeMenuOption};
use super::ui_toolkit::{Color, UiToolkit};
use crate::colorscheme;
use crate::draw_all_iter;
use crate::editor::Keypress;
use crate::insert_code_menu::{CodeSearchParams, InsertCodeMenuOptionsGroup};
use crate::ui_toolkit::{ChildRegionHeight, DrawFnRef};
use cs::env_genie::EnvGenie;
use cs::lang;
use cs::lang::CodeNode;
use cs::structs;

pub const PLACEHOLDER_ICON: &str = "\u{F071}";
// TODO: move this into the color scheme, but we'll leave it in here for now -- lazy
pub const BLACK_COLOR: Color = [0.0, 0.0, 0.0, 1.0];

pub const PX_PER_INDENTATION_LEVEL: i16 = 20;

fn transparency(mut color: Color, p: f32) -> Color {
    color[3] = p;
    color
}

pub struct CodeEditorRenderer<'a, T> {
    ui_toolkit: &'a T,
    arg_nesting_level: RefCell<u32>,
    code_editor: &'a code_editor::CodeEditor,
    command_buffer: Rc<RefCell<PerEditorCommandBuffer>>,
    env_genie: &'a EnvGenie<'a>,
    // this is used for rendering code... if we're in menu rendering mode then don't go into edit
    // mode and clicks also shouldn't do anything. surely a cleaner way to do this but whatever RN
    is_rendering_menu: RefCell<bool>,
    render_menu_when_next_possible: RefCell<bool>,
}

// ok stupid but all the methods on this take &self instead of &mut self because the ImGui closures
// all take Fn instead of FnMut
impl<'a, T: UiToolkit> CodeEditorRenderer<'a, T> {
    pub fn new(ui_toolkit: &'a T,
               code_editor: &'a code_editor::CodeEditor,
               command_buffer: Rc<RefCell<editor::CommandBuffer>>,
               env_genie: &'a EnvGenie)
               -> Self {
        let command_buffer = PerEditorCommandBuffer::new(command_buffer, code_editor.id());
        Self { ui_toolkit,
               code_editor,
               command_buffer: Rc::new(RefCell::new(command_buffer)),
               arg_nesting_level: RefCell::new(0),
               env_genie,
               is_rendering_menu: RefCell::new(false),
               render_menu_when_next_possible: RefCell::new(false) }
    }

    pub fn render(&self) -> T::DrawResult {
        let code = self.code_editor.get_code();
        let cmd_buffer1 = Rc::clone(&self.command_buffer);
        let cmd_buffer = Rc::clone(&self.command_buffer);
        self.ui_toolkit.draw_child_region(colorscheme!(child_region_bg_color),
                                          &|| self.render_code(code),
                                          ChildRegionHeight::ExpandFill { min_height: 100. },
                                          Some(&move || {
                                              let cmd_buffer1 = Rc::clone(&cmd_buffer1);
                                              self.ui_toolkit.draw_menu_item("Insert code", move || {
                                                  cmd_buffer1.borrow_mut().add_editor_command(move |code_editor| {
                                                      code_editor.set_insertion_point_on_next_line_in_block();
                                                  })
                                              })
                                          }),
                                          Some(move |keypress| {
                                              cmd_buffer.borrow_mut()
                                                        .add_editor_command(move |code_editor| {
                                                            code_editor.handle_keypress(keypress)
                                                        })
                                          }))
    }

    fn is_insertion_pointer_immediately_before(&self, id: lang::ID) -> bool {
        let insertion_point = self.code_editor.insertion_point();
        match insertion_point {
            Some(InsertionPoint::Before(code_node_id)) if code_node_id == id => true,
            _ => false,
        }
    }

    fn draw_code_node_and_insertion_point_if_before_or_after(&self,
                                                             code_node: &CodeNode,
                                                             draw: &dyn Fn() -> T::DrawResult)
                                                             -> T::DrawResult {
        self.ui_toolkit.draw_all(&[
            &|| {
                if self.is_insertion_pointer_immediately_before(code_node.id()) {
                    self.render_insert_code_node()
                } else {
                    self.ui_toolkit.draw_all(&[])
                }
            },
            draw,
            &|| {
                if self.is_insertion_pointer_immediately_after(code_node.id()) {
                    self.render_insert_code_node()
                } else {
                    self.ui_toolkit.draw_all(&[])
                }
            },
        ])
    }

    fn is_insertion_pointer_immediately_after(&self, id: lang::ID) -> bool {
        match self.code_editor.insertion_point() {
            Some(InsertionPoint::After(code_node_id)) if code_node_id == id => true,
            _ => false,
        }
    }

    fn draw_selected(&self,
                     scroll_hash: String,
                     draw: &dyn Fn() -> T::DrawResult)
                     -> T::DrawResult {
        self.ui_toolkit
            .scrolled_to_y_if_not_visible(scroll_hash, &|| {
                self.ui_toolkit
                    .draw_box_around(colorscheme!(selection_overlay_color), draw)
            })
    }

    fn render_assignment(&self, assignment: &lang::Assignment) -> T::DrawResult {
        let type_of_assignment = self.code_editor
                                     .code_genie
                                     .guess_type(assignment.expression.as_ref(), self.env_genie);
        self.ui_toolkit.draw_all_on_same_line(&[
            &|| {
                self.set_selected_on_click(&|| {
                    self.render_name_with_type_definition(&assignment.name, colorscheme!(variable_color), &type_of_assignment)
                }, assignment.id)
            },
            &|| self.draw_text("   \u{f52c}   "),
            &|| self.render_code(assignment.expression.as_ref()),
        ])
    }

    fn render_insert_code_node(&self) -> T::DrawResult {
        let menu = self.code_editor.insert_code_menu.as_ref().unwrap();

        // this only renders the field for entering text inline where the insertion happens.
        //
        // we'll actually render the menu after we finish rendering the current block expression
        self.render_menu_when_next_possible.replace(true);
        self.ui_toolkit.focused(&|| {
                           let cmdb_1 = Rc::clone(&self.command_buffer);
                           let cmdb_2 = Rc::clone(&self.command_buffer);
                           let insertion_point = menu.insertion_point.clone();
                           let new_code_node = menu.selected_option_code(&self.code_editor
                                                                              .code_genie,
                                                                         self.env_genie);

                           self.draw_text_input(
                                                menu.input_str(),
                                                move |input| {
                                                    let input = input.to_string();
                                                    cmdb_1.borrow_mut()
                                                          .add_editor_command(move |editor| {
                                                              editor.insert_code_menu
                                                                    .as_mut()
                                                                    .map(|menu| {
                                                                        menu.set_search_str(&input);
                                                                    });
                                                          })
                                                },
                                                move || {
                                                    let mut cmdb = cmdb_2.borrow_mut();
                                                    if let Some(ref new_code_node) = new_code_node {
                                                        let new_code_node = new_code_node.clone();
                                                        cmdb.add_editor_command(move |editor| {
                            editor.hide_insert_code_menu();
                            editor.insert_code(new_code_node.clone(),
                                               insertion_point);
                        });
                                                    } else {
                                                        cmdb.add_editor_command(|editor| {
                                                                editor.undo();
                                                                editor.hide_insert_code_menu();
                                                            });
                                                    }
                                                },
            )
                       })
    }

    fn render_code_insertion_menu_here_if_it_was_requested(&self) -> T::DrawResult {
        let should_render_menu = self.render_menu_when_next_possible.replace(false);
        if should_render_menu {
            self.render_code_insertion_menu()
        } else {
            self.ui_toolkit.draw_all(&[])
        }
    }

    fn render_code_insertion_menu(&self) -> T::DrawResult {
        self.is_rendering_menu.replace(true);
        let menu = self.code_editor.insert_code_menu.as_ref().unwrap();
        let drawn = self.render_without_nesting(&|| self.render_insertion_options(&menu));
        self.is_rendering_menu.replace(false);
        drawn
    }

    // TODO: this could be a constant but i'm lazy rn
    fn transparent_black(&self) -> Color {
        transparency(BLACK_COLOR, 0.5)
    }

    fn render_insertion_options(&self, menu: &InsertCodeMenu) -> T::DrawResult {
        let transparent_black = self.transparent_black();
        self.ui_toolkit.draw_all(&[
            &|| {
                self.ui_toolkit.draw_with_no_spacing_afterwards(&|| {
                    self.ui_toolkit.draw_with_bgcolor(transparent_black, &|| {
                        self.ui_toolkit.draw_taking_up_full_width(&|| {
                            self.render_insertion_header(menu)
                        })
                    })
                })
            },
            &|| {
                self.ui_toolkit.draw_child_region(transparent_black,
                                                  &|| {
                                                      let options_groups = menu.grouped_options(&self.code_editor.code_genie, self.env_genie);
                                                          draw_all_iter!(T::self.ui_toolkit,
                                                      options_groups.iter().map(|group| {
                                                          move || self.render_insertion_options_group(group, menu.insertion_point)
                                                      })
                                                  )
                                                  },
                                                  ChildRegionHeight::Pixels(300),
                                                  None::<&dyn Fn() -> T::DrawResult>,
                                                  None::<fn(Keypress)>)
            }
        ])
    }

    fn render_insertion_header(&self, menu: &InsertCodeMenu) -> T::DrawResult {
        // TODO: show the type of the thing being inserted
        // show hints: like if you want a list of something, type list
        // or if the type is number, then tell the user they can start typing numbers
        // or even better, maybe have clickable numbers? idk how that would work tho
        let search_params = menu.search_params(&self.code_editor.code_genie, self.env_genie);
        //        self.ui_toolkit.draw_with_margin((0., 0.), &|| {
        self.ui_toolkit
            .draw_with_bgcolor(transparency(BLACK_COLOR, 0.68), &|| {
                self.ui_toolkit.draw_taking_up_full_width(&|| {
                                   self.ui_toolkit
                    .draw_all(&[&|| self.render_insertion_point_header(menu.insertion_point),
                        &|| self.render_insertion_type_information(&search_params),
                        &|| self.render_wraps_type_information(&search_params)])
                               })
            })
        //                       })
    }

    fn render_insertion_type_information(&self, search_params: &CodeSearchParams) -> T::DrawResult {
        let typ = search_params.return_type.as_ref();
        if let Some(typ) = typ {
            self.ui_toolkit.draw_all_on_same_line(&[
                &|| {
                    self.ui_toolkit
                        .draw_text("Showing options resulting in type")
                },
                &|| {
                    let type_name = self.env_genie
                                        .get_name_for_type(typ)
                                        .ok_or_else(|| {
                                            format!("couldn't find name for type w/ typespec id {}",
                                                    typ.typespec_id)
                                        })
                                        .unwrap();
                    self.render_name_with_type_definition(&type_name, BLACK_COLOR, typ)
                },
            ])
        } else {
            self.ui_toolkit.draw_all(&[])
        }
    }

    fn render_wraps_type_information(&self, search_params: &CodeSearchParams) -> T::DrawResult {
        if let Some(wraps_type) = search_params.wraps_type.as_ref() {
            self.ui_toolkit
                .draw_all_on_same_line(&[&|| self.ui_toolkit.draw_text("Showing options accepting input type"), &|| {
                    let type_name =
                        self.env_genie.get_name_for_type(wraps_type).unwrap();
                    self.render_name_with_type_definition(&type_name,
                                                          BLACK_COLOR,
                                                          wraps_type)
                }])
        } else {
            self.ui_toolkit.draw_all(&[])
        }
    }

    fn render_insertion_point_header(&self, insertion_point: InsertionPoint) -> T::DrawResult {
        match insertion_point {
            InsertionPoint::BeginningOfBlock(_)
            | InsertionPoint::Before(_)
            | InsertionPoint::After(_) => self.draw_operation_label("New code insert"),
            InsertionPoint::StructLiteralField(struct_literal_field_id) => {
                let struct_literal_field = self.code_editor
                                               .code_genie
                                               .find_node(struct_literal_field_id)
                                               .unwrap();
                let struct_literal_field =
                    struct_literal_field.into_struct_literal_field().unwrap();
                let (strukt, struct_field) =
                    self.env_genie
                        .find_struct_and_field(struct_literal_field.struct_field_id)
                        .unwrap();
                self.ui_toolkit.draw_all_on_same_line(&[
                    &|| self.draw_operation_label("Struct field insertion"),
                    &|| self.render_struct_identifier(strukt),
                    &|| self.ui_toolkit.draw_text("for"),
                    &|| self.render_struct_literal_field_label(struct_field),
                ])
            }
            InsertionPoint::Editing(edited_code_node_id) => {
                // TODO: we should probably show the old value in here, lul
                // TODO: ok, maybe later, because right now we don't even show the menu here
                let code_node = self.code_editor
                                    .code_genie
                                    .find_node(edited_code_node_id)
                                    .unwrap();
                self.ui_toolkit
                    .draw_all_on_same_line(&[&|| self.draw_operation_label("Edit"), &|| {
                                               self.render_code(code_node)
                                           }])
            }
            InsertionPoint::ListLiteralElement { list_literal_id,
                                                 pos, } => {
                let code_node = self.code_editor
                                    .code_genie
                                    .find_node(list_literal_id)
                                    .unwrap();
                self.ui_toolkit.draw_all_on_same_line(&[
                    &|| self.draw_operation_label("Insertion into list"),
                    &|| self.render_list_literal_label(code_node),
                    &|| self.ui_toolkit.draw_text("at position"),
                    &|| self.render_list_literal_position(pos),
                ])
            }
            InsertionPoint::Replace(code_node_being_replaced_id) => {
                let code_node_being_replaced = self.code_editor
                                                   .code_genie
                                                   .find_node(code_node_being_replaced_id)
                                                   .unwrap();
                self.ui_toolkit
                    .draw_all_on_same_line(&[
                        &|| self.draw_operation_label("Replace operation"),
                        &|| self.ui_toolkit.draw_text("replacing"),
                        &|| self.render_code(code_node_being_replaced),
                    ])
            }
            InsertionPoint::Wrap(code_node_being_wrapped_id) => {
                let code_node_being_wrapped = self.code_editor
                                                  .code_genie
                                                  .find_node(code_node_being_wrapped_id)
                                                  .unwrap();
                self.ui_toolkit
                    .draw_all_on_same_line(&[
                        &|| self.draw_operation_label("Wrap operation"),
                        &|| self.ui_toolkit.draw_text("around"),
                        &|| self.render_code(code_node_being_wrapped)
                    ])
            }
        }
    }

    fn draw_operation_label(&self, text: &str) -> T::DrawResult {
        self.ui_toolkit
            .draw_buttony_text(text, colorscheme!(cool_color))
    }

    fn render_insertion_options_group(&self,
                                      group: &InsertCodeMenuOptionsGroup,
                                      insertion_point: InsertionPoint)
                                      -> T::DrawResult {
        self.ui_toolkit.draw_all(&[
        &|| self.ui_toolkit.draw_full_width_heading(BLACK_COLOR, (5., 5.), group.group_name),
        &|| draw_all_iter!(T::self.ui_toolkit,
                group.options.iter().enumerate()
                .map(|(index, option)| {
                    move || {
                        let scroll_hash = self.insertion_option_menu_hash(index, &group.group_name, &insertion_point);
                        self.render_insertion_option(scroll_hash, option, insertion_point)
                    }}))
    ])
    }

    fn render_insertion_option(&'a self,
                               scroll_hash: String,
                               option: &'a InsertCodeMenuOption,
                               insertion_point: InsertionPoint)
                               -> T::DrawResult {
        let cmd_buffer = Rc::clone(&self.command_buffer);
        let new_code_node = option.new_node.clone();

        let draw = move || {
            let new_code_node = new_code_node.clone();
            let cmdb = cmd_buffer.clone();
            self.ui_toolkit.buttonize(
                                      &|| {
                                          self.ui_toolkit.draw_taking_up_full_width(&|| {
                      match &self.help_text(&option.new_node) {
                          Some(help_text) if !help_text.is_empty() => {
                              self.ui_toolkit.draw_all(&[
                                    &|| self.render_code(&option.new_node),
                                    &|| self.ui_toolkit.draw_wrapped_text(self.darken(colorscheme!(text_color)), &help_text),
                              ])
                          }
                          _ => self.render_code(&option.new_node),
                      }
                  })
                                      },
                                      move || {
                                          let ncn = new_code_node.clone();
                                          cmdb.borrow_mut().add_editor_command(move |editor| {
                                                               editor.hide_insert_code_menu();
                                                               editor.insert_code(ncn.clone(),
                                                                                  insertion_point);
                                                           });
                                      },
            )
        };
        if option.is_selected {
            self.draw_selected(scroll_hash, &draw)
        } else {
            draw()
        }
    }

    fn insertion_option_menu_hash(&self,
                                  index: usize,
                                  group_name: &str,
                                  insertion_point: &InsertionPoint)
                                  -> String {
        format!("{}:{}:{:?}", index, group_name, insertion_point)
    }

    fn render_list_literal_label(&self, code_node: &CodeNode) -> T::DrawResult {
        let t = self.code_editor
                    .code_genie
                    .guess_type(code_node, self.env_genie);
        // TODO: we can use smth better to express the nesting than ascii art, like our nesting scheme
        //       with the black lines (can actually make that generic so we can swap it with something
        //       else
        let type_symbol = self.env_genie.get_symbol_for_type(&t);
        self.ui_toolkit
            .draw_buttony_text(&type_symbol, colorscheme!(cool_color))
    }

    fn render_list_literal_position(&self, pos: usize) -> T::DrawResult {
        self.ui_toolkit
            .draw_buttony_text(&pos.to_string(), BLACK_COLOR)
    }

    fn render_list_literal(&self,
                           list_literal: &lang::ListLiteral,
                           code_node: &lang::CodeNode)
                           -> T::DrawResult {
        let lhs = &|| {
            self.set_selected_on_click(&|| self.render_list_literal_label(code_node),
                                       list_literal.id)
        };

        let insert_pos = match self.code_editor.insert_code_menu {
            Some(InsertCodeMenu { insertion_point:
                                      InsertionPoint::ListLiteralElement { list_literal_id,
                                                                           pos, },
                                  .. })
                if list_literal_id == list_literal.id =>
            {
                Some(pos)
            }
            _ => None,
        };

        let mut rhs: Vec<Box<dyn Fn() -> T::DrawResult>> = vec![];
        let mut position_label = 0;
        let mut i = 0;
        while i <= list_literal.elements.len() {
            if insert_pos.map_or(false, |insert_pos| insert_pos == i) {
                rhs.push(Box::new(move || {
                       self.ui_toolkit.draw_all_on_same_line(&[
                        &|| self.render_list_literal_position(position_label),
                        &|| self.render_nested(&|| self.render_insert_code_node()),
                    ])
                   }));
                position_label += 1;
            }

            list_literal.elements.get(i).map(|el| {
                                            rhs.push(Box::new(move || {
                                                   self.ui_toolkit.draw_all_on_same_line(&[
                        &|| self.render_list_literal_position(position_label),
                        &|| self.render_nested(&|| self.render_code(el)),
                    ])
                                               }));
                                            position_label += 1;
                                        });
            i += 1;
        }

        self.ui_toolkit
            .align(lhs, &rhs.iter().map(|c| c.as_ref()).collect_vec())
    }

    fn render_nested(&self, draw_fn: &dyn Fn() -> T::DrawResult) -> T::DrawResult {
        self.arg_nesting_level.replace_with(|l| *l + 1);
        let drawn = draw_fn();
        self.arg_nesting_level.replace_with(|l| *l - 1);
        drawn
    }

    fn render_without_nesting(&self, draw_fn: &dyn Fn() -> T::DrawResult) -> T::DrawResult {
        let old_nesting_level = self.arg_nesting_level.replace(0);
        let drawn = draw_fn();
        self.arg_nesting_level.replace(old_nesting_level);
        drawn
    }

    fn draw_nested_borders_around(&self,
                                  draw_element_fn: &dyn Fn() -> T::DrawResult)
                                  -> T::DrawResult {
        let nesting_level = *self.arg_nesting_level.borrow();
        if nesting_level == 0 {
            return draw_element_fn();
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

    fn help_text(&self, code_node: &CodeNode) -> Option<String> {
        match code_node {
            CodeNode::FunctionCall(function_call) => {
                let function = self.env_genie
                                   .find_function(function_call.function_reference().function_id)?;
                Some(function.description().to_string())
            }
            CodeNode::FunctionReference(_) => None,
            CodeNode::Argument(_) => None,
            CodeNode::StringLiteral(_) => Some(lang::STRING_TYPESPEC.description.clone()),
            CodeNode::NullLiteral(_) => Some(lang::NULL_TYPESPEC.description.clone()),
            CodeNode::Assignment(_) => Some("Make a new variable".to_string()),
            CodeNode::Block(_) => None,
            CodeNode::VariableReference(vr) => {
                let (_name, typ) = self.lookup_variable_name_and_type(vr)?;
                let typespec = self.env_genie.find_typespec(typ.typespec_id)?;
                Some(typespec.description().to_string())
            }
            CodeNode::Placeholder(_) => {
                Some("A placeholder. Use this when you don't know what you want yet, or nothing else fits. The program won't actually run if a placeholder is present.".into())
            },
            CodeNode::StructLiteral(struct_literal) => {
                let ts = self.env_genie.find_typespec(struct_literal.struct_id)?;
                Some(ts.description().into())
            }
            CodeNode::StructLiteralField(_) => None,
            CodeNode::Conditional(_) => Some("Insert an if-statement".into()),
            CodeNode::Match(_) => {
                Some("Handle enumerations that have multiple possible values".into())
            }
            CodeNode::ListLiteral(list_literal) => {
                let type_name = self.env_genie.get_name_for_type(&list_literal.element_type)
                    .expect("wtf we couldn't find the type?");
                Some(format!("A new list of {}", type_name))
            },
            CodeNode::StructFieldGet(sfg) => {
                let struct_field = self.env_genie.find_struct_field(sfg.struct_field_id)?;
                Some(struct_field.description.clone())
            }
            CodeNode::NumberLiteral(_) => Some(lang::NUMBER_TYPESPEC.description.clone()),
            CodeNode::ListIndex(_) => None,
        }
    }

    fn render_code(&self, code_node: &CodeNode) -> T::DrawResult {
        // TODO: lots of is_rendering_menu_atm() checks... how can we clean it up?
        if self.is_editing(code_node.id()) && !self.is_rendering_menu_atm() {
            return self.draw_inline_editor(code_node);
        }

        match self.insertion_point() {
            Some(InsertionPoint::Replace(id)) | Some(InsertionPoint::Wrap(id))
                if { id == code_node.id() && !self.is_rendering_menu_atm() } =>
            {
                return self.render_insert_code_node()
            }
            _ => {}
        }

        let draw = || {
            match code_node {
                CodeNode::FunctionCall(function_call) => self.render_function_call(&function_call),
                CodeNode::StringLiteral(string_literal) => {
                    self.render_string_literal(&string_literal)
                }
                CodeNode::NumberLiteral(number_literal) => {
                    self.render_number_literal(&number_literal)
                }
                CodeNode::Assignment(assignment) => self.render_assignment(&assignment),
                CodeNode::Block(block) => self.render_block(&block),
                CodeNode::VariableReference(variable_reference) => {
                    self.render_variable_reference(&variable_reference)
                }
                CodeNode::FunctionReference(function_reference) => {
                    self.render_function_reference(&function_reference)
                }
                CodeNode::Argument(argument) => self.render_function_call_argument(&argument),
                CodeNode::Placeholder(placeholder) => self.render_placeholder(&placeholder),
                CodeNode::NullLiteral(null_literal_id) => self.render_null_literal(null_literal_id),
                CodeNode::StructLiteral(struct_literal) => {
                    self.render_struct_literal(&struct_literal)
                }
                CodeNode::StructLiteralField(_field) => {
                    panic!("struct literal fields shouldn't be rendered from here");
                    //self.ui_toolkit.draw_all(vec![])
                    // we would, except render_struct_literal_field isn't called from here...
                    //self.render_struct_literal_field(&field)
                }
                CodeNode::Conditional(conditional) => self.render_conditional(&conditional),
                CodeNode::Match(mach) => self.render_match(&mach),
                CodeNode::ListLiteral(list_literal) => {
                    self.render_list_literal(&list_literal, code_node)
                }
                CodeNode::StructFieldGet(sfg) => self.render_struct_field_get(&sfg),
                CodeNode::ListIndex(list_index) => self.render_list_index(&list_index),
            }
        };

        let draw = || self.render_context_menu(code_node, &draw);

        if self.is_selected(code_node.id()) {
            self.draw_selected(self.code_node_cursor_scroll_hash(code_node), &draw)
        } else {
            self.draw_code_node_and_insertion_point_if_before_or_after(code_node, &draw)
        }
    }

    pub fn render_context_menu(&self,
                               code_node: &CodeNode,
                               draw_code_fn: DrawFnRef<T>)
                               -> T::DrawResult {
        match code_node {
            CodeNode::FunctionReference(_)
            | CodeNode::StringLiteral(_)
            | CodeNode::NullLiteral(_)
            | CodeNode::Assignment(_)
            | CodeNode::VariableReference(_)
            | CodeNode::Placeholder(_)
            | CodeNode::StructLiteral(_)
            | CodeNode::StructLiteralField(_)
            | CodeNode::Conditional(_)
            | CodeNode::Match(_)
            | CodeNode::ListLiteral(_)
            | CodeNode::StructFieldGet(_)
            | CodeNode::NumberLiteral(_)
            | CodeNode::ListIndex(_) => {
                self.ui_toolkit.context_menu(draw_code_fn, &|| {
                                   let cmd_buffer = Rc::clone(&self.command_buffer);
                                   self.ui_toolkit.draw_menu_item("Delete", move || {
                                                      cmd_buffer.borrow_mut()
                                                                .add_editor_command(|editor| {
                                                                    editor.delete_selected_code();
                                                                })
                                                  })
                               })
            }
            CodeNode::FunctionCall(_) | CodeNode::Block(_) | CodeNode::Argument(_) => {
                draw_code_fn()
            }
        }
    }

    // this identifies the current selection to the UI, so it can remember the scroll of the current
    // item
    fn code_node_cursor_scroll_hash(&self, code_node: &lang::CodeNode) -> String {
        format!("{:?}:{}", self.code_editor.location, code_node.id())
    }

    fn render_null_literal(&self, null_literal_id: &lang::ID) -> T::DrawResult {
        self.set_selected_on_click(&|| self.draw_text(&format!(" {} ", lang::NULL_TYPESPEC.symbol)),
                                   *null_literal_id)
    }

    fn render_placeholder(&self, placeholder: &lang::Placeholder) -> T::DrawResult {
        // TODO: maybe use the traffic cone instead of the exclamation triangle,
        // which is kinda hard to see
        self.set_selected_on_click(&|| {
                                       self.ui_toolkit
                                           .draw_buttony_text(&format!("{} {}",
                                                                       PLACEHOLDER_ICON,
                                                                       placeholder.description),
                                                              colorscheme!(warning_color))
                                   },
                                   placeholder.id)
    }

    fn render_function_reference(&self,
                                 function_reference: &lang::FunctionReference)
                                 -> T::DrawResult {
        let function_id = function_reference.function_id;

        // TODO: don't do validation in here. this is just so i can see what this error looks
        // like visually. for realz, i would probably be better off having a separate validation
        // step. and THEN show the errors in here. or maybe overlay something on the codenode that
        // contains the error
        //
        // UPDATE: so i tried that, but figured i still needed to have this code here. i guess maybe
        // there's gonna be no avoiding doing double validation in some situations, and that's ok
        // i think
        let func = self.env_genie.find_function(function_id);
        if func.is_none() {
            let error_msg = format!("Error: function ID {} not found", function_id);
            return self.draw_button(&error_msg, colorscheme!(danger_color), &|| {});
        }
        let func = func.unwrap();
        // TODO: rework this for when we have generics. the function arguments will need to take
        // the actual parameters as parameters
        self.render_function_name(&func.name(), colorscheme!(action_color), &func.returns())
    }

    fn render_variable_reference(&self,
                                 variable_reference: &lang::VariableReference)
                                 -> T::DrawResult {
        let draw = &|| {
            if let Some((name, typ)) = self.lookup_variable_name_and_type(variable_reference) {
                self.render_name_with_type_definition(&name, colorscheme!(variable_color), &typ)
            } else {
                self.draw_button("Variable reference not found",
                                 colorscheme!(danger_color),
                                 &|| {})
            }
        };
        self.set_selected_on_click(draw, variable_reference.id)
    }

    fn darken(&self, mut color: Color) -> Color {
        color[0] *= 0.75;
        color[1] *= 0.75;
        color[2] *= 0.75;
        color
    }

    fn render_function_name(&self, name: &str, color: Color, typ: &lang::Type) -> T::DrawResult {
        let sym = self.env_genie.get_symbol_for_type(typ);
        //        let darker_color = self.darken(self.darken(self.darken(color)));
        let darker_color = self.darken(self.darken(color));

        self.draw_nested_borders_around(&|| {
            self.ui_toolkit.draw_top_border_inside(darker_color, 2, &|| {
                self.ui_toolkit.draw_right_border_inside(darker_color, 1, &|| {
                    self.ui_toolkit.draw_left_border_inside(darker_color, 1, &|| {
                        self.ui_toolkit.draw_bottom_border_inside(darker_color, 1, &|| {
                            // don't show the return type if the function returns null. there's no
                            // use in looking at it
                            if typ.matches_spec(&lang::NULL_TYPESPEC) {
                                self.ui_toolkit.draw_all_on_same_line(&[
                                    &|| self.ui_toolkit.draw_buttony_text("", darker_color),
                                    &|| self.ui_toolkit.draw_buttony_text(name, color),
                                ])
                            } else {
                                self.ui_toolkit.draw_all_on_same_line(&[
                                    &|| self.ui_toolkit.draw_buttony_text(&format!(" {}", sym), darker_color),
                                    &|| self.ui_toolkit.draw_buttony_text(name, color),
//                                        &|| self.ui_toolkit.draw_text("  "),
//                                        &|| self.ui_toolkit.draw_buttony_text(&sym, darker_color),
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
    fn render_name_with_type_definition(&self,
                                        name: &str,
                                        color: Color,
                                        typ: &lang::Type)
                                        -> T::DrawResult {
        let sym = self.env_genie.get_symbol_for_type(typ);

        let darker_color = self.darken(color);
        self.draw_nested_borders_around(&|| {
                // UHHHH: even though i went throuhg a lot of effort to get the borders working, i
                // think it looks better without them honestly. and also there's this bug when this
                // is on a line by itself on web, it takes up the whole line for some reason. so
                // let's live without the borders, either for now, or permanently.

                //                self.ui_toolkit
                //                    .draw_top_border_inside(darker_color, 2, &|| {
                //                        self.ui_toolkit
                //                            .draw_right_border_inside(darker_color, 1, &|| {
                //                                self.ui_toolkit.draw_left_border_inside(darker_color, 1, &|| {
                //                            self.ui_toolkit.draw_bottom_border_inside(darker_color, 1, &|| {
                //
                // AAAAAAAAAND this doesn't need buttonize
                //                self.ui_toolkit.buttonize(
                //                                          &|| {
                self.ui_toolkit.draw_all_on_same_line(&[&|| {
                                                            self.ui_toolkit
                                                                .draw_buttony_text(&sym,
                                                                                   darker_color)
                                                        },
                                                        &|| {
                                                            self.ui_toolkit
                                                                .draw_buttony_text(name, color)
                                                        }])
                //                                          },
                //                                          || {},
                //            )
                //                            })
                //                        })
                //                            })
                //                    })
            })
    }

    // should we move this into the genie?
    fn lookup_variable_name_and_type(&self,
                                     variable_reference: &lang::VariableReference)
                                     -> Option<(String, lang::Type)> {
        let assignment = self.code_editor
                             .code_genie
                             .find_node(variable_reference.assignment_id);
        if let Some(CodeNode::Assignment(assignment)) = assignment {
            return Some((assignment.name.clone(),
                         self.code_editor
                             .code_genie
                             .guess_type(assignment.expression.as_ref(), self.env_genie)));
        }
        // TODO: this searches all functions, but we could be smarter here because we already know which
        //       function we're inside
        if let Some(arg) = self.env_genie
                               .get_arg_definition(variable_reference.assignment_id)
        {
            return Some((arg.short_name, arg.arg_type));
        }
        // variables can also refer to enum variants
        self.code_editor
            .code_genie
            .find_enum_variant_by_assignment_id(variable_reference.assignment_id, self.env_genie)
            .map(|match_variant| (match_variant.enum_variant.name, match_variant.typ))
    }

    // TODO: combine the insertion point stuff with the insertion point stuff elsewhere, mainly
    // the is_insertion_point_before_or_after stuff
    fn render_block(&self, block: &lang::Block) -> T::DrawResult {
        // TODO: i think i could move the is_insertion_point_before_or_after crapola to here
        let not_inserting_code = self.insertion_point().is_none();

        self.ui_toolkit.draw_all(&[
            &|| match self.code_editor.insertion_point() {
                Some(InsertionPoint::BeginningOfBlock(block_id)) if block.id == block_id => {
                    self.render_insert_code_node()
                }
                _ => self.ui_toolkit.draw_all(&[]),
            },
            &|| {
                if block.expressions.is_empty() {
                    if not_inserting_code {
                        // THE NULL STATE
                        self.render_add_code_here_button(InsertionPoint::BeginningOfBlock(block.id))
                    } else {
                        self.ui_toolkit.draw_all(&[&|| self.render_code_insertion_menu_here_if_it_was_requested()])
                    }
                } else {
                    self.ui_toolkit.draw_all(&[
                        &|| if not_inserting_code {
                            let first_code_id = block.expressions.first().unwrap().id();
                            self.render_add_code_here_line(InsertionPoint::Before(first_code_id))
                        } else {
                            self.ui_toolkit.draw_all(&[])
                        },
                        &|| {
                            draw_all_iter!(T::self.ui_toolkit,
                               block.expressions.iter().map(|code| {
                                   move || {
                                       if not_inserting_code {
                                           self.ui_toolkit.draw_all(&[
                                                &|| self.render_code(code),
                                                &|| self.render_add_code_here_line(InsertionPoint::After(code.id())),
                                           ])
                                       } else {
                                           self.ui_toolkit.draw_all(&[
                                              &|| self.render_code_insertion_menu_here_if_it_was_requested(),
                                              &|| self.render_code(code),
                                              &|| self.render_code_insertion_menu_here_if_it_was_requested(),
                                           ])
                                       }
                                   }
                               })
                           )
                        },
                    ])
                }
            },
        ])
    }

    fn render_add_code_here_line(&self, insertion_point: InsertionPoint) -> T::DrawResult {
        self.ui_toolkit.replace_on_hover(&|| {
                                             self.ui_toolkit
                                                 // HAX: 70 is the size of the Insert Code button lol
                                                 .draw_code_line_separator('\u{f0fe}',
                                                                           70.,
                                                                           2.,
                                                                           colorscheme!(separator_color))
                                         },
                                         &|| self.render_add_code_here_button(insertion_point))
    }

    // this is for the button itself. for the line with hover, see ^
    fn render_add_code_here_button(&self, insertion_point: InsertionPoint) -> T::DrawResult {
        let cmd_buffer = Rc::clone(&self.command_buffer);
        self.ui_toolkit.buttonize(&|| {
                                      self.ui_toolkit.draw_buttony_text("\u{f0fe} Add code",
                                                                        colorscheme!(adding_color))
                                  },
                                  move || {
                                      cmd_buffer.borrow_mut().add_editor_command(move |editor| {
                                          editor.mark_as_editing(insertion_point);
                                      })
                                  })
    }

    fn render_function_call(&self, function_call: &lang::FunctionCall) -> T::DrawResult {
        // XXX: we've gotta have this conditional because of a quirk with the way the imgui
        // toolkit works. if render_function_call_arguments doesn't actually draw anything, it
        // will cause the next drawn thing to appear on the same line. weird i know, maybe we can
        // one day fix this jumbledness
        if function_call.args.is_empty() {
            return self.render_code(&function_call.function_reference);
        }

        let rhs = self.render_function_call_arguments(function_call.function_reference()
                                                                   .function_id,
                                                      function_call.args());
        let rhs: Vec<Box<dyn Fn() -> T::DrawResult>> =
            rhs.iter()
               .map(|cl| {
                   let b: Box<dyn Fn() -> T::DrawResult> = Box::new(move || cl(&self));
                   b
               })
               .collect_vec();

        self.ui_toolkit.align(
                              &|| {
                                  self.set_selected_on_click(
                    &|| self.render_code(&function_call.function_reference),
                    function_call.id)
                              },
                              &rhs.iter().map(|b| b.as_ref()).collect_vec(),
        )
    }

    fn render_function_call_argument(&self, argument: &lang::Argument) -> T::DrawResult {
        let arg_display = {
            match self.env_genie
                      .get_arg_definition(argument.argument_definition_id)
            {
                Some(arg_def) => {
                    let type_symbol = self.env_genie.get_symbol_for_type(&arg_def.arg_type);
                    format!("{} {}", type_symbol, arg_def.short_name)
                }
                None => "\u{f059}".to_string(),
            }
        };

        self.render_nested(&|| {
                self.ui_toolkit.draw_all_on_same_line(&[&|| {
                                                            self.ui_toolkit
                                                                .draw_buttony_text(&arg_display,
                                                                                   BLACK_COLOR)
                                                        },
                                                        &|| {
                                                            self.render_code(argument.expr.as_ref())
                                                        }])
            })
    }

    fn render_args_for_found_function(
        &self,
        function: &dyn lang::Function,
        args: Vec<&lang::Argument>)
        -> Vec<Box<dyn Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> {
        let provided_arg_by_definition_id: HashMap<lang::ID, lang::Argument> =
            args.into_iter()
                .map(|arg| (arg.argument_definition_id, arg.clone()))
                .collect();
        let expected_args = function.takes_args();

        let mut draw_fns: Vec<Box<dyn Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> = vec![];

        for expected_arg in expected_args.into_iter() {
            if let Some(provided_arg) = provided_arg_by_definition_id.get(&expected_arg.id).clone()
            {
                let provided_arg = provided_arg.clone();
                draw_fns.push(Box::new(move |s: &CodeEditorRenderer<T>| {
                                  s.render_code(&CodeNode::Argument(provided_arg.clone()))
                              }))
            } else {
                draw_fns.push(Box::new(move |s: &CodeEditorRenderer<T>| {
                                  s.render_missing_function_argument(&expected_arg)
                              }))
            }
        }
        draw_fns
    }

    fn render_missing_function_argument(&self, _arg: &lang::ArgumentDefinition) -> T::DrawResult {
        self.draw_button("this shouldn't have happened, you've got a missing function arg somehow",
                         colorscheme!(danger_color),
                         &|| {})
    }

    fn render_function_call_arguments(
        &self,
        function_id: lang::ID,
        args: Vec<&lang::Argument>)
        -> Vec<Box<dyn Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> {
        let function = self.env_genie
                           .find_function(function_id)
                           .map(|func| func.clone());
        let args = args.clone();
        match function {
            Some(function) => return self.render_args_for_found_function(&*function, args),
            None => return self.render_args_for_missing_function(args),
        }
    }

    fn render_args_for_missing_function(
        &self,
        _args: Vec<&lang::Argument>)
        -> Vec<Box<dyn Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> {
        vec![Box::new(|s: &CodeEditorRenderer<T>| s.ui_toolkit.draw_all(&[]))]
    }

    fn render_struct_literal_field_label(&self, field: &structs::StructField) -> T::DrawResult {
        let field_text = format!("{} {}",
                                 self.env_genie.get_symbol_for_type(&field.field_type),
                                 field.name);
        self.ui_toolkit.draw_buttony_text(&field_text, BLACK_COLOR)
    }

    fn render_struct_literal_field(&self,
                                   field: &structs::StructField,
                                   literal_field: &lang::StructLiteralField)
                                   -> T::DrawResult {
        self.ui_toolkit.draw_all_on_same_line(&[
            &|| {
                self.set_selected_on_click(&|| self.render_struct_literal_field_label(field),
                                           literal_field.id)
            },
            &|| {
                self.render_nested(&|| {
                        if self.is_editing(literal_field.id) {
                            self.render_insert_code_node()
                        } else {
                            self.render_code(&literal_field.expr)
                        }
                    })
            },
        ])
    }

    fn render_struct_literal_fields(
        &self,
        strukt: &'a structs::Struct,
        fields: impl Iterator<Item = &'a lang::StructLiteralField>)
        -> Vec<Box<dyn Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> {
        // TODO: should this map just go inside the struct????
        let struct_field_by_id = strukt.field_by_id();

        let mut to_draw: Vec<Box<dyn Fn(&CodeEditorRenderer<T>) -> T::DrawResult>> = vec![];
        for literal_field in fields {
            // this is where the bug is
            //
            // ^^ coming in and looking at this comment months later:::: WHAT BUG???
            let strukt_field = struct_field_by_id.get(&literal_field.struct_field_id)
                                                 .unwrap();
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
        let strukt = self.env_genie
                         .find_struct(struct_literal.struct_id)
                         .unwrap();

        if struct_literal.fields.is_empty() {
            return self.render_struct_identifier(&strukt);
        }
        let rhs = self.render_struct_literal_fields(&strukt, struct_literal.fields());
        let rhs: Vec<Box<dyn Fn() -> T::DrawResult>> =
            rhs.into_iter()
               .map(|draw_fn| {
                   let b: Box<dyn Fn() -> T::DrawResult> = Box::new(move || draw_fn(&self));
                   b
               })
               .collect_vec();
        self.ui_toolkit.align(
                              &|| {
                                  self.set_selected_on_click(
                    &|| self.render_struct_identifier(&strukt),
                    struct_literal.id)
                              },
                              &rhs.iter().map(|b| b.as_ref()).collect_vec(),
        )
    }

    fn render_list_index(&self, list_index: &lang::ListIndex) -> T::DrawResult {
        self.set_selected_on_click(&|| {
            self.draw_nested_borders_around(&|| {
                self.ui_toolkit.draw_all_on_same_line(&[
                    &|| self.render_without_nesting(&|| self.render_code(&list_index.list_expr)),
                    &|| self.render_without_nesting(&|| self.render_nested(&|| self.render_code(&list_index.index_expr))),
                ])
            })
        },
            list_index.id
        )
    }

    fn render_struct_field_get(&self, sfg: &lang::StructFieldGet) -> T::DrawResult {
        let struct_field = self.env_genie
                               .find_struct_field(sfg.struct_field_id)
                               .unwrap();

        self.set_selected_on_click(
                                   &|| {
                                       self.draw_nested_borders_around(&|| {
                                               self.ui_toolkit.draw_all_on_same_line(&[
                    &|| self.render_code(&sfg.struct_expr),
                    &|| {
                        self.render_nested(&|| {
                                self.render_name_with_type_definition(&struct_field.name,
                                                                      colorscheme!(cool_color),
                                                                      &struct_field.field_type)
                            })
                    },
                ])
                                           })
                                   },
                                   sfg.id,
        )
    }

    fn render_struct_identifier(&self, strukt: &structs::Struct) -> T::DrawResult {
        let typ = lang::Type::from_spec(strukt);
        self.render_name_with_type_definition(&strukt.name, colorscheme!(cool_color), &typ)
    }

    fn render_conditional(&self, conditional: &lang::Conditional) -> T::DrawResult {
        self.ui_toolkit.draw_all(&[
            &|| {
                self.ui_toolkit.draw_all_on_same_line(&[&|| {
                                                            self.draw_button("If",
                                                                             colorscheme!(action_color),
                                                                             &|| {})
                                                        },
                                                        &|| {
                                                            self.render_code(&conditional.condition)
                                                        }])
            },
            &|| self.render_indented(&|| self.render_code(&conditional.true_branch)),
        ])
    }

    fn render_match(&self, mach: &lang::Match) -> T::DrawResult {
        self.ui_toolkit.draw_all(&[
            &|| {
                self.ui_toolkit.draw_all_on_same_line(&[&|| {
                                                            self.draw_button("Match",
                                                                             colorscheme!(action_color),
                                                                             &|| {})
                                                        },
                                                        &|| {
                                                            self.render_code(&mach.match_expression)
                                                        }])
            },
            &|| {
                let type_and_enum_by_variant_id =
                    self.code_editor
                        .code_genie
                        .match_variant_by_variant_id(mach, self.env_genie);
                draw_all_iter!(
                               T::self.ui_toolkit,
                               mach.branch_by_variant_id
                                   .iter()
                                   .map(|(variant_id, branch)| {
                                       let match_variant =
                                           type_and_enum_by_variant_id.get(variant_id).unwrap();
                                       move || {
                                           self.render_indented(&|| {
                                                   self.ui_toolkit.draw_all(&[
                                &|| {
                                    self.ui_toolkit.draw_all_on_same_line(&[
                                        &|| {
                                            let type_symbol =
                                                self.env_genie
                                                    .get_symbol_for_type(&match_variant.typ);
                                            self.ui_toolkit
                                                .draw_button(&type_symbol, BLACK_COLOR, &|| {})
                                        },
                                        &|| {
                                            self.ui_toolkit
                                                .draw_button(&match_variant.enum_variant.name,
                                                             colorscheme!(variable_color),
                                                             &|| {})
                                        },
                                    ])
                                },
                                &|| self.render_indented(&|| self.render_code(branch)),
                            ])
                                               })
                                       }
                                   })
                )
            },
        ])
    }

    fn render_indented(&self, draw_fn: &dyn Fn() -> T::DrawResult) -> T::DrawResult {
        self.ui_toolkit.indent(PX_PER_INDENTATION_LEVEL, draw_fn)
    }

    fn render_string_literal(&self, string_literal: &lang::StringLiteral) -> T::DrawResult {
        self.set_selected_on_click(&|| {
                                       self.draw_buttony_text(&format!("\u{F10D} {} \u{F10E}",
                                                                       string_literal.value),
                                                              colorscheme!(literal_bg_color))
                                   },
                                   string_literal.id)
    }

    fn render_number_literal(&self, number_literal: &lang::NumberLiteral) -> T::DrawResult {
        // TODO: for now lettttttt's not implement the editor for number literals. i think we
        // don't need it just yet. the insert code menu can insert number literals. perhaps we
        // can implement an InsertionPoint::Replace(node_id) that will suffice for number literals
        self.set_selected_on_click(&|| {
                                       self.ui_toolkit
                                           .draw_buttony_text(&number_literal.value.to_string(),
                                                              colorscheme!(literal_bg_color))
                                   },
                                   number_literal.id)
    }

    fn draw_inline_editor(&self, code_node: &CodeNode) -> T::DrawResult {
        // this is kind of a mess. render_insert_code_node() does `focus` inside of
        // it. the other parts of the branch need to be wrapped in focus() but not
        // render_insert_code_node()
        match code_node {
            CodeNode::StringLiteral(string_literal) => {
                self.ui_toolkit.focused(&move || {
                                   let new_literal = string_literal.clone();
                                   self.draw_inline_text_editor(&string_literal.value,
                                                                move |new_value| {
                                                                    let mut sl =
                                                                        new_literal.clone();
                                                                    sl.value =
                                                                        new_value.to_string();
                                                                    CodeNode::StringLiteral(sl)
                                                                })
                               })
            }
            CodeNode::Assignment(assignment) => {
                self.ui_toolkit.focused(&|| {
                                   let a = assignment.clone();
                                   self.draw_inline_text_editor(&assignment.name, move |new_value| {
                                       let mut new_assignment = a.clone();
                                       new_assignment.name = new_value.to_string();
                                       CodeNode::Assignment(new_assignment)
                                   })
                               })
            }
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
                self.command_buffer
                    .borrow_mut()
                    .add_editor_command(|e| e.mark_as_not_editing());
                self.draw_button(&format!("Not possible to edit {:?}", code_node),
                                 colorscheme!(danger_color),
                                 &|| {})
            }
        }
    }

    fn draw_inline_text_editor<F: Fn(&str) -> CodeNode + 'static>(&self,
                                                                  initial_value: &str,
                                                                  new_node_fn: F)
                                                                  -> T::DrawResult {
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

    fn draw_button<F: Fn() + 'static>(&self,
                                      label: &str,
                                      color: Color,
                                      onclick: F)
                                      -> T::DrawResult {
        let onclick_rc = Rc::new(RefCell::new(onclick));
        self.draw_nested_borders_around(&|| {
                let onclick_rc = Rc::clone(&onclick_rc);
                self.ui_toolkit
                    .draw_button(label, color, move || onclick_rc.borrow()())
            })
    }

    fn draw_buttony_text(&self, label: &str, color: Color) -> T::DrawResult {
        self.draw_nested_borders_around(&|| self.ui_toolkit.draw_buttony_text(label, color))
    }

    #[allow(dead_code)]
    fn draw_small_button<F: Fn() + 'static>(&self,
                                            label: &str,
                                            color: Color,
                                            onclick: F)
                                            -> T::DrawResult {
        let onclick_rc = Rc::new(RefCell::new(onclick));
        self.draw_nested_borders_around(&|| {
                let onclick_rc = Rc::clone(&onclick_rc);
                self.ui_toolkit
                    .draw_small_button(label, color, move || onclick_rc.borrow()())
            })
    }

    fn draw_text(&self, text: &str) -> T::DrawResult {
        self.draw_nested_borders_around(&|| self.ui_toolkit.draw_text(text))
    }

    fn draw_text_input<F: Fn(&str) + 'static, D: Fn() + 'static>(&self,
                                                                 existing_value: &str,
                                                                 onchange: F,
                                                                 ondone: D)
                                                                 -> T::DrawResult {
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

    fn is_rendering_menu_atm(&self) -> bool {
        *self.is_rendering_menu.borrow()
    }

    fn set_selected_on_click(&self,
                             draw_fn: &dyn Fn() -> T::DrawResult,
                             code_node_id: lang::ID)
                             -> T::DrawResult {
        if self.is_rendering_menu_atm() {
            return draw_fn();
        }
        let cmd_buffer = Rc::clone(&self.command_buffer);
        self.ui_toolkit.buttonize(draw_fn, move || {
                           cmd_buffer.borrow_mut()
                                     .add_editor_command(move |editor| {
                                         editor.set_selected_node_id(Some(code_node_id))
                                     })
                       })
    }
}

struct PerEditorCommandBuffer {
    actual_command_buffer: Rc<RefCell<editor::CommandBuffer>>,
    editor_id: lang::ID,
}

impl PerEditorCommandBuffer {
    pub fn new(actual_command_buffer: Rc<RefCell<editor::CommandBuffer>>,
               editor_id: lang::ID)
               -> Self {
        Self { actual_command_buffer,
               editor_id }
    }

    pub fn add_editor_command<F: FnOnce(&mut code_editor::CodeEditor) + 'static>(&mut self, f: F) {
        let editor_id = self.editor_id;
        self.actual_command_buffer
            .borrow_mut()
            .add_controller_command(move |controller| {
                controller.get_editor_mut(editor_id).map(f);
            });

        // update the function that the code being edited belongs to
        self.actual_command_buffer
            .borrow_mut()
            .add_integrating_command(move |cont, interpreter, _, _| {
                let mut env = interpreter.env.borrow_mut();

                let editor = cont.get_editor_mut(editor_id).unwrap();
                let code = editor.get_code().clone();
                code_editor::update_code_in_env(editor.location, code, cont, &mut env)
            });
    }
}
