use super::{CSApp};
use super::editor::{UiToolkit};
use super::editor::{Key,Keypress};
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
       |ui, keypress| {
            let mut toolkit = ImguiToolkit::new(ui, keypress);
            app.draw(&mut toolkit);
            true
        },
    );
}

struct State {
    prev_window_size: (f32, f32),
    prev_window_pos: (f32, f32),
}

fn buf(text: &str) -> ImString {
    let mut imstr = ImString::with_capacity(100);
    imstr.push_str(text);
    imstr
}

impl State {
    fn new() -> Self {
        State {
            prev_window_pos: FIRST_WINDOW_PADDING,
            prev_window_size: (0.0, 0.0),
        }
    }
}

struct ImguiToolkit<'a> {
    ui: &'a Ui<'a>,
    keypress: Option<Keypress>,
    state: RefCell<State>,
}

impl<'a> ImguiToolkit<'a> {
    pub fn new(ui: &'a Ui, keypress: Option<Keypress>) -> ImguiToolkit<'a> {
        ImguiToolkit {
            ui: ui,
            keypress,
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

    fn draw_window<F: Fn(Keypress)>(&self, window_name: &str, f: &Fn(), handle_keypress: F) {
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

                if let(Some(keypress)) = self.keypress {
                    if self.ui.is_window_focused() {
                        handle_keypress(keypress)
                    }
                }
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
        self.draw_text_input_with_label("", existing_value, onchange, ondone)
    }

    fn draw_multiline_text_input_with_label<F: Fn(&str) -> () + 'static>(&self, label: &str, existing_value: &str, onchange: F) {
        let mut box_input = buf(existing_value);
        self.ui.input_text_multiline(im_str!("{}", label), &mut box_input, (0., 100.)).build();
        if box_input.as_ref() as &str != existing_value {
            onchange(box_input.as_ref() as &str)
        }
    }

    fn draw_text_input_with_label<F: Fn(&str) -> () + 'static, D: Fn() + 'static>(&self, label: &str, existing_value: &str, onchange: F, ondone: D) {
        let mut box_input = buf(existing_value);

        let mut flags = ImGuiInputTextFlags::empty();
        flags.set(ImGuiInputTextFlags::EnterReturnsTrue, true);

        let enter_pressed = self.ui.input_text(im_str!("{}", label), &mut box_input)
            .flags(flags)
            .build();
        if enter_pressed {
            ondone();
            return
        }

        if box_input.as_ref() as &str != existing_value {
            onchange(box_input.as_ref() as &str)
        }
    }

    fn draw_combo_box_with_label<F: Fn(i32) -> () + 'static>(&self, label: &str, current_item: i32, items: &[&str], onchange: F) {
        let mut selected_item_in_combo_box = current_item.clone();
        let items : Vec<ImString> = items.iter().map(|s| im_str!("{}", s).clone()).collect();
        let items : Vec<&ImStr> = items.iter().map(|s| s.as_ref()).collect();
        self.ui.combo(
            im_str!("{}", label),
            &mut selected_item_in_combo_box,
            &items,
            5
        );
        if selected_item_in_combo_box != current_item {
            onchange(selected_item_in_combo_box)
        }
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
