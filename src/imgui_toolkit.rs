use super::{App};
use super::ui_toolkit::{UiToolkit,SelectableItem};
use super::editor::{Keypress};
use super::imgui_support;
use super::async_executor;
use itertools::Itertools;

use imgui::*;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::hash_map::HashMap;

pub const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const TRANSPARENT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.0];
const BUTTON_SIZE: (f32, f32) = (0.0, 0.0);
const FIRST_WINDOW_PADDING: (f32, f32) = (25.0, 50.0);
const INITIAL_WINDOW_SIZE: (f32, f32) = (300.0, 200.0);

pub fn draw_app(app: Rc<RefCell<App>>, mut async_executor: async_executor::AsyncExecutor) {
    imgui_support::run("cs".to_string(), CLEAR_COLOR,
       |ui, keypress| {
            let mut app = app.borrow_mut();
            app.flush_commands(&mut async_executor);
            async_executor.turn();
            let mut toolkit = ImguiToolkit::new(ui, keypress);
            app.draw(&mut toolkit);
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
    let mut imstr = ImString::with_capacity(text.len() + 500);
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

    fn handle_global_keypress(&self, handle_keypress: impl Fn(Keypress) + 'static) {
        if let Some(keypress) = self.keypress {
            handle_keypress(keypress)
        }
    }

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

    fn indent(&self, px: i16, draw_fn: &Fn()) {
        unsafe { imgui_sys::igIndent(px as f32) }
        draw_fn();
        unsafe { imgui_sys::igUnindent(px as f32) }
    }

    fn align(&self, lhs: &Fn(), rhss: &[&Fn()]) {
        if rhss.is_empty() {
            return lhs();
        }
        if rhss.len() == 1 {
            return self.draw_all_on_same_line(&[lhs, rhss[0]]);
        }

        let (first_rhs, rest) = rhss.split_first().unwrap();
        let (last_rhs, inner_rhs) = rest.split_last().unwrap();

        unsafe { imgui_sys::igPushStyleVarVec2(imgui_sys::ImGuiStyleVar::ItemSpacing, (0.0, -1.0).into()) };
        self.ui.group(|| lhs());

        self.ui.same_line_spacing(0., 0.);
        let cursor_pos = unsafe { imgui_sys::igGetCursorPosX() };

        first_rhs();

        for draw in inner_rhs {
            unsafe { imgui_sys::igSetCursorPosX(cursor_pos) };
            draw();
        }
        unsafe { imgui_sys::igPopStyleVar(1) };
        unsafe { imgui_sys::igSetCursorPosX(cursor_pos) };
        last_rhs();
    }

    fn draw_centered_popup<F: Fn(Keypress) + 'static>(&self, draw_fn: &Fn(),
                                                      handle_keypress: Option<F>) -> Self::DrawResult {
        let (display_size_x, display_size_y) = self.ui.imgui().display_size();
        self.ui.window(&self.imlabel("draw_centered_popup"))
            .size(INITIAL_WINDOW_SIZE, ImGuiCond::Always)
            .position((display_size_x * 0.5, display_size_y * 0.5), ImGuiCond::Always)
            .position_pivot((0.5, 0.5))
            .resizable(false)
            .scrollable(true)
            .title_bar(false)
            .build(&|| {
                //unsafe { imgui_sys::igSetWindowFocus() };
                draw_fn();
                if let Some(keypress) = self.keypress {
                    if self.ui.is_window_focused() {
                        if let Some(ref handle_keypress) = handle_keypress {
                            handle_keypress(keypress)
                        }
                    }
                }
            });
    }

    fn draw_window<F: Fn(Keypress) + 'static, G: Fn() + 'static>(&self, window_name: &str, f: &Fn(),
                                                                 handle_keypress: Option<F>,
                                                                 onclose: Option<G>) {
        let prev_window_size = self.state.borrow().prev_window_size;
        let prev_window_pos = self.state.borrow().prev_window_pos;

        let mut should_stay_open = true;

        let window_name = self.imlabel(window_name);
        let mut window_builder = self.ui.window(&window_name)
            .size(INITIAL_WINDOW_SIZE, ImGuiCond::FirstUseEver)
            .scrollable(true)
            .position((prev_window_pos.0, prev_window_size.1 + prev_window_pos.1), ImGuiCond::FirstUseEver);

        if onclose.is_some() {
            window_builder = window_builder.opened(&mut should_stay_open);
        }

        window_builder.build(&|| {
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

        if let Some(onclose) = onclose {
            if !should_stay_open {
                onclose()
            }
        }
    }

    fn draw_child_region<F: Fn(Keypress) + 'static>(&self, draw_fn: &Fn(), height_percentage: f32, handle_keypress: Option<F>) {
        let height = height_percentage * unsafe { imgui_sys::igGetWindowHeight() };
        self.ui.child_frame(&self.imlabel(""), (0., height))
            .show_borders(true)
            .build(&|| {
                draw_fn();

                if let Some(keypress) = self.keypress {
                    if self.ui.is_child_window_focused() {
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

    fn draw_box_around(&self, color: [f32; 4], draw_fn: &Fn()) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui.get_window_draw_list()
            .add_rect(min, max, color)
            .filled(true)
            .build();
    }

    fn draw_top_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &Fn()) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui.get_window_draw_list()
            .add_rect(min, (max.x, min.y + thickness as f32 - 1.), color)
            .thickness(1.)
            .filled(true)
            .build();
    }

    fn draw_right_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &Fn()) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui.get_window_draw_list()
            .add_rect((max.x - thickness as f32, min.y), max, color)
            .thickness(1.)
            .filled(true)
            .build()
    }

    fn draw_left_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &Fn()) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui.get_window_draw_list()
            .add_rect(min, (min.x - thickness as f32, max.y), color)
            .thickness(1.)
            .filled(true)
            .build()
    }

    fn draw_bottom_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &Fn()) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
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
        unsafe { imgui_sys::igSetScrollHereY(1.0) };
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

    fn draw_selectables<F, G, H, T>(&self, is_item_selected: G, format_item: H, items: &[&T], onchange: F)
        where T: 'static,
              F: Fn(&T) -> () + 'static,
              G: Fn(&T) -> bool,
              H: Fn(&T) -> &str {
        for item in items {
            if self.ui.selectable(&self.imlabel(&format_item(item)),
                                  is_item_selected(item),
                                  ImGuiSelectableFlags::empty(),
                                  (0., 0.)) {
                onchange(item)
            }
        }
    }

    fn draw_selectables2<T, F: Fn(&T) -> () + 'static>(&self, items: Vec<SelectableItem<T>>, onselect: F) -> Self::DrawResult {
        for selectable in items {
            match selectable {
                SelectableItem::GroupHeader(label) => {
                    self.draw_all_on_same_line(&[
                        &|| self.draw_text("-" ),
                        &|| self.draw_text(label),
                    ])
                },
                SelectableItem::Selectable { item, label, is_selected } => {
                    if self.ui.selectable(&self.imlabel(&label),
                                          is_selected,
                                          ImGuiSelectableFlags::empty(),
                                          (0., 0.)) {
                        onselect(&item)
                    }
                }
            }
        }
    }

    fn draw_checkbox_with_label<F: Fn(bool) + 'static>(&self, label: &str, value: bool, onchange: F) {
        let mut val = value;
        self.ui.checkbox(&self.imlabel(label), &mut val);
        if val != value {
            onchange(val);
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

//    fn draw_tree_node(&self, label: &str, draw_fn: &Fn()) {
//        self.ui.tree_node(im_str!("{}", label)).leaf(false).build(draw_fn)
//    }
//
//    fn draw_tree_leaf(&self, label: &str, draw_fn: &Fn()) {
//        self.ui.tree_node(im_str!("{}", label)).leaf(true).build(draw_fn)
//    }
}
