use super::{CSApp};
use super::editor::{UiToolkit};
use super::editor::{Keypress};
use super::imgui_support;
use itertools::Itertools;

use imgui::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::hash_map::HashMap;

use tokio::spawn_async;

pub const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const TRANSPARENT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.0];
const BUTTON_SIZE: (f32, f32) = (0.0, 0.0);
const FIRST_WINDOW_PADDING: (f32, f32) = (25.0, 50.0);
const INITIAL_WINDOW_SIZE: (f32, f32) = (300.0, 200.0);

pub fn draw_app(app: Rc<CSApp>) {
    imgui_support::run("cs".to_string(), CLEAR_COLOR,
       |ui, keypress| {
            let mut toolkit = ImguiToolkit::new(ui, keypress);
            app.draw(&mut toolkit);
//            app.async_executor.borrow_mut().turn();
            true
        },
    );
}

struct State {
    prev_window_size: (f32, f32),
    prev_window_pos: (f32, f32),
    used_labels: HashMap<String,i32>,

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
            used_labels: HashMap::new(),
        }
    }
}

pub struct ImguiToolkit<'a> {
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

    // stupid, but it works. imgui does this stupid shit where if you use the same label for two
    // separate buttons, it won't detect clicks on the second button. labels have to be unique. but
    // it supports suffixing labels with ##<UNIQUE_STUFF> so we're taking advantage of that here. so
    // on every draw, we'll just keep track of how many times we use each label and increment so no
    // two labels are the same!
    fn imlabel(&self, str: &str) -> ImString {
        let map = &mut self.state.borrow_mut().used_labels;
        let label_count = map.entry(str.to_string()).or_insert(0);
        let label = im_str!("{}##{}", str, label_count);
        *label_count += 1;
        // XXX: not sure why we have to clone this label, but rust will NOT let me dereference it
        label.clone()
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
                self.ui.window(&self.imlabel("statusbar"))
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
        self.ui.with_color_vars(
            &[(ImGuiCol::ButtonHovered, TRANSPARENT_COLOR),
              (ImGuiCol::ButtonActive, TRANSPARENT_COLOR),
            ],
            &|| { self.draw_button(text, TRANSPARENT_COLOR, &||{}) }
        )
    }

    fn draw_text_with_label(&self, text: &str, label: &str) -> Self::DrawResult {
        self.ui.label_text(im_str!("{}", label), im_str!("{}", text))
    }

    fn draw_all_on_same_line(&self, draw_fns: &[&Fn()]) {
        if let Some((last_draw_fn, first_draw_fns)) = draw_fns.split_last() {
            for draw_fn in first_draw_fns {
                draw_fn();
                self.ui.same_line_spacing(0.0, 0.0);
            }
            last_draw_fn();
        }
    }

    fn draw_window<F: Fn(Keypress) + 'static>(&self, window_name: &str, f: &Fn(),
                                              handle_keypress: Option<F>) {
        let prev_window_size = self.state.borrow().prev_window_size;
        let prev_window_pos = self.state.borrow().prev_window_pos;

        self.ui.window(&self.imlabel(window_name))
            .size(INITIAL_WINDOW_SIZE, ImGuiCond::FirstUseEver)
            .scrollable(true)
            .position((prev_window_pos.0, prev_window_size.1 + prev_window_pos.1), ImGuiCond::FirstUseEver)
            .build(&|| {
                f();
                self.state.borrow_mut().prev_window_size = self.ui.get_window_size();
                self.state.borrow_mut().prev_window_pos = self.ui.get_window_pos();

                if let Some(keypress) = self.keypress {
                    if self.ui.is_window_focused() {
                        if let Some(ref handle_keypress) = handle_keypress {
                            handle_keypress(keypress)
                        }
                    }
                }
            });
    }

    fn draw_layout_with_bottom_bar(&self, draw_content_fn: &Fn(), draw_bottom_bar_fn: &Fn()) {
        let frame_height = unsafe { imgui_sys::igGetFrameHeightWithSpacing() };
        self.ui.child_frame(&self.imlabel(""), (0.0, -frame_height))
            .build(draw_content_fn);
        draw_bottom_bar_fn()
    }

    fn draw_separator(&self) {
        self.ui.separator();
    }

    fn draw_empty_line(&self) {
        self.ui.new_line();
    }

    // TODO: draw my own border using the draw list + a group... then it'll work right. jeez
    fn draw_box_around(&self, color: [f32; 4], draw_fn: &Fn()) {
        self.ui.group(draw_fn);
        let mut min = ImVec2::zero();
        let mut max = ImVec2::zero();
        unsafe { imgui_sys::igGetItemRectMin(&mut min) };
        unsafe { imgui_sys::igGetItemRectMax(&mut max) };
        self.ui.get_window_draw_list()
            .add_rect(min, max, color)
            .filled(true)
            .build();
    }

    fn draw_top_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &Fn()) {
        self.ui.group(draw_fn);
        let mut min = ImVec2::zero();
        let mut max = ImVec2::zero();
        unsafe { imgui_sys::igGetItemRectMin(&mut min) };
        unsafe { imgui_sys::igGetItemRectMax(&mut max) };
        self.ui.get_window_draw_list()
            .add_rect(min, (max.x, min.y + thickness as f32 - 1.), color)
            .thickness(1.)
            .filled(true)
            .build();
    }

    fn draw_right_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &Fn()) {
        self.ui.group(draw_fn);
        let mut min = ImVec2::zero();
        let mut max = ImVec2::zero();
        unsafe { imgui_sys::igGetItemRectMin(&mut min) };
        unsafe { imgui_sys::igGetItemRectMax(&mut max) };
        self.ui.get_window_draw_list()
            .add_rect((max.x - thickness as f32, min.y), max, color)
            .thickness(1.)
            .filled(true)
            .build()
    }

    fn draw_bottom_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &Fn()) {
        self.ui.group(draw_fn);
        let mut min = ImVec2::zero();
        let mut max = ImVec2::zero();
        unsafe { imgui_sys::igGetItemRectMin(&mut min) };
        unsafe { imgui_sys::igGetItemRectMax(&mut max) };
        self.ui.get_window_draw_list()
            .add_rect((min.x, max.y - thickness as f32), max, color)
            .thickness(1.)
            .filled(true)
            .build()
    }

    fn draw_button<F: Fn() + 'static>(&self, label: &str, color: [f32; 4], on_button_activate: F) {
        self.ui.with_color_var(ImGuiCol::Button, color, || {
            if self.ui.button(&self.imlabel(label), BUTTON_SIZE) {
                on_button_activate()
            }
        });
    }

    // XXX: why do i have the small button look like a normal button again????
    // maybe it's because the code icons looked like this
    fn draw_small_button<F: Fn() + 'static>(&self, label: &str, color: [f32; 4], on_button_activate: F) {
        self.ui.with_color_var(ImGuiCol::Button, color, || {
            if self.ui.small_button(&self.imlabel(label)) {
                on_button_activate()
            }
        })
    }

    fn draw_text_box(&self, text: &str) {
        self.ui.text(text);
        // GHETTO: text box is always scrolled to the bottom
        unsafe { imgui_sys::igSetScrollHere(1.0) };
    }

    fn draw_text_input<F: Fn(&str) -> () + 'static, D: Fn() + 'static>(&self, existing_value: &str,
                                                                       onchange: F, ondone: D) {
        self.draw_text_input_with_label("", existing_value, onchange, ondone)
    }

    fn draw_multiline_text_input_with_label<F: Fn(&str) -> () + 'static>(&self, label: &str, existing_value: &str, onchange: F) {
        let mut box_input = buf(existing_value);
        self.ui.input_text_multiline(&self.imlabel(label), &mut box_input, (0., 100.)).build();
        if box_input.as_ref() as &str != existing_value {
            onchange(box_input.as_ref() as &str)
        }
    }

    fn draw_text_input_with_label<F: Fn(&str) -> () + 'static, D: Fn() + 'static>(&self, label: &str, existing_value: &str, onchange: F, ondone: D) {
        let mut box_input = buf(existing_value);

        let mut flags = ImGuiInputTextFlags::empty();
        flags.set(ImGuiInputTextFlags::EnterReturnsTrue, true);

        let enter_pressed = self.ui.input_text(&self.imlabel(label), &mut box_input)
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

    fn draw_combo_box_with_label<F, G, H, T>(&self, label: &str, is_item_selected: G,
                                              format_item: H, items: &[&T],
                                              onchange: F) -> Self::DrawResult
        where T: Clone,
              F: Fn(&T) -> () + 'static,
              G: Fn(&T) -> bool,
              H: Fn(&T) -> String {
        let mut selected_item_in_combo_box = items.into_iter()
            .position(|i| is_item_selected(i)).unwrap() as i32;
        let previous_selection = selected_item_in_combo_box.clone();

        let formatted_items = items.into_iter()
                .map(|s| im_str!("{}", format_item(s)).clone())
                .collect_vec();

        self.ui.combo(
            &self.imlabel(label),
            &mut selected_item_in_combo_box,
            &formatted_items.iter().map(|s| s.as_ref()).collect_vec(),
            5
        );
        if selected_item_in_combo_box != previous_selection {
            onchange(items[selected_item_in_combo_box as usize])
        }
    }

    fn draw_main_menu_bar(&self, draw_menus: &Fn()) {
        self.ui.main_menu_bar(draw_menus)
    }

    fn draw_menu(&self, label: &str, draw_menu_items: &Fn()) {
        self.ui.menu(&self.imlabel(label)).build(draw_menu_items)
    }

    fn draw_menu_item<F: Fn() + 'static>(&self, label: &str, onselect: F) {
        if self.ui.menu_item(&self.imlabel(label)).build() {
            onselect()
        }
    }
}
