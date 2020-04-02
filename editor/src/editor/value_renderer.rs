use crate::align;
use crate::colorscheme;
use crate::ui_toolkit::{DrawFnRef, UiToolkit};
use cs::env::ExecutionEnvironment;
use cs::lang::{StructValues, Value};
use cs::{lang, EnvGenie};
use lazy_static::lazy_static;

lazy_static! {
    static ref TRUE_LABEL: String = format!("{} True", lang::BOOLEAN_TYPESPEC.symbol);
    static ref FALSE_LABEL: String = format!("{} False", lang::BOOLEAN_TYPESPEC.symbol);
    static ref NULL_LABEL: String = format!("{} Null", lang::NULL_TYPESPEC.symbol);
}

pub struct ValueRenderer<'a, T: UiToolkit> {
    env_genie: EnvGenie<'a>,
    #[allow(unused)]
    env: &'a ExecutionEnvironment,
    value: &'a lang::Value,
    ui_toolkit: &'a T,
}

impl<'a, T: UiToolkit> ValueRenderer<'a, T> {
    pub fn new(env: &'a ExecutionEnvironment, value: &'a lang::Value, ui_toolkit: &'a T) -> Self {
        let env_genie = EnvGenie::new(env);
        Self { env,
               env_genie,
               value,
               ui_toolkit }
    }

    pub fn render(&self) -> T::DrawResult {
        let label = match self.value {
            Value::Null => NULL_LABEL.clone(),
            Value::Boolean(bool) => {
                if *bool {
                    TRUE_LABEL.clone()
                } else {
                    FALSE_LABEL.clone()
                }
            }
            Value::String(string) => format!("{}{}{}",
                                             lang::STRING_TYPESPEC.symbol,
                                             string,
                                             lang::STRING_TYPESPEC.symbol),
            Value::Error(e) => format!("Error: {:?}", e),
            Value::Number(num) => num.to_string(),
            Value::List(_) => {
                panic!("let's worry about lists later, they're not even in the example")
            }
            Value::Struct { struct_id, values } => {
                return align!(T::self.ui_toolkit,
                              &|| self.render_struct_symbol_and_name_button(struct_id),
                              values.iter().map(|(struct_field_id, value)| {
                                               move || {
                                                   self.render_struct_field_value(struct_id,
                                                                                  struct_field_id,
                                                                                  value)
                                               }
                                           }))
            }
            Value::Future(_) => {
                panic!("let's worry about lists later, they're not even in the example")
            }
            Value::Enum { .. } => {
                panic!("let's worry about lists later, they're not even in the example")
            }
        };
        self.draw_buttony_text(&label)
    }

    fn render_struct_symbol_and_name_button(&self, struct_id: &lang::ID) -> T::DrawResult {
        let typ = lang::Type::from_spec_id(*struct_id, vec![]);
        let type_display_info = self.env_genie.get_type_display_info(&typ).unwrap();
        self.draw_buttony_text(&format!("{:?} {:?}",
                                        type_display_info.symbol, type_display_info.name))
    }

    fn render_struct_field_value(&self,
                                 struct_id: &lang::ID,
                                 struct_field_id: &lang::ID,
                                 value: &lang::Value)
                                 -> T::DrawResult {
        let (_strukt, strukt_field) = self.env_genie
                                          .get_struct_and_field(*struct_id, *struct_field_id)
                                          .unwrap();
        let type_display_info = self.env_genie
                                    .get_type_display_info(&strukt_field.field_type)
                                    .unwrap();
        let arg_name = &strukt_field.name;
        self.ui_toolkit.draw_all_on_same_line(&[
            &|| self.draw_buttony_text(&format!("{:?} {:?}", type_display_info.symbol, arg_name)),
            &|| Self::new(self.env, value, self.ui_toolkit).render(),
        ])
    }

    fn draw_buttony_text(&self, label: &str) -> T::DrawResult {
        self.ui_toolkit
            .draw_buttony_text(label, colorscheme!(menubar_color))
    }
}
