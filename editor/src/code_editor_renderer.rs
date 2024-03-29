use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use itertools::Itertools;

use super::code_editor;
use super::code_editor::InsertionPoint;
use super::editor;
use super::insert_code_menu::{InsertCodeMenu, InsertCodeMenuOption};
use super::ui_toolkit::{Color, UiToolkit};
use crate::code_editor::locals::{find_antecedent_for_variable_reference, VariableAntecedentPlace};
use crate::code_editor::related_to_assignment_id;
use crate::code_rendering::{
    darken, draw_nested_borders_around, render_enum_variant_identifier, render_list_literal_label,
    render_list_literal_position, render_list_literal_value, render_name_with_type_definition,
    render_null, render_struct_field, render_struct_field_label, render_struct_identifier,
    render_type_symbol, CONTROL_FLOW_GREY_COLOR,
};
use crate::colorscheme;
use crate::draw_all_iter;
use crate::editor::drag_drop::DragDropPayload;
use crate::editor::value_renderer::ValueRenderer;
use crate::editor::{CommandBuffer, Key, Keypress};
use crate::insert_code_menu::{CodeSearchParams, InsertCodeMenuOptionsGroup};
use crate::ui_toolkit::{
    ChildRegionFrameStyle, ChildRegionHeight, ChildRegionStyle, ChildRegionTopPadding,
    ChildRegionWidth, DrawFnRef,
};
use cs::env_genie::EnvGenie;
use cs::lang::{AnonymousFunction, CodeNode, FunctionRenderingStyle};
use cs::structs;
use cs::{env, lang};
use lazy_static::lazy_static;

pub const PLACEHOLDER_ICON: &str = "\u{f85d}";
lazy_static! {
    // static ref TRY_ICON: String = format!("\u{f321}");
    static ref TRY_ICON: String = format!("\u{f712}");
    // static ref OR_RETURN_ICON: String = format!("{}{}", ELSE_ICON, EARLY_RETURN_ICON);
    static ref OR_RETURN_ICON: String = format!("{}", ELSE_ICON);
}
const EARLY_RETURN_ICON: &str = "\u{f151}";
const ELSE_ICON: &str = "\u{f352}";
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

    pub fn render(&self, height: ChildRegionHeight) -> T::DrawResult {
        let code = self.code_editor.get_code();
        let cmd_buffer = Rc::clone(&self.command_buffer);
        let cmd_buffer2 = Rc::clone(&cmd_buffer);

        let style = ChildRegionStyle { height,
                                       width: ChildRegionWidth::All,
                                       frame_style: ChildRegionFrameStyle::Framed,
                                       top_padding: ChildRegionTopPadding::None };
        self.ui_toolkit
            .draw_child_region(colorscheme!(child_region_bg_color),
                               &|| {
                                   self.ui_toolkit
                                       .with_y_padding(0, &|| self.render_code(code))
                               },
                               style,
                               Some(&move || self.draw_right_click_menu()),
                               Some(move |keypress| {
                                   cmd_buffer.borrow_mut()
                                             .add_editor_and_interp_command(move |code_editor, interp| {
                                                 code_editor.handle_keypress(keypress, interp)
                                             })
                               }),
                               move || {
                                   cmd_buffer2.borrow_mut().set_selected_node_ids();
                               })
    }

    fn draw_right_click_menu(&self) -> T::DrawResult {
        self.ui_toolkit.draw_all(&[
            &|| {
                if self.code_editor.get_last_selected_node_id().is_some() {
                    let cmd_buffer1 = Rc::clone(&self.command_buffer);
                    self.ui_toolkit.draw_menu_item("Deselect code", move || {
                                       let cmd_buffer1 = Rc::clone(&cmd_buffer1);
                                       let mut cmd_buffer1 = cmd_buffer1.borrow_mut();
                                       cmd_buffer1.add_editor_command(move |code_editor| {
                                                      code_editor.deselect_selected_code();
                                                  })
                                   })
                } else {
                    self.ui_toolkit.draw_all(&[])
                }
            },
            &|| {
                let cmd_buffer1 = Rc::clone(&self.command_buffer);
                self.ui_toolkit.draw_menu_item("Insert code", move || {
                                   let cmd_buffer1 = Rc::clone(&cmd_buffer1);
                                   let mut cmd_buffer1 = cmd_buffer1.borrow_mut();
                                   cmd_buffer1.add_editor_command(move |code_editor| {
                                       code_editor.set_insertion_point_on_next_line_in_block();
                                   })
                               })
            },
            // &|| {
            //     let codes = get_code_from_clipboard();
            //     if codes.is_none() {
            //         return self.ui_toolkit.draw_all(&[]);
            //     }
            //     let codes = codes.unwrap();
            //     let cmd_buffer1 = Rc::clone(&self.command_buffer);
            //     self.ui_toolkit.draw_menu_item("Paste code", move || {
            //                        let cmd_buffer1 = Rc::clone(&cmd_buffer1);
            //                        let mut cmd_buffer1 = cmd_buffer1.borrow_mut();
            //         let codes = codes.clone();
            //                        cmd_buffer1.add_editor_command(move |code_editor| {
            //                                       code_editor.insert_code(codes.clone().into_iter(),
            //                                       code_editor.insertion_point_for_menu()
            //                                           .unwrap_or_else(|| InsertionPoint::BeginningOfBlock(todo!()))
            //                                       );
            //                                   })
            //                    })
            // },
        ])
    }

    fn is_insertion_pointer_immediately_before(&self, id: lang::ID) -> bool {
        let insertion_point = self.code_editor.insertion_point_for_menu();
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
        match self.code_editor.insertion_point_for_menu() {
            Some(InsertionPoint::After(code_node_id)) if code_node_id == id => true,
            _ => false,
        }
    }

    // used to color variables references/assignments if a variable is selected
    fn draw_related_to_selection_highlight(&self,
                                           draw: &dyn Fn() -> T::DrawResult)
                                           -> T::DrawResult {
        let mut overlay_color = colorscheme!(cool_color);
        overlay_color[3] /= 2.;
        self.ui_toolkit.draw_box_around(overlay_color, draw)
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
                                     .guess_type(assignment.expression.as_ref(), self.env_genie)
                                     .unwrap();
        self.render_assignment_specify_lhs(assignment, &|| {
                self.code_handle(&|| {
                                     self.render_name_with_type_definition(&assignment.name,
                                                                       colorscheme!(variable_color),
                                                                       &type_of_assignment)
                                 },
                                 "assignment",
                                 assignment.id)
            })
    }

    fn render_reassignment(&self, reassignment: &lang::Reassignment) -> T::DrawResult {
        let type_of_assignment = self.code_editor
                                     .code_genie
                                     .guess_type(reassignment.expression.as_ref(), self.env_genie)
                                     .unwrap();
        let assignment_name = self.code_editor
                                  .code_genie
                                  .find_node(reassignment.assignment_id)
                                  .and_then(|code_node| code_node.as_assignment().ok())
                                  .map(|assignment| assignment.name.clone())
                                  .unwrap_or_else(|| "couldn't find it".into());
        let render_reassignment_name = &|| {
            self.code_handle(&|| {
                                 self.render_name_with_type_definition(&assignment_name,
                                                                       colorscheme!(variable_color),
                                                                       &type_of_assignment)
                             },
                             "reassignment",
                             reassignment.id)
        };
        self.ui_toolkit
            .draw_all_on_same_line(&[render_reassignment_name,
                                     &|| self.draw_text("   \u{f30a}   "),
                                     &|| self.render_code(reassignment.expression.as_ref())])
    }

    fn render_reassign_list_index(&self, rli: &lang::ReassignListIndex) -> T::DrawResult {
        // copy+pasted from render_reassignment
        let type_of_assignment = self.code_editor
                                     .code_genie
                                     .guess_type(rli.set_to_expr.as_ref(), self.env_genie)
                                     .unwrap();
        let assignment = self.code_editor
                             .code_genie
                             .find_node(rli.assignment_id)
                             .unwrap()
                             .as_assignment()
                             .unwrap();
        let render_variable_name = &|| {
            self.render_name_with_type_definition(&assignment.name,
                                                  colorscheme!(variable_color),
                                                  &type_of_assignment)
        };
        // copy+pasted from render_list_index
        let render_lvalue = &|| {
            self.code_handle(&|| {
            self.draw_nested_borders_around(&|| {
                self.ui_toolkit.draw_all_on_same_line(&[
                    &|| self.render_without_nesting(render_variable_name),
                    &|| self.render_without_nesting(&|| self.render_nested(&|| self.render_code(&rli.index_expr))),
                ])
            })
        },
                         "reassign_list_index", rli.id
        )
        };

        self.ui_toolkit.draw_all_on_same_line(&[render_lvalue,
                                                &|| self.draw_text("   \u{f30a}   "),
                                                &|| self.render_code(rli.set_to_expr.as_ref())])
    }

    fn render_assignment_specify_lhs(&self,
                                     assignment: &lang::Assignment,
                                     draw_lhs_func: DrawFnRef<T>)
                                     -> T::DrawResult {
        self.ui_toolkit.draw_all_on_same_line(&[draw_lhs_func,
                                                &|| self.draw_text("   \u{f52c}   "),
                                                &|| {
                                                    self.render_code(assignment.expression.as_ref())
                                                }])
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
                                                                         self.env_genie, self.code_editor.location.unwrap());

            let cmd_buffer = Rc::clone(&self.command_buffer);
                           self.draw_text_input(
                               menu.input_str(),
                               false,
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
                            editor.insert_code_and_set_where_cursor_ends_up_next(vec![new_code_node.clone()],
                                                                                 insertion_point);
                        });
                                                    } else {
                                                        cmdb.add_editor_command(|editor| {
                                                                editor.undo();
                                                                editor.hide_insert_code_menu();
                                                            });
                                                    }
                                                }, move |key| {
                                   match key {
                                      Keypress {
                                          key: Key::Escape, ..
                                      }  => {
                                          cmd_buffer.borrow_mut().add_editor_command(|editor| {
                                              editor.escape_out_of_autocomplete_and_undo();
                                          })
                                      },
                                       Keypress {
                                           key: Key::Tab, shift: false, ..
                                       } | Keypress { key: Key::DownArrow, .. } => {
                                           cmd_buffer.borrow_mut().add_editor_command(|editor| {
                                               editor.insert_code_menu
                                                   .as_mut()
                                                   .map(|menu| menu.select_next());
                                           })
                                       },
                                       Keypress {
                                           key: Key::Tab, shift: true, ..
                                       } | Keypress { key: Key::UpArrow, .. } => {
                                           cmd_buffer.borrow_mut().add_editor_command(|editor| {
                                               editor.insert_code_menu
                                                   .as_mut()
                                                   .map(|menu| menu.select_prev());
                                           })
                                       }
                                       _ => (),
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
                                                      self.ui_toolkit
                                                          .draw_taking_up_full_width(&|| {
                                                              self.render_insertion_header(menu)
                                                          })
                                                  })
                               })
            },
            &|| {
                let style = ChildRegionStyle {
                    height: ChildRegionHeight::Pixels(300),
                    width: ChildRegionWidth::All,
                    frame_style: ChildRegionFrameStyle::Framed,
                    top_padding: ChildRegionTopPadding::Default
                };
                self.ui_toolkit.draw_child_region(transparent_black,
                                                  &|| {
                                                      let options_groups =
                                                          menu.grouped_options(&self.code_editor
                                                                                    .code_genie,
                                                                               self.env_genie, self.code_editor.location.unwrap());
                                                      self.ui_toolkit
                                                          .with_y_padding(2, &|| {
                                                              draw_all_iter!(T::self.ui_toolkit, options_groups.iter().map(|group| {
                                                                  move || self.render_insertion_options_group(group, menu.insertion_point)
                                                              }))
                                                          })
                                                  },
                                                  style,
                                                  None::<&dyn Fn() -> T::DrawResult>,
                                                  None::<fn(Keypress)>,
                                                  || ())
            },
        ])
    }

    fn render_insertion_header(&self, menu: &InsertCodeMenu) -> T::DrawResult {
        // TODO: show the type of the thing being inserted
        // show hints: like if you want a list of something, type list
        // or if the type is number, then tell the user they can start typing numbers
        // or even better, maybe have clickable numbers? idk how that would work tho
        let search_params = menu.search_params(&self.code_editor.code_genie,
                                               self.env_genie,
                                               self.code_editor.location.unwrap());
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
            | InsertionPoint::After(_) => self.draw_operation_label("\u{f0fe}", "Insert code"),
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
                    &|| self.draw_operation_label("\u{f055}", "Insert struct field"),
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
                self.ui_toolkit.draw_all_on_same_line(&[&|| {
                                                            self.draw_operation_label("\u{f14b}",
                                                                                      "Edit")
                                                        },
                                                        &|| self.render_code(code_node)])
            }
            InsertionPoint::ListLiteralElement { list_literal_id,
                                                 pos, } => {
                let code_node = self.code_editor
                                    .code_genie
                                    .find_node(list_literal_id)
                                    .unwrap();
                self.ui_toolkit.draw_all_on_same_line(&[
                    &|| self.draw_operation_label("\u{f055}", "Insert into list"),
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
                    .draw_all_on_same_line(&[&|| self.draw_operation_label("\u{f0ec}", "Replace"), &|| {
                                               self.render_code(code_node_being_replaced)
                                           }])
            }
            InsertionPoint::Wrap(code_node_being_wrapped_id) => {
                let code_node_being_wrapped = self.code_editor
                                                  .code_genie
                                                  .find_node(code_node_being_wrapped_id)
                                                  .unwrap();
                self.ui_toolkit
                    .draw_all_on_same_line(&[
                        &|| self.draw_operation_label("\u{f79c}", "Wrap around"),
                        &|| self.render_code(code_node_being_wrapped)
                    ])
            }
            InsertionPoint::Unwrap(code_node_being_unwrapped_id) => {
                let code_node_being_unwrapped = self.code_editor
                                                    .code_genie
                                                    .find_node(code_node_being_unwrapped_id)
                                                    .unwrap();
                self.ui_toolkit
                    .draw_all_on_same_line(&[
                        &|| self.draw_operation_label("\u{f49e}", "Unwrap out of"),
                        &|| self.render_code(code_node_being_unwrapped)
                    ])
            }
        }
    }

    fn draw_operation_label(&self, symbol: &str, text: &str) -> T::DrawResult {
        self.ui_toolkit.draw_all_on_same_line(&[
            &|| {
                self.ui_toolkit
                    .draw_buttony_text(symbol, darken(colorscheme!(cool_color)))
            },
            &|| {
                self.ui_toolkit
                    .draw_buttony_text(text, colorscheme!(cool_color))
            },
        ])
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
            // println!("rendering insertion option {:?}", option);
            self.ui_toolkit.buttonize(
                                      &|| {
                                          self.ui_toolkit.draw_taking_up_full_width(&|| {
                      match &self.help_text(&option.new_node) {
                          Some(help_text) if !help_text.is_empty() => {
                              self.ui_toolkit.draw_all(&[
                                    &|| self.render_code_for_insertion_menu_preview(option.new_node.clone(), insertion_point),
                                    &|| self.ui_toolkit.draw_wrapped_text(darken(colorscheme!(text_color)), &help_text),
                              ])
                          }
                          _ => self.render_code_for_insertion_menu_preview(option.new_node.clone(), insertion_point),
                      }
                  })
                                      },
                                      move || {
                                          let ncn = new_code_node.clone();
                                          cmdb.borrow_mut().add_editor_command(move |editor| {
                                                               editor.hide_insert_code_menu();
                                                               editor.insert_code_and_set_where_cursor_ends_up_next(vec![ncn.clone()],
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

    fn render_code_for_insertion_menu_preview(&self,
                                              new_node: lang::CodeNode,
                                              insertion_point: InsertionPoint)
                                              -> T::DrawResult {
        let new_editor = self.code_editor
                             .for_insert_code_preview(new_node.clone(), insertion_point);
        let command_buffer_that_does_nothing = Rc::new(RefCell::new(CommandBuffer::new()));
        let new_renderer = CodeEditorRenderer::new(self.ui_toolkit,
                                                   &new_editor,
                                                   command_buffer_that_does_nothing,
                                                   self.env_genie);
        new_renderer.render_code(&new_node)
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
                    .guess_type(code_node, self.env_genie)
                    .unwrap();
        render_list_literal_label(self.ui_toolkit, self.env_genie, &t)
    }

    fn render_list_literal_position(&self, pos: usize) -> T::DrawResult {
        render_list_literal_position(self.ui_toolkit, pos)
    }

    fn render_map_literal(&self, map_literal: &lang::MapLiteral) -> T::DrawResult {
        // renders just the handle for now... we eventually need a way to put things in here
        let typ = lang::Type::map(map_literal.from_type.clone(), map_literal.to_type.clone());
        self.code_handle(&|| render_list_literal_label(self.ui_toolkit, self.env_genie, &typ),
                         "map literal handle label",
                         map_literal.id)
    }

    fn render_list_literal(&self,
                           list_literal: &lang::ListLiteral,
                           code_node: &lang::CodeNode)
                           -> T::DrawResult {
        let lhs = &|| {
            self.code_handle(&|| self.render_list_literal_label(code_node),
                             "list_literal_label",
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
                             render_list_literal_value(self.ui_toolkit, position_label, &|| {
                                 self.render_nested(&|| self.render_insert_code_node())
                             })
                         }));
                position_label += 1;
            }

            list_literal.elements.get(i).map(|el| {
                                            rhs.push(Box::new(move || {
                    render_list_literal_value(self.ui_toolkit, position_label, &|| {
                        self.render_nested(&|| self.render_code(el))
                    })
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
        draw_nested_borders_around(self.ui_toolkit, draw_element_fn, nesting_level as u8)
    }

    fn is_part_of_selection(&self, code_node_id: lang::ID) -> bool {
        self.code_editor.selected_node_ids.contains(&code_node_id)
    }

    fn is_selected_for_editing(&self, code_node_id: lang::ID) -> bool {
        Some(code_node_id) == self.code_editor.get_last_selected_node_id()
    }

    fn is_editing(&self, code_node_id: lang::ID) -> bool {
        self.is_selected_for_editing(code_node_id) && self.code_editor.editing
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
            CodeNode::Reassignment(_) => Some("Change variable to a new value".to_string()),
            CodeNode::Block(_) => None,
            CodeNode::VariableReference(vr) => {
                let (_name, typ) = self.lookup_variable_reference(vr)?;
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
            CodeNode::MapLiteral(map_literal) => {
                let from_name = self.env_genie.get_name_for_type(&map_literal.from_type)
                    .expect("wtf we couldn't find the type?");
                let to_name = self.env_genie.get_name_for_type(&map_literal.to_type)
                    .expect("wtf we couldn't find the type?");
                Some(format!("A new mapping from {} to {}", from_name, to_name))
            },
            CodeNode::StructFieldGet(sfg) => {
                let struct_field = self.env_genie.find_struct_field(sfg.struct_field_id)?;
                Some(struct_field.description.clone())
            }
            CodeNode::NumberLiteral(_) => Some(lang::NUMBER_TYPESPEC.description.clone()),
            CodeNode::ListIndex(_) => None,
            CodeNode::AnonymousFunction(_) => {
                Some("Executable code that can be passed around like data, and executed later. Sometimes referred to as a \"callback\"".into())
            }
            CodeNode::ReassignListIndex(_) => {
                Some("Change one element in a list".into())
            }
            CodeNode::WhileLoop(_) => {
                Some("Run code again and again until the condition is false".into())
            }
            CodeNode::ForLoop(_) => {
               Some("Do something with each element of the list".into()) 
            }
            CodeNode::EnumVariantLiteral(evl) => {
                let (eneom, variant) = self.env_genie.find_enum_variant(evl.variant_id).unwrap();
                Some(format!("A variant of {}: {}", eneom.name, variant.name))
            }
            CodeNode::EarlyReturn(_) => {
                Some("Exit the function early".into())
            }
            CodeNode::Try(_) => {
                Some("Try, or fallback if there's an error".into())
            }
        }
    }

    fn render_code(&self, code_node: &CodeNode) -> T::DrawResult {
        // TODO: lots of is_rendering_menu_atm() checks... how can we clean it up?
        if self.is_editing(code_node.id()) && !self.is_rendering_menu_atm() {
            return self.draw_inline_editor(code_node);
        }

        match self.insertion_point() {
            Some(InsertionPoint::Replace(id))
            | Some(InsertionPoint::Wrap(id))
            | Some(InsertionPoint::Unwrap(id))
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
                CodeNode::Reassignment(reassignment) => self.render_reassignment(&reassignment),
                CodeNode::ReassignListIndex(reassign_list_index) => {
                    self.render_reassign_list_index(reassign_list_index)
                }
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
                CodeNode::MapLiteral(map_literal) => self.render_map_literal(&map_literal),
                CodeNode::StructFieldGet(sfg) => self.render_struct_field_get(&sfg),
                CodeNode::ListIndex(list_index) => self.render_list_index(&list_index),
                CodeNode::AnonymousFunction(anon_func) => self.render_anonymous_function(anon_func),
                CodeNode::WhileLoop(while_loop) => self.render_while_loop(while_loop),
                CodeNode::EnumVariantLiteral(evl) => {
                    let typ = self.code_editor
                                  .code_genie
                                  .guess_type(code_node, self.env_genie)
                                  .unwrap();
                    self.render_enum_variant_literal(evl, &typ)
                }
                CodeNode::EarlyReturn(early_return) => self.render_early_return(early_return),
                CodeNode::Try(trai) => self.render_try(trai),
                CodeNode::ForLoop(for_loop) => self.render_for_loop(for_loop),
            }
        };

        let draw = || {
            if let Some(focused_assignment_id) = self.code_editor.focused_assignment_id() {
                if Some(focused_assignment_id) == related_to_assignment_id(code_node)
                   && !self.is_part_of_selection(code_node.id())
                {
                    return self.draw_related_to_selection_highlight(&draw);
                }
            }
            draw()
        };

        let draw = || self.render_context_menu(code_node, &draw);

        let draw_fn = &|| {
            if self.is_part_of_selection(code_node.id()) {
                self.draw_selected(self.code_node_cursor_scroll_hash(code_node), &draw)
            } else {
                self.draw_code_node_and_insertion_point_if_before_or_after(code_node, &draw)
            }
        };

        // this is good for debugging
        // self.ui_toolkit
        //     .draw_all_on_same_line(&[draw_fn, &|| {
        //                                self.ui_toolkit.draw_text(&format!("{:?}", code_node.id()))
        //                            }])
        draw_fn()
    }

    pub fn render_try(&self, trai: &lang::Try) -> T::DrawResult {
        let draw_fn = &|| {
            self.draw_nested_borders_around(&|| {
                    self.ui_toolkit.draw_all_on_same_line(&[
                    // &|| self.ui_toolkit.draw_all_on_same_line(&[
                    // &|| self.render_control_flow_label(&TRY_ICON, "Try"),
                    &|| {
                        self.code_handle(
                                         &|| {
                                             self.ui_toolkit.draw_all_on_same_line(&[
                                &|| {
                                    let code_node = lang::CodeNode::Try(trai.clone());
                                    let typ = self.code_editor
                                                  .code_genie
                                                  .guess_type(&code_node, self.env_genie)
                                                  .unwrap();
                                    let sym = self.env_genie.get_symbol_for_type(&typ);
                                    self.render_control_flow_symbol(&sym)
                                },
                                &|| self.render_control_flow_text(&TRY_ICON),
                            ])
                                         },
                                         "try_main_label",
                                         trai.id,
                        )
                    },
                    &|| self.render_code(&trai.maybe_error_expr),
                    // ]),
                    // &|| self.ui_toolkit.draw_all_on_same_line(&[
                    &|| {
                        self.code_handle(&|| self.render_control_flow_text(&OR_RETURN_ICON),
                                         "try_else_label",
                                         trai.id)
                        // self.ui_toolkit.draw_with_margin((6., 0.), &|| {
                        //                    self.ui_toolkit.draw_all_on_same_line(&[
                        //         &|| {
                        // self.ui_toolkit
                        //     .draw_buttony_text("\u{f106}", CONTROL_FLOW_GREY_COLOR)
                    },
                    &|| self.render_without_nesting(&|| self.render_code(&trai.or_else_expr)),
                    // ])
                    // &|| self.render_control_flow_label(&ELSE_ICON, "Or"),
                    // ])
                    //            })
                    // },
                ])
                })
        };
        // TODO: pretty sure i don't need this commented out code anymore since i'm nerfing Try to not
        // early return
        // if let Some(value) = self.env_genie
        //                          .get_last_executed_result(trai.or_else_expr.id())
        // {
        //     self.draw_code_with_output(draw_fn, value)
        // } else {
        draw_fn()
        // }
    }

    pub fn render_early_return(&self, early_return: &lang::EarlyReturn) -> T::DrawResult {
        self.ui_toolkit.draw_all_on_same_line(&[&|| {
                                                    self.code_handle(&|| {
                                                        self.render_control_flow_label(EARLY_RETURN_ICON,
                                                                                       "Return")
                                                        // self.ui_toolkit
                                                        //     .draw_buttony_text(&EARLY_RETURN_ICON, CONTROL_FLOW_GREY_COLOR)
                                                                     },
                                                                     "early_return_label", early_return.id)
                                                },
                                                &|| self.render_code(early_return.code.as_ref())])
    }

    pub fn render_anonymous_function(&self, anon_func: &AnonymousFunction) -> T::DrawResult {
        self.render_block_inside_child_region(&anon_func.block.as_ref().as_block().unwrap(),
                                              ChildRegionFrameStyle::Framed)
    }

    fn render_block_inside_child_region(&self,
                                        block: &lang::Block,
                                        frame_style: ChildRegionFrameStyle)
                                        -> T::DrawResult {
        let top_padding = match frame_style {
            ChildRegionFrameStyle::Framed => ChildRegionTopPadding::None,
            ChildRegionFrameStyle::NoFrame => ChildRegionTopPadding::Default,
        };
        let style = ChildRegionStyle { height: ChildRegionHeight::FitContent,
                                       width: ChildRegionWidth::All,
                                       frame_style,
                                       top_padding };
        self.ui_toolkit
            .draw_child_region(colorscheme!(child_region_bg_color),
                               &|| self.render_block(&block),
                               style,
                               Some(&|| self.draw_right_click_menu()),
                               None::<fn(Keypress)>,
                               || ())
    }

    pub fn render_context_menu(&self,
                               code_node: &CodeNode,
                               draw_code_fn: DrawFnRef<T>)
                               -> T::DrawResult {
        let code_node_id = code_node.id();
        match code_node {
            CodeNode::FunctionReference(_) => {
                let parent = self.code_editor
                                 .code_genie
                                 .find_parent(code_node_id)
                                 .unwrap();
                // just want to make sure this is a function reference
                parent.as_function_call().unwrap();
                self.render_general_code_context_menu(draw_code_fn, parent.id())
            }
            CodeNode::StringLiteral(_)
            | CodeNode::NullLiteral(_)
            | CodeNode::Assignment(_)
            | CodeNode::Reassignment(_)
            | CodeNode::VariableReference(_)
            | CodeNode::Placeholder(_)
            | CodeNode::StructLiteral(_)
            | CodeNode::StructLiteralField(_)
            | CodeNode::Conditional(_)
            | CodeNode::WhileLoop(_)
            | CodeNode::Match(_)
            | CodeNode::ListLiteral(_)
            | CodeNode::MapLiteral(_)
            | CodeNode::StructFieldGet(_)
            | CodeNode::NumberLiteral(_)
            | CodeNode::ListIndex(_)
            | CodeNode::EnumVariantLiteral(_)
            | CodeNode::EarlyReturn(_)
            | CodeNode::Try(_)
            | CodeNode::ForLoop(_) => {
                self.render_general_code_context_menu(draw_code_fn, code_node_id)
            }
            CodeNode::FunctionCall(_)
            | CodeNode::Block(_)
            | CodeNode::Argument(_)
            | CodeNode::ReassignListIndex(_)
            | CodeNode::AnonymousFunction(_) => draw_code_fn(),
        }
    }

    fn render_general_code_context_menu(&self,
                                        draw_code_fn: DrawFnRef<T>,
                                        code_node_id_to_act_on: lang::ID)
                                        -> <T as UiToolkit>::DrawResult {
        self.ui_toolkit.context_menu(draw_code_fn, &|| {
                           self.ui_toolkit.draw_all(&[
                &|| {
                    if self.code_editor.can_be_replaced(code_node_id_to_act_on) {
                        let cmd_buffer = Rc::clone(&self.command_buffer);
                        self.ui_toolkit.draw_menu_item("Replace", move || {
                            cmd_buffer.borrow_mut().add_editor_command(move |editor| {
                                editor.enter_replace_for_node(code_node_id_to_act_on);
                            })
                        })
                    } else {
                        self.ui_toolkit.draw_all(&[])
                    }
                },
                &|| {
                    let code_node = self.code_editor
                                        .code_genie
                                        .find_node(code_node_id_to_act_on)
                                        .unwrap();
                    if self.code_editor.can_be_edited(code_node) {
                        let cmd_buffer = Rc::clone(&self.command_buffer);
                        self.ui_toolkit.draw_menu_item(self.code_editor.edit_menu_text(code_node), move || {
                            cmd_buffer.borrow_mut().add_editor_command(move |editor| {
                                editor.mark_as_editing(InsertionPoint::Editing(code_node_id_to_act_on));
                            })
                        })
                    } else {
                        self.ui_toolkit.draw_all(&[])
                    }
                },
                &|| {
                    let cmd_buffer = Rc::clone(&self.command_buffer);
                    self.ui_toolkit
                        .draw_menu_item("Extract into variable", move || {
                            cmd_buffer.borrow_mut().add_editor_command(move |editor| {
                                editor.extract_into_variable(code_node_id_to_act_on);
                            })
                        })
                },
                &|| {
                    let cmd_buffer = Rc::clone(&self.command_buffer);
                    self.ui_toolkit.draw_menu_item("Wrap with...", move || {
                                       cmd_buffer.borrow_mut().add_editor_command(move |editor| {
                                editor.enter_wrap_for_node(code_node_id_to_act_on);
                            })
                                   })
                },
                &|| {
                    if self.code_editor.can_be_deleted(code_node_id_to_act_on) {
                        let cmd_buffer = Rc::clone(&self.command_buffer);
                        self.ui_toolkit.draw_menu_item("Delete", move || {
                            cmd_buffer.borrow_mut()
                                .add_editor_command(move |editor| {
                                    editor.delete_node_ids(std::iter::once(code_node_id_to_act_on));
                                })
                        })
                    } else {
                        self.ui_toolkit.draw_all(&[])
                    }
                },
                               &|| {
                                   if self.code_editor.can_be_unwrapped(code_node_id_to_act_on) {
                                       let cmd_buffer = Rc::clone(&self.command_buffer);
                                       self.ui_toolkit.draw_menu_item("Unwrap...", move || {
                                           cmd_buffer.borrow_mut().add_editor_command(move |editor| {
                                               editor.enter_wrap_for_node(code_node_id_to_act_on);
                                           })
                                       })
                                   } else {
                                       self.ui_toolkit.draw_all(&[])
                                   }
                               }
                           ])
                       })
    }

    // this identifies the current selection to the UI, so it can remember the scroll of the current
    // item
    fn code_node_cursor_scroll_hash(&self, code_node: &lang::CodeNode) -> String {
        format!("{:?}:{}", self.code_editor.location, code_node.id())
    }

    fn render_null_literal(&self, null_literal_id: &lang::ID) -> T::DrawResult {
        self.code_handle(&|| self.draw_nested_borders_around(&|| render_null(self.ui_toolkit)),
                         "null_literal_label",
                         *null_literal_id)
    }

    fn render_placeholder(&self, placeholder: &lang::Placeholder) -> T::DrawResult {
        self.code_handle(
                         &|| {
                             self.ui_toolkit.draw_all_on_same_line(&[
                &|| {
                    let sym = self.env_genie.get_symbol_for_type(&placeholder.typ);
                    self.ui_toolkit
                        .draw_buttony_text(&sym, darken(colorscheme!(warning_color)))
                },
                &|| {
                    self.ui_toolkit
                        .draw_buttony_text(&format!("{} {}",
                                                    PLACEHOLDER_ICON, placeholder.description),
                                           colorscheme!(warning_color))
                },
            ])
                         },
                         "placeholder_main_handle",
                         placeholder.id,
        )
    }

    fn render_function_reference(&self,
                                 function_reference: &lang::FunctionReference)
                                 -> T::DrawResult {
        let function_call = self.code_editor
                                .code_genie
                                .find_parent(function_reference.id)
                                .unwrap();

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
        match func.style() {
            FunctionRenderingStyle::Default => {
                let func_call_typ = self.code_editor
                                        .code_genie
                                        .guess_type(function_call, self.env_genie)
                                        .unwrap();
                self.render_function_name(&func.name(), colorscheme!(action_color), &func_call_typ)
            }
            FunctionRenderingStyle::Infix(_, infix_symbol) => {
                self.ui_toolkit.draw_text(&format!(" {} ", infix_symbol))
                // self.render_function_name("", colorscheme!(action_color), &func.returns())
            }
        }
    }

    fn render_variable_reference(&self,
                                 variable_reference: &lang::VariableReference)
                                 -> T::DrawResult {
        let draw = &|| {
            if let Some((name, typ)) = self.lookup_variable_reference(variable_reference) {
                self.render_variable_appearance(&name, &typ)
            } else {
                self.draw_button("Variable reference not found",
                                 colorscheme!(danger_color),
                                 &|| {})
            }
        };
        self.code_handle(draw, "variable_reference_label", variable_reference.id)
    }

    fn render_variable_appearance(&self, name: &str, typ: &lang::Type) -> T::DrawResult {
        self.render_name_with_type_definition(name, colorscheme!(variable_color), typ)
    }

    fn render_function_name(&self, name: &str, color: Color, typ: &lang::Type) -> T::DrawResult {
        let sym = self.env_genie.get_symbol_for_type(typ);
        let darker_color = darken(darken(color));

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
                                    &|| self.ui_toolkit.draw_buttony_text(&format!("\u{f152} {}", sym), darker_color),
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
        self.draw_nested_borders_around(&|| {
                render_name_with_type_definition(self.ui_toolkit, self.env_genie, name, color, typ)
            })
    }

    // should we move this into the genie?
    fn lookup_variable_reference(&self,
                                 variable_reference: &lang::VariableReference)
                                 -> Option<(String, lang::Type)> {
        let antecedent = find_antecedent_for_variable_reference(variable_reference,
                                                                false,
                                                                &self.code_editor.code_genie,
                                                                self.env_genie)?;
        let typ = self.code_editor
                      .code_genie
                      .guess_type_for_variable(antecedent.place, self.env_genie)
                      .unwrap();
        Some((antecedent.name, typ))
    }

    fn render_for_loop_variable(&self, for_loop: &lang::ForLoop) -> T::DrawResult {
        let typ = self.code_editor
                      .code_genie
                      .guess_type_for_variable(VariableAntecedentPlace::ForLoop { for_loop_id:
                                                                                      for_loop.id },
                                               self.env_genie);
        if let Ok(typ) = typ {
            self.render_variable_appearance(&for_loop.variable_name, &typ)
        } else {
            self.ui_toolkit
                .draw_text("Unable to find type for for loop variable")
        }
    }

    // TODO: combine the insertion point stuff with the insertion point stuff elsewhere, mainly
    // the is_insertion_point_before_or_after stuff
    fn render_block(&self, block: &lang::Block) -> T::DrawResult {
        // TODO: i think i could move the is_insertion_point_before_or_after crapola to here
        let not_inserting_code = self.insertion_point().is_none();

        self.ui_toolkit.draw_all(&[
            &|| match self.code_editor.insertion_point_for_menu() {
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
                        self.render_code_insertion_menu_here_if_it_was_requested()
                    }
                } else {
                    self.ui_toolkit.draw_all(&[
                        &|| if not_inserting_code {
                            let first_code_id = block.expressions.first().unwrap().id();
                            self.render_add_code_here_line(InsertionPoint::Before(first_code_id), false)
                        } else {
                            self.ui_toolkit.draw_all(&[])
                        },
                        &|| {
                            let len = block.expressions.len();
                            draw_all_iter!(T::self.ui_toolkit,
                               block.expressions.iter().enumerate().map(|(i, code)| {
                                let is_last = i == len - 1;
                                   move || {
                                       if not_inserting_code {
                                           self.ui_toolkit.draw_all(&[
                                                &|| self.render_code_line_in_block(code),
                                                &|| self.render_add_code_here_line(InsertionPoint::After(code.id()), is_last),
                                           ])
                                       } else {
                                           self.ui_toolkit.draw_all(&[
                                              &|| self.render_code_insertion_menu_here_if_it_was_requested(),
                                              &|| self.render_code_line_in_block(code),
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

    fn render_code_line_in_block(&self, code_node: &lang::CodeNode) -> T::DrawResult {
        let draw_code_fn = &|| self.render_code(code_node);
        let draw_code_with_output_if_present = &|| {
            self.env_genie
                .get_last_executed_result(code_node.id(), |result| {
                    if let Some(result) = result {
                        self.draw_code_with_output(draw_code_fn, result)
                    } else {
                        draw_code_fn()
                    }
                })
        };

        let cmd_buffer = Rc::clone(&self.command_buffer);
        let insertion_point = InsertionPoint::After(code_node.id());
        let target_editor_id = self.code_editor.id();
        self.ui_toolkit
            .drag_drop_target(draw_code_with_output_if_present,
                              &|| {
                                  self.ui_toolkit.draw_all(&[draw_code_fn, &|| {
                                                               self.ui_toolkit.draw_empty_line()
                                                           }])
                              },
                              move |payload: DragDropPayload| {
                                  cmd_buffer.borrow_mut().handle_drag_drop(payload,
                                                                           insertion_point,
                                                                           target_editor_id);
                              })
    }

    fn draw_code_with_output(&self,
                             draw_code_fn: DrawFnRef<T>,
                             result: &env::EvaluationDebugResult)
                             -> T::DrawResult {
        // XXX: really messy :(((( fix this
        // the problem is that we're doing an if check here to see if we should show output, but
        // the places that are calling this are also doing their own if lets to see if there's any
        // output, this should be consolidated, but i'm in a rush right now
        if !self.code_editor.show_output {
            return draw_code_fn();
        }

        let style = ChildRegionStyle { height: ChildRegionHeight::Max(50),
                                       width: ChildRegionWidth::FitContent,
                                       frame_style: ChildRegionFrameStyle::NoFrame,
                                       top_padding: ChildRegionTopPadding::Default };
        self.ui_toolkit
            .draw_all(&[draw_code_fn, &|| {
                self.ui_toolkit.draw_child_region([0., 0., 0., 0.2], &|| {
                    self.ui_toolkit.draw_box_around([0., 0., 0., 0.2], &|| {
                        self.ui_toolkit.align(&|| self.ui_toolkit.draw_text("Last result       "), &[&|| {
                            ValueRenderer::new(&self.env_genie.env, self.ui_toolkit).render(&result.last_value)
                        },
                            &|| self.ui_toolkit.draw_text(&format!("{:?}", result.time_elapsed))
                        ])
                    })
                }, style,
                                                  None::<DrawFnRef<T>>,
                                                  None::<fn(Keypress)>, || (),
                )
            }])
    }

    fn render_add_code_here_line(&self,
                                 insertion_point: InsertionPoint,
                                 #[allow(unused)] is_last: bool)
                                 -> T::DrawResult {
        // let height = if is_last { 50. } else { 6. };
        let height = 6.;
        let target_editor_id = self.code_editor.id();
        self.ui_toolkit.draw_with_no_spacing_afterwards(&|| {
            let cmd_buffer = Rc::clone(&self.command_buffer);
            self.ui_toolkit.drag_drop_target(
                    &|| {
                        self.ui_toolkit.replace_on_hover(&|| {
                            self.ui_toolkit
                                // HAX: 70 is the size of the Insert Code button lol
                                .draw_code_line_separator( 70., height)
                        },
                                                         &|| self.render_add_code_here_button(insertion_point))
                    },
                    &|| self.ui_toolkit.draw_empty_line(),
                    move |payload: DragDropPayload| {
                        cmd_buffer.borrow_mut().handle_drag_drop(payload, insertion_point, target_editor_id)
                    },
                )
            })
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
            return self.code_handle(&|| {
                                        self.render_code(function_call.function_reference.as_ref())
                                    },
                                    "func_call_main_label",
                                    function_call.id);
        }

        let function = self.env_genie
                           .find_function(function_call.function_reference().function_id)
                           .map(|func| func.clone())
                           .unwrap();
        match function.style() {
            FunctionRenderingStyle::Default => {
                self.render_default_function_call_style(&function_call)
            }
            FunctionRenderingStyle::Infix(_, _) => {
                self.render_infix_function_call_style(&function_call)
            }
        }
    }

    fn render_infix_function_call_style(&self,
                                        function_call: &lang::FunctionCall)
                                        -> T::DrawResult {
        self.render_nested(&|| {
                self.draw_nested_borders_around(&|| {
                        self.ui_toolkit.draw_all_on_same_line(&[
                    &|| self.render_without_nesting(&|| self.render_code(&function_call.args[0])),
                    &|| {
                        self.code_handle(&|| {
                                             self.render_code(function_call.function_reference
                                                                           .as_ref())
                                         },
                                         "func_call_infix_label",
                                         function_call.id)

                        // self.code_handle(&|| self.ui_toolkit.draw_text(infix_symbol),
                        //                  function_call.id)
                    },
                    &|| self.render_without_nesting(&|| self.render_code(&function_call.args[1])),
                ])
                    })
            })
    }

    fn render_default_function_call_style(&self,
                                          function_call: &lang::FunctionCall)
                                          -> <T as UiToolkit>::DrawResult {
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
                                  self.code_handle(
                    &|| self.render_code(&function_call.function_reference),
                    "func_call_main_label", function_call.id)
                              },
                              &rhs.iter().map(|b| b.as_ref()).collect_vec(),
        )
    }

    fn render_function_call_argument(&self, argument: &lang::Argument) -> T::DrawResult {
        // println!("rendering function call arg (and guessing type for: {:?}",
        //          argument);
        let func = self.env_genie
                       .get_function_containing_arg(argument.argument_definition_id)
                       .unwrap();

        let render_inner_code = &|| self.render_code(argument.expr.as_ref());

        match func.style() {
            FunctionRenderingStyle::Default => {
                let arg_display = {
                    match self.env_genie
                              .get_arg_definition(argument.argument_definition_id)
                    {
                        Some((_func, arg_def)) => {
                            let typ = self.code_editor
                                          .code_genie
                                          .guess_type(&lang::CodeNode::Argument(argument.clone()),
                                                      self.env_genie)
                                          .unwrap();
                            let type_symbol = self.env_genie.get_symbol_for_type(&typ);
                            format!("{} {}", type_symbol, arg_def.short_name)
                        }
                        None => "\u{f059}".to_string(),
                    }
                };

                self.render_nested(&|| {
                        self.ui_toolkit.draw_all_on_same_line(&[
                        &|| self.ui_toolkit.draw_buttony_text(&arg_display, BLACK_COLOR),
                        render_inner_code,
                    ])
                    })
            }
            FunctionRenderingStyle::Infix(_, _) => render_inner_code(),
        }
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
        render_struct_field_label(self.ui_toolkit, self.env_genie, field)
    }

    fn render_struct_literal_field(&self,
                                   field: &structs::StructField,
                                   literal_field: &lang::StructLiteralField)
                                   -> T::DrawResult {
        render_struct_field(self.ui_toolkit,
                            &|| {
                                self.code_handle(&|| self.render_struct_literal_field_label(field),
                                                 "struct_literal_field_label",
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
                            })
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
            return self.render_struct_identifier(strukt);
        }
        let rhs = self.render_struct_literal_fields(&strukt, struct_literal.fields());
        let rhs: Vec<Box<dyn Fn() -> T::DrawResult>> =
            rhs.into_iter()
               .map(|draw_fn| {
                   let b: Box<dyn Fn() -> T::DrawResult> = Box::new(move || draw_fn(&self));
                   b
               })
               .collect_vec();
        self.ui_toolkit.align(&|| {
                                  self.code_handle(&|| self.render_struct_identifier(strukt),
                                                   "struct_literal_label",
                                                   struct_literal.id)
                              },
                              &rhs.iter().map(|b| b.as_ref()).collect_vec())
    }

    fn render_list_index(&self, list_index: &lang::ListIndex) -> T::DrawResult {
        self.draw_nested_borders_around(&|| {
                self.ui_toolkit.draw_all_on_same_line(&[
                &|| {
                    self.code_handle(&|| {
                                         self.render_without_nesting(&|| {
                                                 self.render_code(&list_index.list_expr)
                                             })
                                     },
                                     "list index label",
                                     list_index.id)
                },
                &|| {
                    self.render_without_nesting(&|| {
                            self.render_nested(&|| self.render_code(&list_index.index_expr))
                        })
                },
            ])
            })
    }

    fn render_struct_field_get(&self, sfg: &lang::StructFieldGet) -> T::DrawResult {
        let struct_field = self.env_genie
                               .find_struct_field(sfg.struct_field_id)
                               .unwrap();

        self.code_handle(
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
                         "struct field get label",
                         sfg.id,
        )
    }

    fn render_struct_identifier(&self, strukt: &structs::Struct) -> T::DrawResult {
        render_struct_identifier::<T>(strukt, &|name, color, typ| {
            self.render_name_with_type_definition(name, color, typ)
        })
    }

    fn render_control_flow_symbol(&self, symbol: &str) -> T::DrawResult {
        self.ui_toolkit
            .draw_buttony_text(symbol, darken(darken(CONTROL_FLOW_GREY_COLOR)))
    }

    fn render_control_flow_text(&self, text: &str) -> T::DrawResult {
        self.ui_toolkit
            .draw_buttony_text(text, CONTROL_FLOW_GREY_COLOR)
    }

    fn render_control_flow_label(&self, symbol: &str, label: &str) -> T::DrawResult {
        self.ui_toolkit
            .draw_all_on_same_line(&[&|| self.render_control_flow_symbol(symbol), &|| {
                                       self.render_control_flow_text(label)
                                   }])
    }

    fn render_conditional(&self, conditional: &lang::Conditional) -> T::DrawResult {
        self.ui_toolkit.draw_all(&[&|| {
                                       self.render_control_flow("\u{f2fd}",
                                                                "  If   ",
                                                                conditional.id,
                                                                Some(&|| self.render_code(&conditional.condition)),
                                                                0,
                                                                &conditional.true_branch)
                                   },
                                   &|| {
                                       self.render_control_flow(&ELSE_ICON,
                                                                "Else",
                                                                conditional.id,
                                                                None,
                                                                0,
                                                                conditional.else_branch
                                                                           .as_ref()
                                                                           .unwrap())
                                   }])
    }

    fn render_for_loop(&self, for_loop: &lang::ForLoop) -> T::DrawResult {
        self.render_control_flow("\u{f101}",
                                 "For",
                                 for_loop.id,
                                 Some(&|| {
                                     self.ui_toolkit.draw_all_on_same_line(&[
                                         &|| self.render_for_loop_variable(for_loop),
                                         &|| self.draw_text("   \u{f30a}   "),
                                         // &|| self.ui_toolkit.draw_buttony_text("in", CONTROL_FLOW_GREY_COLOR),
                                         &|| self.render_code(&for_loop.list_expression),
                                     ])
                                 }),
                                 1,
                                 &for_loop.body)
    }

    fn render_while_loop(&self, while_loop: &lang::WhileLoop) -> T::DrawResult {
        self.render_control_flow("\u{f2f9}",
                                 "While",
                                 while_loop.id,
                                 Some(&|| self.render_code(&while_loop.condition)),
                                 0,
                                 &while_loop.body)
    }

    fn render_control_flow(&self,
                           symbol: &str,
                           label: &str,
                           control_flow_node_id: lang::ID,
                           condition_row_draw: Option<DrawFnRef<T>>,
                           x_padding_left_block_hack: i8,
                           body_block: &lang::CodeNode)
                           -> T::DrawResult {
        let code_body_left_padding = 8;
        self.ui_toolkit.align_fill_lhs(x_padding_left_block_hack,
                                       &|| {
                                           self.code_handle(&|| {
                                                            self.render_control_flow_label(symbol,
                                                                                           label)
                                                        },
                                                        (label, body_block.id()),
                                                        control_flow_node_id)
                                       },
                                       CONTROL_FLOW_GREY_COLOR,
                                       &[&|| {
                                             if let Some(condition_row_draw) = condition_row_draw {
                                                 condition_row_draw()
                                             } else {
                                                 self.ui_toolkit.draw_all(&[])
                                             }
                                         },
                                         &|| {
                                             self.ui_toolkit.indent(code_body_left_padding, &|| {
                                                                self.render_code(&body_block)
                                                            })
                                             //            self.render_block_inside_child_region(body_block
                                             // .as_block()
                                             // .unwrap(), ChildRegionFrameStyle::NoFrame)
                                             //        })
                                         }])
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
                if type_and_enum_by_variant_id.len() != mach.branch_by_variant_id.len() {
                    return self.ui_toolkit.draw_buttony_text("Enum and code mismatch", colorscheme!(danger_color))
                }
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
                                    render_enum_variant_identifier(self.ui_toolkit,
                                        self.env_genie,
                                        &match_variant.enum_variant,
                                        &match_variant.typ)
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

    fn render_enum_variant_literal(&self,
                                   evl: &lang::EnumVariantLiteral,
                                   typ: &lang::Type)
                                   -> T::DrawResult {
        let (_, enum_variant) = self.env_genie.find_enum_variant(evl.variant_id).unwrap();
        self.ui_toolkit.draw_all_on_same_line(&[
            &|| {
                self.code_handle(&|| {
                                     render_enum_variant_identifier(self.ui_toolkit,
                                                                    self.env_genie,
                                                                    enum_variant,
                                                                    &typ)
                                 },
                                 "enum variant identifier label",
                                 evl.id)
            },
            &|| self.render_nested(&|| self.render_code(&evl.variant_value_expr)),
        ])
    }

    fn render_indented(&self, draw_fn: &dyn Fn() -> T::DrawResult) -> T::DrawResult {
        self.ui_toolkit.indent(PX_PER_INDENTATION_LEVEL, draw_fn)
    }

    fn render_string_literal(&self, string_literal: &lang::StringLiteral) -> T::DrawResult {
        self.code_handle(&|| {
                                       self.draw_buttony_text(&self.format_string_literal_display(&string_literal.value),
                                                              colorscheme!(literal_bg_color))
                                   },
                         "string literal label", string_literal.id)
    }

    fn format_string_literal_display(&self, value: &str) -> String {
        let most_num_newlines_to_display = 2;
        let inner_value = if value.matches("\n").count() >= most_num_newlines_to_display {
            let i = value.match_indices("\n")
                         .nth(most_num_newlines_to_display - 1)
                         .unwrap()
                         .0;
            // TODO: would like to use unicode ellipsis here but need to fix fonts
            format!("{}...", value.chars().take(i).join(""))
        } else {
            value.to_string()
        };
        format!("\u{F10D}{}\u{F10E}", inner_value)
    }

    fn render_number_literal(&self, number_literal: &lang::NumberLiteral) -> T::DrawResult {
        // TODO: for now lettttttt's not implement the editor for number literals. i think we
        // don't need it just yet. the insert code menu can insert number literals. perhaps we
        // can implement an InsertionPoint::Replace(node_id) that will suffice for number literals
        self.code_handle(&|| {
                             self.draw_buttony_text(&number_literal.value.to_string(),
                                                    colorscheme!(literal_bg_color))
                         },
                         "number literal label",
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
                                   self.draw_multiline_text_editor(&string_literal.value,
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
                let type_of_assignment =
                    self.code_editor
                        .code_genie
                        .guess_type(assignment.expression.as_ref(), self.env_genie)
                        .unwrap();

                self.render_assignment_specify_lhs(assignment, &|| {
                        self.ui_toolkit.draw_all_on_same_line(&[
                        &|| {
                            render_type_symbol(self.ui_toolkit,
                                               self.env_genie,
                                               colorscheme!(variable_color),
                                               &type_of_assignment)
                        },
                        &|| {
                            self.ui_toolkit.focused(&|| {
                                let a = assignment.clone();
                                self.draw_inline_text_editor(&assignment.name, move |new_value| {
                                    let mut new_assignment = a.clone();
                                    new_assignment.name = new_value.to_string();
                                    CodeNode::Assignment(new_assignment)
                                })
                            })
                        },
                    ])
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
            otherwise => {
                self.ui_toolkit
                    .draw_text(&format!("wasn't supposed to edit this node {:?}", otherwise))
            }
        }
    }

    fn draw_multiline_text_editor<F: Fn(&str) -> CodeNode + 'static>(&self,
                                                                     initial_value: &str,
                                                                     new_node_fn: F)
                                                                     -> T::DrawResult {
        let cmd_buffer = Rc::clone(&self.command_buffer);
        let cmd_buffer2 = Rc::clone(&self.command_buffer);
        let cmd_buffer3 = Rc::clone(&self.command_buffer);
        let new_node_fn = Rc::new(new_node_fn);

        self.draw_multiline_text_input(
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
                                   move |key| match key {
                                       Keypress { key: Key::Escape, .. } => {
                                           cmd_buffer3.borrow_mut().add_editor_command(|editor| {
                                               editor.mark_as_not_editing();
                                           })
                                       }
                                       _ => (),
                                   },
        )
    }

    fn draw_inline_text_editor<F: Fn(&str) -> CodeNode + 'static>(&self,
                                                                  initial_value: &str,
                                                                  new_node_fn: F)
                                                                  -> T::DrawResult {
        let cmd_buffer = Rc::clone(&self.command_buffer);
        let cmd_buffer2 = Rc::clone(&self.command_buffer);
        let cmd_buffer3 = Rc::clone(&self.command_buffer);

        let new_node_fn = Rc::new(new_node_fn);

        self.draw_text_input(
                             initial_value,
                             true,
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
                             move |key| match key {
                                 Keypress { key: Key::Escape, .. } => {
                                     cmd_buffer3.borrow_mut().add_editor_command(|editor| {
                                         editor.mark_as_not_editing();
                                     })
                                 }
                                 _ => (),
                             },
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

    fn draw_multiline_text_input<F: Fn(&str) + 'static, E: Fn() + 'static>(&self,
                                                                           existing_value: &str,
                                                                           onchange: F,
                                                                           onenter: E,
                                                                           onkeypress: impl Fn(Keypress) + 'static)
                                                                           -> T::DrawResult {
        let onchange_rc = Rc::new(RefCell::new(onchange));
        let onenter_rc = Rc::new(RefCell::new(onenter));
        let onkeypress_rc = Rc::new(RefCell::new(onkeypress));
        self.draw_nested_borders_around(&move || {
                let onchange_rc = Rc::clone(&onchange_rc);
                let onenter_rc = Rc::clone(&onenter_rc);
                let onkeypress_rc = Rc::clone(&onkeypress_rc);
                self.ui_toolkit.draw_multiline_text_input_with_label("",
                                                                     existing_value,
                                                                     move |v| {
                                                                         onchange_rc.borrow()(v)
                                                                     },
                                                                     move || onenter_rc.borrow()(),
                                                                     move |key| {
                                                                         onkeypress_rc.borrow()(key)
                                                                     })
            })
    }

    fn draw_text_input<F: Fn(&str) + 'static, D: Fn() + 'static>(&self,
                                                                 existing_value: &str,
                                                                 fit_input_width: bool,
                                                                 onchange: F,
                                                                 ondone: D,
                                                                 onkeypress: impl Fn(Keypress)
                                                                     + 'static)
                                                                 -> T::DrawResult {
        let onchange_rc = Rc::new(RefCell::new(onchange));
        let ondone_rc = Rc::new(RefCell::new(ondone));
        let onkeypress_rc = Rc::new(RefCell::new(onkeypress));
        self.draw_nested_borders_around(&|| {
                let onchange_rc = Rc::clone(&onchange_rc);
                let ondone_rc = Rc::clone(&ondone_rc);
                let onkeypress_rc = Rc::clone(&onkeypress_rc);
                self.ui_toolkit.draw_text_input(existing_value,
                                                fit_input_width,
                                                move |v| onchange_rc.borrow()(v),
                                                move || ondone_rc.borrow()(),
                                                move |key| onkeypress_rc.borrow()(key))
            })
    }

    fn insertion_point(&self) -> Option<InsertionPoint> {
        Some(self.code_editor.insert_code_menu.as_ref()?.insertion_point)
    }

    fn is_rendering_menu_atm(&self) -> bool {
        *self.is_rendering_menu.borrow()
    }

    fn code_handle(&self,
                   draw_handle_fn: &dyn Fn() -> T::DrawResult,
                   unique_label_per_code_id: impl std::hash::Hash + Clone + std::fmt::Debug,
                   code_node_id: lang::ID)
                   -> T::DrawResult {
        if self.is_rendering_menu_atm() {
            return draw_handle_fn();
        }

        let draw_handle_fn = &|| {
            let cmd_buffer = self.command_buffer.clone();
            self.ui_toolkit
                .callback_when_drag_intersects(draw_handle_fn, move || {
                    cmd_buffer.borrow_mut()
                              .add_overlapped_code_node_id(code_node_id);
                })
        };

        let cmd_buffer = Rc::clone(&self.command_buffer);
        let code_to_drag_drop = self.code_to_drag_drop(code_node_id)
                                    .cloned()
                                    .collect::<Vec<_>>();
        let drag_drop_payload = DragDropPayload::new(self.code_editor.id(), code_to_drag_drop);

        self.ui_toolkit.drag_drop_source(
                                         (unique_label_per_code_id, code_node_id),
                                         &|| {
                                             let cmd_buffer = Rc::clone(&cmd_buffer);
                                             self.ui_toolkit.buttonize(draw_handle_fn, move || {
                                                                cmd_buffer.borrow_mut()
                    .add_editor_command(move |editor| {
                        editor.set_selected_node_id(Some(code_node_id))
                    })
                                                            })
                                         },
                                         // TODO: draw everything that's selected
                                         &|| {
                                             draw_all_iter!(T::self.ui_toolkit,
                                                            self.code_to_drag_drop(code_node_id)
                                                                .map(move |code_node| {
                                                                    move || {
                                                                        self.render_code(code_node)
                                                                    }
                                                                }))
                                         },
                                         // TODO: everything that's selected
                                         drag_drop_payload,
        )
    }

    fn code_to_drag_drop(&self,
                         clicked_code_node_id: lang::ID)
                         -> Box<dyn Iterator<Item = &lang::CodeNode> + '_> {
        if !self.code_editor
                .selected_node_ids
                .contains(&clicked_code_node_id)
        {
            Box::new(self.code_editor
                         .code_genie
                         .find_node(clicked_code_node_id)
                         .into_iter())
        } else {
            Box::new(self.code_editor
                         .selected_node_ids
                         .iter()
                         .map(move |id| self.code_editor.code_genie.find_node(*id))
                         .flatten())
        }
    }
}

struct PerEditorCommandBuffer {
    overlapped_code_node_ids: Vec<lang::ID>,
    actual_command_buffer: Rc<RefCell<editor::CommandBuffer>>,
    editor_id: lang::ID,
}

impl PerEditorCommandBuffer {
    pub fn new(actual_command_buffer: Rc<RefCell<editor::CommandBuffer>>,
               editor_id: lang::ID)
               -> Self {
        Self { actual_command_buffer,
               editor_id,
               overlapped_code_node_ids: vec![] }
    }

    pub fn add_overlapped_code_node_id(&mut self, id: lang::ID) {
        self.overlapped_code_node_ids.push(id);
    }

    pub fn set_selected_node_ids(&mut self) {
        let overlapped_code_node_ids = self.overlapped_code_node_ids.clone();
        self.add_editor_command(move |editor| {
                let ids = editor.code_genie
                                .dedup_and_sort_children(overlapped_code_node_ids.iter().cloned());
                editor.selected_node_ids = ids;
            });
    }

    pub fn handle_drag_drop(&mut self,
                            payload: DragDropPayload,
                            to: InsertionPoint,
                            target_editor_id: lang::ID) {
        let node_ids = payload.code_nodes
                              .iter()
                              .map(|code| code.id())
                              .collect_vec();
        let from_editor_id = payload.from_code_editor_id;
        // XXX: i don't understand this well, but for some reason the insert needs to happen before
        // the delete... seems like it should be the other way around

        // insert into target
        self.actual_command_buffer
            .borrow_mut()
            .add_editor_command(target_editor_id, move |editor, _| {
                editor.insert_code_and_set_where_cursor_ends_up_next(payload.code_nodes, to);
            });

        // delete code from source
        self.actual_command_buffer
            .borrow_mut()
            .add_editor_command(from_editor_id, move |editor, _| {
                editor.delete_node_ids(node_ids.into_iter());
            });
    }

    pub fn add_editor_command<F: FnOnce(&mut code_editor::CodeEditor) + 'static>(&mut self, f: F) {
        self.actual_command_buffer
            .borrow_mut()
            .add_editor_command(self.editor_id, |editor, _| f(editor));
    }

    pub fn add_editor_and_interp_command<F: FnOnce(&mut code_editor::CodeEditor,
                                                        &mut env::Interpreter)
                                                 + 'static>(
        &mut self,
        f: F) {
        self.actual_command_buffer
            .borrow_mut()
            .add_editor_command(self.editor_id, f);
    }
}
