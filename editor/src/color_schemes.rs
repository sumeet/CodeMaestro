use crate::ui_toolkit::Color;

#[derive(Clone, Copy)]
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
                  separator_color: [144. / 255., 144. / 255., 144. / 255., 1.],
                  selection_overlay_color: [1., 1., 1., 0.3] };

pub static COLOR_SCHEME: ColorScheme = IMGUI_CLASSIC_COLOR_SCHEME;
