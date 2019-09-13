use crate::ui_toolkit::Color;
use lazy_static::lazy_static;
use serde_derive::{Deserialize, Serialize};
use std::sync::{Mutex, MutexGuard};

lazy_static! {
    pub static ref COLOR_SCHEME: Mutex<ColorScheme> =
        Mutex::new(IMGUI_CLASSIC_COLOR_SCHEME.clone());
}

#[macro_export]
macro_rules! colorscheme {
    ($attr_name:ident) => {{
        let mut _color: $crate::ui_toolkit::Color;
        // use an inner scope to make sure the mutex is dropped
        {
            _color = $crate::color_schemes::colorscheme2().$attr_name.clone();
        }
        _color
    }};
}

pub fn set_colorscheme(colorscheme: ColorScheme) {
    *colorscheme2() = colorscheme;
}

pub fn serialize_colorscheme() -> String {
    serde_json::to_string_pretty(&*colorscheme2()).unwrap()
}

pub fn colorscheme2() -> MutexGuard<'static, ColorScheme> {
    COLOR_SCHEME.lock().unwrap()
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ColorScheme {
    pub action_color: Color,
    // traditionally a green, like green in diffs
    pub adding_color: Color,
    // TODO: rename this, not sure what to call it. but i'm just mass replacing
    // blue right now. and the places i'm using blue don't seem to be super
    // related in function. so for now i'll call it cool_color
    pub cool_color: Color,
    pub button_active_color: Color,
    pub button_hover_color: Color,
    pub literal_bg_color: Color,
    pub danger_color: Color,
    pub selection_overlay_color: Color,
    pub separator_color: Color,
    pub text_color: Color,
    pub variable_color: Color,
    pub warning_color: Color,
    pub window_bg_color: Color,
    pub child_region_bg_color: Color,
    pub menubar_color: Color,
    pub titlebar_bg_color: Color,
    pub titlebar_active_bg_color: Color,
    pub input_bg_color: Color,
}

impl ColorScheme {
    pub fn from_json(json_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(json_str)?)
    }
}

pub static IMGUI_CLASSIC_COLOR_SCHEME: ColorScheme =
    ColorScheme { text_color: [1., 1., 1., 1.],
                  adding_color: [80. / 255., 161. / 255., 78. / 255., 1.],
                  button_active_color: [0.46, 0.54, 0.8, 0.6],
                  // pulled out from imgui classic theme, style.colors[ImGuiCol::ButtonHovered]
                  button_hover_color: [0.4, 0.48, 0.71, 0.6],
                  cool_color: [100.0 / 255.0, 149.0 / 255.0, 237.0 / 255.0, 1.0],
                  literal_bg_color: [0., 0., 0., 0.],
                  danger_color: [0.858, 0.180, 0.180, 1.0],
                  warning_color: [253.0 / 255.0, 159.0 / 255.0, 19.0 / 255.0, 0.4],
                  action_color: [0.521, 0.521, 0.521, 1.0],
                  variable_color: [0.486, 0.353, 0.952, 1.0],
                  // the default BG color is transparent black, which is super annoying. make it a
                  // little lighter (0.3 -> 0.35), so it contrasts with the black used for
                  // signifying nesting.
                  window_bg_color: [0.375, 0.375, 0.375, 1.0],
                  // lifted from imgui classic colorscheme
                  child_region_bg_color: [0.396, 0.396, 0.396, 1.0],
                  separator_color: [144. / 255., 144. / 255., 144. / 255., 1.],
                  selection_overlay_color: [1., 1., 1., 0.3],
                  // lifted from imgui classic colorscheme
                  // using the colorpicker because imgui uses layered transparent backgrounds, that
                  // are not only difficult to reproduce, but we don't want translucency anyway
                  input_bg_color: [0.3961, 0.3961, 0.3961, 1.],
                  menubar_color: [0.3961, 0.3961, 0.5137, 1.],
                  titlebar_bg_color: [0.3922, 0.3922, 0.6196, 1.],
                  titlebar_active_bg_color: [0.4078, 0.4078, 0.6784, 1.] };
