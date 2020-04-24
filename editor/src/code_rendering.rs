use crate::code_editor_renderer::BLACK_COLOR; // TODO: maybe this should be part of this module instead?
use crate::colorscheme;
use crate::ui_toolkit::{Color, DrawFnRef, UiToolkit};
use cs::{lang, structs, EnvGenie};
use lazy_static::lazy_static;
use std::cell::RefCell;

lazy_static! {
    static ref NULL_TEXT: String = format!(" {} ", lang::NULL_TYPESPEC.symbol);
}
// const ARGUMENT_GREY_COLOR: Color = [0.411, 0.411, 0.411, 1.];
const ARGUMENT_GREY_COLOR: Color = [0., 0., 0., 1.];

pub fn render_null<T: UiToolkit>(ui_toolkit: &T) -> T::DrawResult {
    ui_toolkit.draw_text(&NULL_TEXT)
}

pub fn render_list_literal_value<T: UiToolkit>(ui_toolkit: &T,
                                               pos: usize,
                                               render_value_fn: DrawFnRef<T>)
                                               -> T::DrawResult {
    ui_toolkit.draw_all_on_same_line(&[&|| render_list_literal_position(ui_toolkit, pos),
                                       render_value_fn])
}

pub fn render_list_literal_position<T: UiToolkit>(ui_toolkit: &T, pos: usize) -> T::DrawResult {
    draw_border_inside(ui_toolkit, BLACK_COLOR, [2, 1, 1, 1], &|| {
        render_argument_label(ui_toolkit, &pos.to_string())
    })
}

pub fn render_list_literal_label<T: UiToolkit>(ui_toolkit: &T,
                                               env_genie: &EnvGenie,
                                               typ: &lang::Type)
                                               -> T::DrawResult {
    // TODO: we can use smth better to express the nesting than ascii art, like our nesting scheme
    //       with the black lines (can actually make that generic so we can swap it with something
    //       else
    let type_symbol = env_genie.get_symbol_for_type(typ);
    ui_toolkit.draw_buttony_text(&type_symbol, colorscheme!(cool_color))
}

pub fn render_struct_field<T: UiToolkit>(ui_toolkit: &T,
                                         render_label_fn: DrawFnRef<T>,
                                         render_value_fn: DrawFnRef<T>)
                                         -> T::DrawResult {
    ui_toolkit.draw_all_on_same_line(&[render_label_fn, render_value_fn])
}

pub fn render_struct_field_label<T: UiToolkit>(ui_toolkit: &T,
                                               env_genie: &EnvGenie,
                                               field: &structs::StructField)
                                               -> T::DrawResult {
    let field_text = format!("{} {}",
                             env_genie.get_symbol_for_type(&field.field_type),
                             field.name);
    render_argument_label(ui_toolkit, &field_text)
}

pub fn render_struct_identifier<T: UiToolkit>(strukt: &structs::Struct,
                                              render_name_with_type_fn: &dyn Fn(&str,
                                                      Color,
                                                      &lang::Type)
                                                      -> T::DrawResult)
                                              -> T::DrawResult {
    let typ = lang::Type::from_spec(strukt);
    render_name_with_type_fn(&strukt.name, colorscheme!(cool_color), &typ)
}

pub fn render_name_with_type_definition<T: UiToolkit>(ui_toolkit: &T,
                                                      env_genie: &EnvGenie,
                                                      name: &str,
                                                      color: Color,
                                                      typ: &lang::Type)
                                                      -> T::DrawResult {
    let sym = env_genie.get_symbol_for_type(typ);
    let darker_color = darken(color);
    ui_toolkit.draw_all_on_same_line(&[&|| ui_toolkit.draw_buttony_text(&sym, darker_color),
                                       &|| ui_toolkit.draw_buttony_text(name, color)])
}

pub struct NestingRenderer<'a, T: UiToolkit> {
    ui_toolkit: &'a T,
    // this is a RefCell because this function is going to call itself recursively... it would
    // violate the exclusive lock to have a mutable reference to this guy, and then need to call
    // it recursively
    nesting_level: RefCell<u8>,
}

impl<'a, T: UiToolkit> NestingRenderer<'a, T> {
    pub fn new(ui_toolkit: &'a T) -> Self {
        Self { ui_toolkit,
               nesting_level: RefCell::new(0) }
    }

    pub fn current_nesting_level(&self) -> u8 {
        *self.nesting_level.borrow()
    }

    pub fn incr_nesting_level(&self) {
        self.nesting_level.replace_with(|l| *l + 1);
    }

    pub fn decr_nesting_level(&self) {
        self.nesting_level.replace_with(|l| *l - 1);
    }

    pub fn draw_nested(&self, draw_fn: DrawFnRef<T>) -> T::DrawResult {
        self.incr_nesting_level();
        let res = self.draw_nested_with_existing_level(draw_fn);
        self.decr_nesting_level();
        res
    }

    pub fn draw_nested_with_existing_level(&self, draw_fn: DrawFnRef<T>) -> T::DrawResult {
        draw_nested_borders_around(self.ui_toolkit, draw_fn, *self.nesting_level.borrow())
    }
}

// TODO: move this into the UiToolkit itself?
// XXX: i think there's a bug here and the top border gets cut off by 1 or smth, every place we
// call this, we have to bump the top border size by 1... will have to look into why that is, later
pub fn draw_border_inside<'a, T: UiToolkit>(ui_toolkit: &'a T,
                                            color: Color,
                                            // trbl: top right bottom left
                                            thickness_trbl: [u8; 4],
                                            draw_fn: DrawFnRef<'a, T>)
                                            -> T::DrawResult {
    ui_toolkit.draw_top_border_inside(color, thickness_trbl[0], &|| {
                  ui_toolkit.draw_right_border_inside(color, thickness_trbl[1], &|| {
                                ui_toolkit.draw_bottom_border_inside(color, thickness_trbl[2], &|| {
                                    ui_toolkit.draw_left_border_inside(color,
                                                                       thickness_trbl[3],
                                                                       draw_fn)
                                })
                            })
              })
}

pub fn draw_nested_borders_around<T: UiToolkit>(ui_toolkit: &T,
                                                draw_fn: DrawFnRef<T>,
                                                nesting_level: u8)
                                                -> T::DrawResult {
    if nesting_level == 0 {
        return draw_fn();
    }
    let top_border_thickness = 1 + nesting_level + 1;
    let right_border_thickness = 1;
    let left_border_thickness = 1;
    let bottom_border_thickness = 1;

    ui_toolkit.draw_top_border_inside(BLACK_COLOR, top_border_thickness as u8, &|| {
                  ui_toolkit.draw_right_border_inside(BLACK_COLOR, right_border_thickness, &|| {
                      ui_toolkit.draw_left_border_inside(BLACK_COLOR, left_border_thickness, &|| {
                          ui_toolkit.draw_bottom_border_inside(BLACK_COLOR,
                                                               bottom_border_thickness,
                                                               draw_fn)
                      })
                  })
              })
}

pub fn darken(mut color: Color) -> Color {
    color[0] *= 0.75;
    color[1] *= 0.75;
    color[2] *= 0.75;
    color
}

fn render_argument_label<T: UiToolkit>(ui_toolkit: &T, label: &str) -> T::DrawResult {
    ui_toolkit.draw_buttony_text(label, ARGUMENT_GREY_COLOR)
}
