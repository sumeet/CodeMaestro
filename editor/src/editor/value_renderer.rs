use crate::colorscheme;
use crate::draw_iter_to_vec;
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
                use itertools::Itertools;
                let v = {
                    draw_iter_to_vec!(T::self.ui_toolkit,
                                                     values.iter().map(|(struct_field_id, value)| {
                                                               move || self.render_struct_field_value(struct_field_id, value)
                                                           }))
                };
                return self.ui_toolkit
                           .align(&|| self.render_struct_symbol_and_name_button(struct_id), &v);
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
                                        type_display_info.symbol, type_display_info.symbol))
    }

    fn render_struct_field_value(&self,
                                 struct_field_id: &lang::ID,
                                 value: &lang::Value)
                                 -> T::DrawResult {
        unimplemented!()
    }

    fn draw_buttony_text(&self, label: &str) -> T::DrawResult {
        self.ui_toolkit
            .draw_buttony_text(label, colorscheme!(menubar_color))
    }
}
