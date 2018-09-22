use super::{CSApp, UiToolkit};
use super::imgui_support;
use imgui::*;

const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const BUTTON_SIZE: (f32, f32) = (0.0, 0.0);

// XXX: look into why this didn't compile before. remove all the lifetime specifiers, AND the
// lifetime specifier from the definition in App::draw. that'll get it back to the way i had it
// before
pub fn draw_app<'a, 'ui: 'a>(app: &'a CSApp) {
    imgui_support::run("cs".to_owned(), CLEAR_COLOR, |ui: &'ui Ui| {
        let toolkit = Box::new(ImguiToolkit::new(ui));
        app.draw(toolkit);
        true
    });
}

struct ImguiToolkit<'a> {
    ui: &'a Ui<'a>,
}

impl<'a> ImguiToolkit<'a> {
    pub fn new(ui: &'a Ui) -> ImguiToolkit<'a> {
        ImguiToolkit { ui: ui }
    }
}

impl<'a> UiToolkit for ImguiToolkit<'a> {
    fn draw_window(&self, window_name: &str, f: &Fn()) {
        self.ui.window(im_str!("{}", window_name))
            .size((300.0, 100.0), ImGuiCond::FirstUseEver)
            .build(f)
    }

    fn draw_empty_line(&self) {
        self.ui.new_line();
    }

    fn draw_button(&self, label: &str, color: [f32; 4], f: &Fn()) {
        self.ui.with_color_var(ImGuiCol::Button, color, || {
            if self.ui.button(im_str!("{}", label), BUTTON_SIZE) {
                f()
            }
        });
    }

    fn draw_next_on_same_line(&self) {
        self.ui.same_line_spacing(0.0, 1.0);
    }

    fn draw_text_box(&self, text: &str) {
        self.ui.text(text);
        // GHETTO: text box is always scrolled to the bottom
        unsafe { imgui_sys::igSetScrollHere(1.0) };
    }
}
