use super::{CSApp, UiToolkit,Key};
use super::imgui_support;
use imgui::*;
use std::rc::Rc;
use std::cell::RefCell;

const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const BUTTON_SIZE: (f32, f32) = (0.0, 0.0);

// XXX: look into why this didn't compile before. remove all the lifetime specifiers, AND the
// lifetime specifier from the definition in App::draw. that'll get it back to the way i had it
// before
pub fn draw_app(app: Rc<CSApp>) {
    imgui_support::run("cs".to_owned(), CLEAR_COLOR,
        |ui| {
            let mut toolkit = ImguiToolkit::new(ui);
            app.draw(&mut toolkit);
            true
        },
        |key| { app.controller.borrow_mut().handle_key_press(key) },
    );
}

struct State {
    editing_text_input_buffer: Option<Rc<RefCell<ImString>>>,
}

impl State {
    fn new() -> Self {
        State { editing_text_input_buffer: None }
    }

    fn text_input_buffer(&mut self, initial_text: &str) -> Rc<RefCell<ImString>> {
        if self.editing_text_input_buffer.is_none() {
            let mut imstr = ImString::with_capacity(100);
            imstr.push_str(initial_text);
            self.editing_text_input_buffer = Some(Rc::new(RefCell::new(imstr)))
        }
        Rc::clone(&self.editing_text_input_buffer.as_ref().unwrap())
    }
}

struct ImguiToolkit<'a> {
    ui: &'a Ui<'a>,
    state: RefCell<State>,
}

impl<'a> ImguiToolkit<'a> {
    fn get_or_initialize_text_input(&self, existing_value: &str) -> Rc<RefCell<ImString>> {
        let mut state = self.state.borrow_mut();
        Rc::clone(&state.text_input_buffer(existing_value))
    }
}

impl<'a> ImguiToolkit<'a> {
    pub fn new(ui: &'a Ui) -> ImguiToolkit<'a> {
        ImguiToolkit {
            ui: ui,
            state: RefCell::new(State::new()),
        }
    }
}

impl<'a> UiToolkit for ImguiToolkit<'a> {
    fn draw_window(&self, window_name: &str, f: &Fn()) {
        self.ui.window(im_str!("{}", window_name))
            .size((300.0, 100.0), ImGuiCond::FirstUseEver)
            .build(f)
    }
    fn draw_layout_with_bottom_bar(&self, draw_content_fn: &Fn(), draw_bottom_bar_fn: &Fn()) {
        let frame_height = unsafe { imgui_sys::igGetFrameHeightWithSpacing() };
        self.ui.child_frame(im_str!(""), (0.0, -frame_height))
            .build(draw_content_fn);
        draw_bottom_bar_fn()
    }

    fn draw_empty_line(&self) {
        self.ui.new_line();
    }

    fn draw_border_around(&self, draw_fn: &Fn()) {
        self.ui.with_style_var(StyleVar::FrameBorderSize(4.0), draw_fn)
    }

    fn draw_button<F: Fn() + 'static>(&self, label: &str, color: [f32; 4], on_button_activate: F) {
            self.ui.with_color_var(ImGuiCol::Button, color, || {
                if self.ui.button(im_str!("{}", label), BUTTON_SIZE) {
                    on_button_activate()
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

    fn draw_text_input<F: Fn(&str) -> () + 'static, D: Fn() + 'static>(&self, existing_value: &str, onchange: F, ondone: D) {
        let input = self.get_or_initialize_text_input(existing_value);
        let mut input = input.borrow_mut();

        let mut flags = ImGuiInputTextFlags::empty();
        flags.set(ImGuiInputTextFlags::EnterReturnsTrue, true);

        let enter_pressed = self.ui.input_text(im_str!(""), &mut input)
            .flags(flags)
            .build();
        if enter_pressed {
            ondone();
            return
        }
        if input.as_ref() as &str != existing_value {
            onchange(input.as_ref() as &str)
        }
    }

    fn focus_last_drawn_element(&self) {
        unsafe { imgui_sys::igSetKeyboardFocusHere(0) }
    }
}
