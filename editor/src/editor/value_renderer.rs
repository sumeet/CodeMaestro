// TODO: many of the draw functions in here are copy and pasted from CodeEditorRenderer...
// ... i couldn't find a good way of sharing the code, but i think i might need to eventually.
// for now it's just copy+paste
use crate::align;
use crate::code_editor_renderer::BLACK_COLOR;
use crate::code_rendering::{
    render_enum_variant_identifier, render_list_literal_label, render_list_literal_value,
    render_name_with_type_definition, render_null, render_struct_field, render_struct_field_label,
    render_struct_identifier, NestingRenderer,
};
use crate::colorscheme;
use crate::draw_all_iter;
use crate::ui_toolkit::{Color, UiToolkit};
use cs::env::ExecutionEnvironment;
use cs::lang::{StructValues, Value};
use cs::{lang, structs, EnvGenie};
use lazy_static::lazy_static;

lazy_static! {
    static ref TRUE_LABEL: String = format!("{} True", lang::BOOLEAN_TYPESPEC.symbol);
    static ref FALSE_LABEL: String = format!("{} False", lang::BOOLEAN_TYPESPEC.symbol);
    static ref NULL_LABEL: String = format!("{} Null", lang::NULL_TYPESPEC.symbol);
}

pub struct ValueRenderer<'a, T: UiToolkit> {
    nesting_renderer: NestingRenderer<'a, T>,
    env_genie: EnvGenie<'a>,
    #[allow(unused)]
    env: &'a ExecutionEnvironment,
    ui_toolkit: &'a T,
}

impl<'a, T: UiToolkit> ValueRenderer<'a, T> {
    pub fn new(env: &'a ExecutionEnvironment, ui_toolkit: &'a T) -> Self {
        let env_genie = EnvGenie::new(env);
        Self { env,
               env_genie,
               nesting_renderer: NestingRenderer::new(ui_toolkit),
               ui_toolkit }
    }

    pub fn render(&'a self, value: &'a lang::Value) -> T::DrawResult {
        match value {
            Value::Null => render_null(self.ui_toolkit),
            Value::Boolean(bool) => {
                let label = if *bool {
                    TRUE_LABEL.as_ref()
                } else {
                    FALSE_LABEL.as_ref()
                };
                self.draw_buttony_text(label, colorscheme!(literal_bg_color))
            }
            Value::String(string) => self.render_string(string),
            Value::Number(num) => self.render_number(num),
            Value::List(typ, values) => self.render_list(typ, values),
            Value::Struct { struct_id, values } => self.render_struct(struct_id, values),
            Value::Future(_) => self.draw_buttony_text("Future", BLACK_COLOR),
            Value::EnumVariant { variant_id, value } => self.render_enum(variant_id, value),
            Value::AnonymousFunction(_, _) => panic!("don't worry about rendering functions"),
            Value::EarlyReturn(inner) => {
                self.ui_toolkit.draw_all_on_same_line(&[&|| {
                                                            self.ui_toolkit
                                                                .draw_text("Early return: ")
                                                        },
                                                        &|| self.render(inner.as_ref())])
            }
            Value::Map { from: _,
                         to: _,
                         value, } => {
                // in a rush because of advent of code, will make this prettier later
                draw_all_iter!(T::self.ui_toolkit,
                               value.iter().map(|(key, val)| {
                                               move || {
                                                   self.ui_toolkit
                                       .draw_all_on_same_line(&[&|| self.render(key), &|| {
                                                                  self.render(val)
                                                              }])
                                               }
                                           }))
            }
        }
    }

    fn render_enum(&self, variant_id: &lang::ID, value: &lang::Value) -> T::DrawResult {
        // TODO: perhaps share this later with the enum literal code (no enum literal yet)
        self.ui_toolkit.draw_all_on_same_line(&[&|| {
                                                    let (_, enum_variant) =
                                                        self.env_genie
                                                            .find_enum_variant(*variant_id)
                                                            .unwrap();
                                                    let typ =
                                                        self.env_genie.guess_type_of_value(value);
                                                    render_enum_variant_identifier(self.ui_toolkit,
                                                                                   &self.env_genie,
                                                                                   enum_variant,
                                                                                   &typ)
                                                },
                                                &|| self.render_nested_value(value)])
    }

    fn render_nested_value(&self, value: &lang::Value) -> T::DrawResult {
        if self.is_scalar(value) {
            self.nesting_renderer.draw_nested(&|| self.render(value))
        } else {
            self.nesting_renderer.incr_nesting_level();
            let rendered = self.render(value);
            self.nesting_renderer.decr_nesting_level();
            rendered
        }
    }

    fn is_scalar(&self, value: &lang::Value) -> bool {
        match value {
            Value::Null
            | Value::Boolean(_)
            | Value::String(_)
            | Value::Number(_)
            | Value::Future(_) => true,
            Value::EnumVariant { .. }
            | Value::List(_, _)
            | Value::Struct { .. }
            | Value::Map { .. } => false,
            Value::AnonymousFunction(_, _) => {
                panic!("no value rendering implemented for anonymous functions")
            }
            Value::EarlyReturn(inner) => self.is_scalar(inner.as_ref()),
        }
    }

    fn render_list(&self, typ: &lang::Type, values: &[lang::Value]) -> T::DrawResult {
        align!(T::self.ui_toolkit,
               &|| {
                   render_list_literal_label(self.ui_toolkit, &self.env_genie, &lang::Type::list_of(typ.clone()))
               },
               values.iter()
                     .enumerate()
                     .map(|(pos, value)| {
                         move || {
                             render_list_literal_value(self.ui_toolkit, pos, &|| {
                                 self.render_nested_value(value)
                             })
                         }
                     }))
    }

    fn render_struct(&self, struct_id: &lang::ID, values: &StructValues) -> T::DrawResult {
        let strukt = self.env_genie.find_struct(*struct_id).unwrap();
        align!(T::self.ui_toolkit,
               &|| {
                   self.nesting_renderer
                       .draw_nested_with_existing_level(&|| self.render_struct_identifier(strukt))
               },
               strukt.fields.iter().map(|strukt_field| {
                                       move || {
                                           let value = values.0.get(&strukt_field.id).unwrap();
                                           self.render_struct_field_value(strukt_field, value)
                                       }
                                   }))
    }

    fn render_number(&self, value: &i128) -> T::DrawResult {
        self.ui_toolkit
            .draw_buttony_text(&value.to_string(), colorscheme!(literal_bg_color))
    }

    fn render_string(&self, value: &str) -> T::DrawResult {
        self.draw_buttony_text(&format!("\u{F10D}{}\u{F10E}", value),
                               colorscheme!(literal_bg_color))
    }

    fn render_struct_identifier(&self, strukt: &structs::Struct) -> T::DrawResult {
        render_struct_identifier::<T>(strukt, &|name, color, typ| {
            render_name_with_type_definition(self.ui_toolkit, &self.env_genie, name, color, typ)
        })
    }

    fn render_struct_field_value(&self,
                                 strukt_field: &structs::StructField,
                                 value: &lang::Value)
                                 -> T::DrawResult {
        render_struct_field(self.ui_toolkit,
                            &|| {
                                render_struct_field_label(self.ui_toolkit,
                                                          &self.env_genie,
                                                          strukt_field)
                            },
                            &|| self.render_nested_value(value))
    }

    fn draw_buttony_text(&self, label: &str, color: Color) -> T::DrawResult {
        self.ui_toolkit.draw_buttony_text(label, color)
    }
}
