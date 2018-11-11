use super::{CSApp};
use super::editor::{UiToolkit};
use super::editor::{Key};
use super::imgui_support;
use imgui::*;
use std::rc::Rc;
use std::cell::RefCell;

const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const BUTTON_SIZE: (f32, f32) = (0.0, 0.0);
const FIRST_WINDOW_PADDING: (f32, f32) = (25.0, 50.0);
const INITIAL_WINDOW_SIZE: (f32, f32) = (300.0, 200.0);

pub fn draw_app(app: Rc<CSApp>) {
    imgui_support::run("cs".to_owned(), CLEAR_COLOR,
       |ui| {
            let mut toolkit = ImguiToolkit::new(ui);
            app.draw(&mut toolkit);
            true
        },
       |keypress| { app.controller.borrow_mut().handle_keypress(keypress) },
    );
}

struct State {
    editing_text_input_buffer: Option<Rc<RefCell<ImString>>>,
    prev_window_size: (f32, f32),
    prev_window_pos: (f32, f32),
}

impl State {
    fn new() -> Self {
        State {
            editing_text_input_buffer: None,
            prev_window_pos: FIRST_WINDOW_PADDING,
            prev_window_size: (0.0, 0.0),
        }
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
    type DrawResult = ();

    // TODO: these should be draw funcs that we execute in here
    fn draw_all(&self, _draw_results: Vec<()>) {
    }

    fn focused(&self, draw_fn: &Fn()) {
        draw_fn();
        unsafe {
            // HACK: the igIsAnyItemHovered allows me to click buttons while focusing a text field.
            // good enough for now.
            if !imgui_sys::igIsAnyItemActive() && !imgui_sys::igIsAnyItemHovered() {
                imgui_sys::igSetKeyboardFocusHere(-1)
            }
        }
    }

    fn draw_statusbar(&self, draw_fn: &Fn()) {
        let x_padding = 4.0;
        let y_padding = 5.0;
        let font_size = unsafe { imgui_sys::igGetFontSize() };
        // cribbed status bar implementation from
        // https://github.com/ocornut/imgui/issues/741#issuecomment-233288320
        let status_height = (y_padding * 2.0) + font_size;

        let display_size = self.ui.imgui().display_size();
        let window_pos = (0.0, display_size.1 - status_height);
        let window_size = (display_size.0, status_height);

        self.ui.with_style_vars(
            &[
                StyleVar::WindowRounding(0.0),
                StyleVar::WindowPadding(ImVec2::new(x_padding, y_padding))
            ],
            &|| {
                self.ui.window(im_str!("statusbar"))
                    .collapsible(false)
                    .horizontal_scrollbar(false)
                    .scroll_bar(false)
                    .scrollable(false)
                    .resizable(false)
                    .always_auto_resize(false)
                    .title_bar(false)
                    .no_focus_on_appearing(true)
                    .movable(false)
                    .no_bring_to_front_on_focus(true)
                    .position(window_pos, ImGuiCond::Always)
                    .size(window_size, ImGuiCond::Always)
                    .build(draw_fn);
            }
        )
    }

    fn draw_text(&self, text: &str) {
        self.ui.text(text)
    }

    fn draw_all_on_same_line(&self, draw_fns: Vec<&Fn()>) {
        let draw_fns = draw_fns.as_slice();
        if draw_fns.is_empty() { return; }
        let last_index = draw_fns.len() - 1;
        let last_draw_fn = draw_fns[last_index];
        for draw_fn in &draw_fns[0..last_index] {
            draw_fn();
            self.ui.same_line_spacing(0.0, 1.0);
        }
        last_draw_fn();
    }

    fn draw_window(&self, window_name: &str, f: &Fn()) {
        let prev_window_size = self.state.borrow().prev_window_size;
        let prev_window_pos = self.state.borrow().prev_window_pos;
        self.ui.window(im_str!("{}", window_name))
            .size(INITIAL_WINDOW_SIZE, ImGuiCond::FirstUseEver)
            .scrollable(true)
            .position((prev_window_pos.0, prev_window_size.1 + prev_window_pos.1), ImGuiCond::FirstUseEver)
            .build(&|| {
                f();
                self.state.borrow_mut().prev_window_size = self.ui.get_window_size();
                self.state.borrow_mut().prev_window_pos = self.ui.get_window_pos();
            });
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

    fn draw_small_button<F: Fn() + 'static>(&self, label: &str, color: [f32; 4], on_button_activate: F) {
        self.ui.with_color_var(ImGuiCol::Button, color, || {
            if self.ui.button(im_str!("{}", label), BUTTON_SIZE) {
                on_button_activate()
            }
        })
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
        onchange(input.as_ref() as &str)
    }

    fn draw_main_menu_bar(&self, draw_menus: &Fn()) {
        self.ui.main_menu_bar(draw_menus)
    }

    fn draw_menu(&self, label: &str, draw_menu_items: &Fn()) {
        self.ui.menu(im_str!("{}", label)).build(draw_menu_items)
    }

    fn draw_menu_item<F: Fn() + 'static>(&self, label: &str, onselect: F) {
        if self.ui.menu_item(im_str!("{}", label)).build() {
            onselect()
        }
    }
}
