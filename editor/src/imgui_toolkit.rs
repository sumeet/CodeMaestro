use super::app::App;
use super::async_executor;
use super::editor::Keypress;
use super::imgui_support;
use super::ui_toolkit::{Color, SelectableItem, UiToolkit};
use nfd;

use crate::colorscheme;
use crate::ui_toolkit::{ChildRegionHeight, DrawFnRef};
use imgui::*;
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::collections::hash_map::HashMap;
use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

// forgot why we need this exactly, but it needs to be passed in some places to imgui support
const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.0];
const BUTTON_SIZE: [f32; 2] = [0.0, 0.0];
const INITIAL_WINDOW_SIZE: [f32; 2] = [400.0, 500.0];
// from http://jkorpela.fi/chars/spaces.html, used for indentation
const SPACE: &'static str = "\u{3000}";

lazy_static! {
    static ref TK_CACHE: Arc<Mutex<TkCache>> = Arc::new(Mutex::new(TkCache::new()));
    static ref SPACE_IMSTR: ImString = im_str! { "{}", SPACE }.clone();
}

#[derive(Clone, Copy, PartialEq, Debug)]
struct Window {
    pos: (f32, f32),
    size: (f32, f32),
    // the amount of screen space the content inside takes, this could actually be bigger
    // than our size, in which case scrollbars would activate if enabled on the window
    content_size: (f32, f32),
    remaining_size: (f32, f32),
    flex: u8,
}

//enum ScrollStatus {
//    FirstFocus,
//    AlreadyFocused,
//}

struct ChildRegion {
    is_focused: bool,
    content_height: f32,
    height: f32,
}

impl ChildRegion {
    fn new(is_focused: bool, height: f32, content_height: f32) -> Self {
        Self { is_focused,
               height,
               content_height }
    }
}

struct TkCache {
    current_window_label: Option<ImString>,
    child_regions: HashMap<String, ChildRegion>,
    windows: HashMap<String, Window>,
    replace_on_hover: HashMap<String, bool>,
    //scroll_for_keyboard_nav: HashMap<String, ScrollStatus>,
    elements_focused_in_prev_iteration: HashSet<String>,
    elements_focused_in_this_iteration: HashSet<String>,
    current_flex: u8,
}

impl TkCache {
    fn new() -> Self {
        Self { child_regions: HashMap::new(),
               replace_on_hover: HashMap::new(),
               current_window_label: None,
               current_flex: 0,
               windows: HashMap::new(),
               //               scroll_for_keyboard_nav: HashMap::new(),
               elements_focused_in_prev_iteration: HashSet::new(),
               elements_focused_in_this_iteration: HashSet::new() }
    }

    pub fn cleanup_after_iteration() {
        let mut cache = TK_CACHE.lock().unwrap();
        cache.elements_focused_in_prev_iteration = cache.elements_focused_in_this_iteration.clone();
        cache.elements_focused_in_this_iteration.clear();
    }

    pub fn get_window(window_name_str: &str) -> Option<Window> {
        TK_CACHE.lock()
                .unwrap()
                .windows
                .get(window_name_str)
                .cloned()
    }

    pub fn is_new_focus_for_scrolling(scroll_hash: String) -> bool {
        let mut cache = TK_CACHE.lock().unwrap();
        let is_new_focus = !cache.elements_focused_in_prev_iteration
                                 .contains(&scroll_hash);
        cache.elements_focused_in_this_iteration.insert(scroll_hash);
        is_new_focus
    }

    pub fn set_current_window_label(label: &ImString) {
        TK_CACHE.lock().unwrap().current_window_label = Some(label.clone())
    }

    pub fn set_current_window_flex(flex: u8) {
        TK_CACHE.lock().unwrap().current_flex = flex
    }

    pub fn clear_current_window_label() {
        TK_CACHE.lock().unwrap().current_window_label.take();
    }

    pub fn get_current_window() -> Option<Window> {
        let cache = TK_CACHE.lock().unwrap();
        let current_window_label = cache.current_window_label.as_ref()?;
        let current_window_label: &str = current_window_label.as_ref();
        cache.windows.get(current_window_label).cloned()
    }

    pub fn is_focused(child_window_id: &str) -> bool {
        *TK_CACHE.lock()
                 .unwrap()
                 .child_regions
                 .get(child_window_id)
                 .map(|child_region| &child_region.is_focused)
                 .unwrap_or(&false)
    }

    pub fn set_child_region_info(child_window_id: &str, child_region: ChildRegion, flex: u8) {
        let mut cache = TK_CACHE.lock().unwrap();
        cache.child_regions
             .insert(child_window_id.to_string(), child_region);
        cache.current_flex += flex;
    }

    pub fn get_child_region_height(child_window_id: &str) -> Option<f32> {
        TK_CACHE.lock()
                .unwrap()
                .child_regions
                .get(child_window_id)
                .map(|cr| cr.height)
    }

    pub fn get_child_region_content_height(child_window_id: &str) -> Option<f32> {
        TK_CACHE.lock()
                .unwrap()
                .child_regions
                .get(child_window_id)
                .map(|cr| cr.content_height)
    }

    pub fn is_hovered(label: &str) -> bool {
        *TK_CACHE.lock()
                 .unwrap()
                 .replace_on_hover
                 .get(label)
                 .unwrap_or(&false)
    }

    pub fn set_is_hovered(label: String, is_hovered: bool) {
        TK_CACHE.lock()
                .unwrap()
                .replace_on_hover
                .insert(label, is_hovered);
    }
}

pub fn draw_app(app: Rc<RefCell<App>>, mut async_executor: async_executor::AsyncExecutor) {
    imgui_support::run("cs".to_string(), move |ui, keypress| {
        let mut app = app.borrow_mut();
        app.flush_commands(&mut async_executor);
        async_executor.turn();
        let mut toolkit = ImguiToolkit::new(ui, keypress);
        app.draw(&mut toolkit);

        TkCache::cleanup_after_iteration();

        true
    });
}

struct State {
    used_labels: HashMap<String, i32>,
}

fn buf(text: &str) -> ImString {
    let mut imstr = ImString::with_capacity(text.len() + 500);
    imstr.push_str(text);
    imstr
}

impl State {
    fn new() -> Self {
        State { used_labels: HashMap::new() }
    }
}

pub struct ImguiToolkit<'a> {
    ui: &'a Ui<'a>,
    keypress: Option<Keypress>,
    state: RefCell<State>,
}

impl<'a> ImguiToolkit<'a> {
    pub fn new(ui: &'a Ui, keypress: Option<Keypress>) -> ImguiToolkit<'a> {
        ImguiToolkit { ui,
                       keypress,
                       state: RefCell::new(State::new()) }
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

    // HAXXXXX
    fn is_last_drawn_item_totally_visible(&self) -> bool {
        let window_min = self.ui.window_pos();
        let window_size = self.ui.window_size();
        let window_max = (window_min[0] + window_size[0], window_min[1] + window_size[1]);

        let item_min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let item_max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };

        (window_min[0] <= item_min.x)
        && (item_min.x <= window_max.0)
        && (window_min[0] <= item_max.x)
        && (item_max.x <= window_max.0)
        && (window_min[1] <= item_min.y)
        && (item_min.y <= window_max.1)
        && (window_min[1] <= item_max.y)
        && (item_max.y <= window_max.1)
    }

    fn mouse_clicked_in_last_drawn_element(&self) -> bool {
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() }.into();
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() }.into();
        let mouse_pos = self.ui.io().mouse_pos;
        self.is_left_button_down() && Rect { min, max }.contains(mouse_pos)
    }

    fn mouse_released_in_last_drawn_element(&self) -> bool {
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() }.into();
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() }.into();
        let mouse_pos = self.ui.io().mouse_pos;
        self.was_left_button_released() && Rect { min, max }.contains(mouse_pos)
    }

    fn was_left_button_released(&self) -> bool {
        self.ui.is_mouse_released(MouseButton::Left)
    }

    fn is_left_button_down(&self) -> bool {
        self.ui.is_mouse_down(MouseButton::Left)
    }

    fn make_last_item_look_active(&self) {
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui
            .get_window_draw_list()
            .add_rect(min.into(), max.into(), colorscheme!(button_active_color))
            .filled(true)
            .build();
    }
}

struct Rect {
    min: (f32, f32),
    max: (f32, f32),
}

impl Rect {
    pub fn contains(&self, p: [f32; 2]) -> bool {
        return self.min.0 <= p[0]
               && p[0] <= self.max.0
               && self.min.1 <= p[1]
               && p[1] <= self.max.1;
    }
}

impl<'a> UiToolkit for ImguiToolkit<'a> {
    type DrawResult = ();

    //    fn draw_in_columns(&self, columns: &[Column<Self>]) {
    //        if columns.len() <= 1 {
    //            for column in columns {
    //                (column.draw_fn)();
    //            }
    //            return;
    //        }
    //
    //        let total_width = unsafe { imgui_sys::igGetContentRegionAvailWidth() };
    //        self.ui
    //            .columns(columns.len() as i32, &self.imlabel("columns"), false);
    //        columns.split_last().map(|(last_column, starting_columns)| {
    //                                for column in starting_columns {
    //                                    unsafe {
    //                                        imgui_sys::igSetColumnWidth(-1,
    //                                                                    column.percentage * total_width)
    //                                    };
    //                                    (column.draw_fn)();
    //                                    self.ui.next_column();
    //                                }
    //                                unsafe {
    //                                    imgui_sys::igSetColumnWidth(-1,
    //                                                                last_column.percentage
    //                                                                * total_width)
    //                                };
    //                                (last_column.draw_fn)();
    //                            });
    //
    //        // set back to a single column
    //        self.ui.columns(1, &self.imlabel("columnsend"), false)
    //    }
    fn open_file_open_dialog(callback: impl Fn(&[u8]) + 'static) {
        let result = nfd::open_file_dialog(None, None).unwrap();
        let filename = match result {
            nfd::Response::Okay(file_path) => Some(file_path),
            nfd::Response::OkayMultiple(file_paths) => file_paths.into_iter().nth(0),
            nfd::Response::Cancel => None,
        };
        if filename.is_none() {
            return;
        }
        let filename = filename.unwrap();
        let mut read_buffer = Vec::new();
        File::open(&filename).unwrap()
                             .read_to_end(&mut read_buffer)
                             .unwrap();
        callback(&read_buffer);
    }

    fn open_file_save_dialog(_filename_suggestion: &str, contents: &[u8], _mimetype: &str) {
        let result = nfd::open_save_dialog(None, None).unwrap();
        let filename = match result {
            nfd::Response::Okay(file_path) => Some(file_path),
            nfd::Response::OkayMultiple(file_paths) => file_paths.into_iter().nth(0),
            nfd::Response::Cancel => None,
        };
        if filename.is_none() {
            return;
        }
        let filename = filename.unwrap();
        File::create(&filename).unwrap()
                               .write_all(&contents)
                               .unwrap();
    }

    fn draw_wrapped_text(&self, color: Color, text: &str) {
        // TODO: explain wtf all this code is for!!!
        // XXX: we didn't need this clone before...
        let style = self.ui.clone_style();
        let padding = style.frame_padding;
        let x_padding = padding[0];

        let current_cursor_pos = self.ui.cursor_pos();

        let x_pos_to_indent_first_line = current_cursor_pos[0] + x_padding;

        let width_of_one_space = self.ui.calc_text_size(&SPACE_IMSTR, false, 0.)[0];
        let number_of_spaces_to_prepad = (x_pos_to_indent_first_line / width_of_one_space).floor();
        let text = SPACE.repeat(number_of_spaces_to_prepad as usize) + text;

        self.ui
            .set_cursor_pos([x_padding * 2., current_cursor_pos[1]]);
        //unsafe { imgui_sys::igAlignTextToFramePadding() };

        let token = self.ui.push_text_wrap_pos(0.);
        let token2 = self.ui.push_style_color(StyleColor::Text, color);
        self.ui.text(&text);
        token2.pop(self.ui);
        token.pop(self.ui);
    }

    fn scrolled_to_y_if_not_visible(&self, scroll_hash: String, draw_fn: &dyn Fn()) {
        self.ui.group(draw_fn);
        // TODO: get rid of clone
        let is_first_focus = TkCache::is_new_focus_for_scrolling(scroll_hash.clone());
        if !self.is_last_drawn_item_totally_visible() && is_first_focus {
            unsafe { imgui_sys::igSetScrollHereY(1.) }
        }
    }

    fn handle_global_keypress(&self, handle_keypress: impl Fn(Keypress) + 'static) {
        if let Some(keypress) = self.keypress {
            handle_keypress(keypress)
        }
    }

    fn draw_all(&self, draw_fns: &[DrawFnRef<Self>]) {
        draw_fns.iter().for_each(|draw_fn| draw_fn())
    }

    fn focused(&self, draw_fn: &dyn Fn()) {
        draw_fn();
        unsafe {
            // HACK: the igIsAnyItemHovered allows me to click buttons while focusing a text field.
            // good enough for now.
            if self.ui.is_item_hovered()
               || (!imgui_sys::igIsAnyItemActive() && !imgui_sys::igIsAnyItemHovered())
            {
                imgui_sys::igSetKeyboardFocusHere(-1)
            }
        }
    }

    fn buttonize<F: Fn() + 'static>(&self, draw_fn: &dyn Fn(), onclick: F) {
        self.ui.group(draw_fn);

        // grabbed this code from draw_box_around
        if self.ui.is_item_hovered() {
            let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
            let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
            self.ui
                .get_window_draw_list()
                .add_rect(min.into(), max.into(), colorscheme!(button_hover_color))
                .filled(true)
                .build();
        }

        if self.mouse_clicked_in_last_drawn_element() {
            self.make_last_item_look_active();
        }

        if self.mouse_released_in_last_drawn_element() {
            onclick()
        }
    }

    fn draw_statusbar(&self, draw_fn: &dyn Fn()) {
        let x_padding = 4.0;
        let y_padding = 5.0;
        let font_size = unsafe { imgui_sys::igGetFontSize() };
        // cribbed status bar implementation from
        // https://github.com/ocornut/imgui/issues/741#issuecomment-233288320
        let status_height = (y_padding * 2.0) + font_size;

        let display_size = self.ui.io().display_size;
        let window_pos = [0.0, display_size[1] - status_height];
        let window_size = [display_size[0], status_height];

        let token = self.ui.push_style_vars(&[StyleVar::WindowRounding(0.0),
                                              StyleVar::WindowPadding([x_padding, y_padding])]);
        imgui::Window::new(&self.imlabel("statusbar")).collapsible(false)
                                                      .horizontal_scrollbar(false)
                                                      .scroll_bar(false)
                                                      .scrollable(false)
                                                      .resizable(false)
                                                      .always_auto_resize(false)
                                                      .title_bar(false)
                                                      .focus_on_appearing(false)
                                                      .movable(false)
                                                      .bring_to_front_on_focus(false)
                                                      .position(window_pos, Condition::Always)
                                                      .size(window_size, Condition::Always)
                                                      .build(self.ui, draw_fn);
        token.pop(self.ui);
    }

    fn draw_text(&self, text: &str) {
        let token = self.ui
                        .push_style_colors(&[(StyleColor::ButtonHovered, CLEAR_COLOR),
                                             (StyleColor::ButtonActive, CLEAR_COLOR)]);
        self.draw_button(text, CLEAR_COLOR, &|| {});
        token.pop(self.ui);
    }

    fn draw_taking_up_full_width(&self, draw_fn: DrawFnRef<Self>) {
        let style = self.ui.clone_style();
        let frame_padding = style.frame_padding;

        let orig_cursor_pos = self.ui.cursor_pos();
        self.ui.group(draw_fn);
        self.ui.set_cursor_pos(orig_cursor_pos);

        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        let width = unsafe { imgui_sys::igGetContentRegionMax_nonUDT2().x } - frame_padding[0] * 2.;
        self.ui.dummy([width, max.y - min.y])
    }

    fn draw_with_bgcolor(&self, bgcolor: Color, draw_fn: DrawFnRef<Self>) {
        // haxxx: draw twice:
        // first to get the bounding box, then draw over it with the bgcolor, and then
        // draw over it again
        let orig_cursor_pos = self.ui.cursor_pos();

        self.ui.group(draw_fn);

        let style = self.ui.clone_style();
        let mut blankoutbgcolor = style.colors[StyleColor::FrameBg as usize];
        // if framebg color is transparent, then make it opaque
        blankoutbgcolor[3] = 1.;

        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        // without the scope here we get a crash for loading drawlist twice, idk what the deal is
        // tbh
        {
            let drawlist = self.ui.get_window_draw_list();
            drawlist.add_rect(min.into(), max.into(), blankoutbgcolor)
                    .filled(true)
                    .build();
            drawlist.add_rect(min.into(), max.into(), bgcolor)
                    .filled(true)
                    .build();
        }

        self.ui.set_cursor_pos(orig_cursor_pos);
        draw_fn();
    }

    fn draw_with_no_spacing_afterwards(&self, draw_fn: DrawFnRef<Self>) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui.set_cursor_screen_pos([min.x, max.y]);
    }

    fn draw_full_width_heading(&self, bgcolor: Color, inner_padding: (f32, f32), text: &str) {
        // copy and paste of draw_buttony_text lol
        let style = self.ui.clone_style();
        let padding = style.frame_padding;

        //let full_width = unsafe { imgui_sys::igGetContentRegionAvailWidth() };
        let full_width = unsafe { imgui_sys::igGetContentRegionAvail_nonUDT2().x };

        let original_cursor_pos = self.ui.cursor_pos();
        let label = im_str!("{}", text);
        let text_size = self.ui.calc_text_size(&label, false, 0.);
        let total_size = [full_width,
                          text_size[1] + (padding[1] * 2.) + (inner_padding.1 * 2.)];

        let draw_cursor_pos = self.ui.cursor_screen_pos();
        let end_of_button_bg_rect = [draw_cursor_pos[0] + total_size[0],
                                     draw_cursor_pos[1] + total_size[1]];
        self.ui
            .get_window_draw_list()
            .add_rect(draw_cursor_pos, end_of_button_bg_rect, bgcolor)
            .filled(true)
            .build();

        let draw_cursor_pos = self.ui.cursor_screen_pos();

        let buttony_text_start_cursor_pos = [draw_cursor_pos[0] + padding[0] + inner_padding.0,
                                             draw_cursor_pos[1] + padding[1] + inner_padding.1];

        let draw_list = self.ui.get_window_draw_list();
        let text_color = style.colors[StyleColor::Text as usize];
        draw_list.add_text(buttony_text_start_cursor_pos, text_color, label);
        self.ui.set_cursor_pos(original_cursor_pos);

        self.ui.invisible_button(&self.imlabel(""), total_size);
    }

    fn draw_with_margin(&self, margin: (f32, f32), draw_fn: DrawFnRef<Self>) {
        let orig_cursor_pos = self.ui.cursor_pos();

        let mut starting_point_for_cursor = orig_cursor_pos;
        starting_point_for_cursor[0] += margin.0 / 2.;
        starting_point_for_cursor[1] += margin.1 / 2.;
        self.ui.set_cursor_pos(starting_point_for_cursor);
        self.ui.group(draw_fn);
        let drawn_rect_size = self.ui.item_rect_size();

        self.ui.set_cursor_pos(orig_cursor_pos);
        self.ui
            .dummy([drawn_rect_size[0] + margin.0, drawn_rect_size[1] + margin.1])
    }

    fn draw_text_with_label(&self, text: &str, label: &str) -> Self::DrawResult {
        self.ui
            .label_text(&im_str!("{}", label), &im_str!("{}", text))
    }

    fn draw_all_on_same_line(&self, draw_fns: &[&dyn Fn()]) {
        if let Some((last_draw_fn, first_draw_fns)) = draw_fns.split_last() {
            for draw_fn in first_draw_fns {
                draw_fn();
                self.ui.same_line_with_spacing(0.0, 0.0);
            }
            last_draw_fn();
        }
    }

    fn indent(&self, px: i16, draw_fn: &dyn Fn()) {
        unsafe { imgui_sys::igIndent(px as f32) }
        draw_fn();
        unsafe { imgui_sys::igUnindent(px as f32) }
    }

    fn align(&self, lhs: &dyn Fn(), rhss: &[&dyn Fn()]) {
        if rhss.is_empty() {
            return lhs();
        }
        if rhss.len() == 1 {
            return self.draw_all_on_same_line(&[lhs, rhss[0]]);
        }

        let (first_rhs, rest) = rhss.split_first().unwrap();
        let (last_rhs, inner_rhs) = rest.split_last().unwrap();

        let style_var = self.ui.push_style_var(StyleVar::ItemSpacing([0.0, -1.0]));
        self.ui.group(|| lhs());

        self.ui.same_line_with_spacing(0., 0.);
        let cursor_pos = unsafe { imgui_sys::igGetCursorPosX() };

        first_rhs();

        for draw in inner_rhs {
            unsafe { imgui_sys::igSetCursorPosX(cursor_pos) };
            draw();
        }
        // this pops the style var
        style_var.pop(self.ui);
        unsafe { imgui_sys::igSetCursorPosX(cursor_pos) };
        last_rhs();
    }

    fn draw_centered_popup<F: Fn(Keypress) + 'static>(&self,
                                                      draw_fn: &dyn Fn(),
                                                      handle_keypress: Option<F>)
                                                      -> Self::DrawResult {
        let [display_size_x, display_size_y] = self.ui.io().display_size;
        imgui::Window::new(&self.imlabel("draw_centered_popup"))
            .size(INITIAL_WINDOW_SIZE, Condition::Always)
            .position([display_size_x * 0.5, display_size_y * 0.5],
                      Condition::Always)
            .position_pivot([0.5, 0.5])
            .resizable(false)
            .scrollable(true)
            .title_bar(false)
            .build(self.ui, &|| {
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

    // stolen from https://github.com/ocornut/imgui/blob/master/imgui_demo.cpp#L3944
    fn draw_top_right_overlay(&self, draw_fn: &dyn Fn()) {
        let distance = 10.0;
        let [display_size_x, _] = self.ui.io().display_size;
        imgui::Window::new(&self.imlabel("top_right_overlay")).flags(WindowFlags::NO_NAV)
                                                              .position(// 2.5: HARDCODE HAX, taking into account menubar height
                                                                        [display_size_x - distance,
                                                                         distance * 2.5],
                                                                        Condition::Always)
                                                              .position_pivot([1.0, 0.0])
                                                              .movable(false)
                                                              .title_bar(false)
                                                              .resizable(false)
                                                              .always_auto_resize(true)
                                                              .save_settings(false)
                                                              .focus_on_appearing(false)
                                                              .build(self.ui, draw_fn)
    }

    // TODO: basically a copy+paste of draw_top_right_overlay
    fn draw_top_left_overlay(&self, draw_fn: &dyn Fn()) {
        let distance = 10.0;
        imgui::Window::new(&self.imlabel("top_left")).flags(WindowFlags::NO_NAV)
                                                     .position(// 2.5: HARDCODE HAX, taking into account menubar height
                                                               [distance * 2.5, distance * 2.5],
                                                               Condition::Always)
                                                     .position_pivot([1.0, 0.0])
                                                     .movable(false)
                                                     .title_bar(false)
                                                     .resizable(false)
                                                     .always_auto_resize(true)
                                                     .save_settings(false)
                                                     .focus_on_appearing(false)
                                                     .build(self.ui, draw_fn)
    }

    // taken from https://github.com/ocornut/imgui/issues/1901#issuecomment-400563921
    fn draw_spinner(&self) {
        let time = self.ui.time();
        self.ui
            .text(["|", "/", "-", "\\"][(time / 0.05) as usize & 3])
    }

    fn draw_window<F: Fn(Keypress) + 'static, G: Fn() + 'static, H>(&self,
                                                                    window_name: &str,
                                                                    size: (usize, usize),
                                                                    pos: (isize, isize),
                                                                    draw_window_contents: &dyn Fn(),
                                                                    handle_keypress: Option<F>,
                                                                    onclose: Option<G>,
                                                                    onwindowchange: H)
        where H: Fn((isize, isize), (usize, usize)) + 'static
    {
        let window_name = self.imlabel(window_name);
        let window_name_str: &str = window_name.as_ref();

        let window_after_prev_draw = TkCache::get_window(window_name_str);
        let mut window_builder = imgui::Window::new(&window_name).movable(true)
                                                                 .scrollable(true);

        if let Some(window) = window_after_prev_draw {
            // size == (0, 0) means don't interfere with the window size, let imgui do its thing
            if window.size != (size.0 as f32, size.1 as f32) && (size != (0, 0)) {
                window_builder =
                    window_builder.size([size.0 as f32, size.1 as f32], Condition::Always)
            }
            if window.pos != (pos.0 as f32, pos.1 as f32) {
                window_builder =
                    window_builder.position([pos.0 as f32, pos.1 as f32], Condition::Always);
            }
        } else {
            window_builder =
                window_builder.size([size.0 as f32, size.1 as f32], Condition::FirstUseEver)
                              .position([pos.0 as f32, pos.1 as f32], Condition::FirstUseEver);
        }

        let mut should_stay_open = true;

        if onclose.is_some() {
            window_builder = window_builder.opened(&mut should_stay_open);
        }

        window_builder.build(self.ui, &|| {
                          TkCache::set_current_window_label(&window_name);
                          TkCache::set_current_window_flex(0);
                          self.ui.group(draw_window_contents);
                          TkCache::clear_current_window_label();
                          let content_size = self.ui.item_rect_size();

                          let mut cache = TK_CACHE.lock().unwrap();
                          let prev_window = cache.windows.get(window_name_str);
                          let window_pos = self.ui.window_pos();
                          let window_size = self.ui.window_size();
                          let remaining_size = self.ui.content_region_avail();
                          let drawn_window =
                              Window { pos: (window_pos[0], window_pos[1]),
                                       size: (window_size[0], window_size[1]),
                                       remaining_size: (remaining_size[0], remaining_size[1]),
                                       content_size: (content_size[0], content_size[1]),
                                       flex: cache.current_flex };
                          if prev_window.is_some() && prev_window.unwrap() != &drawn_window {
                              onwindowchange((drawn_window.pos.0 as isize,
                                              drawn_window.pos.1 as isize),
                                             (drawn_window.size.0 as usize,
                                              drawn_window.size.1 as usize))
                          }
                          cache.windows
                               .insert(window_name_str.to_string(), drawn_window);

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

    fn draw_child_region<F: Fn(Keypress) + 'static>(&self,
                                                    bg: Color,
                                                    draw_fn: &dyn Fn(),
                                                    height: ChildRegionHeight,
                                                    draw_context_menu: Option<&dyn Fn()>,
                                                    handle_keypress: Option<F>) {
        let child_frame_id = self.imlabel("");
        let mut flex = 0;

        let height = match height {
            ChildRegionHeight::FitContent => {
                let current_window = TkCache::get_current_window();
                match current_window {
                    // initially give back 0 if the window size is totally empty
                    None => 0.,
                    Some(_) => {
                        let magic = 16.;
                        TkCache::get_child_region_content_height(child_frame_id.as_ref()).unwrap_or(0.) + magic
                    }
                }
            }
            ChildRegionHeight::ExpandFill { min_height } => {
                flex = 1;

                // TODO: use imgui stack layout once it's in (https://github.com/ocornut/imgui/pull/846)
                // ^^ i'm kind of implementing my own layout code, maybe we won't end up needing this ^
                let current_window = TkCache::get_current_window();
                // MAGICNUMBER: some pixels of padding, for some reason the remaining height gives
                // a few pixels less than we actually need to use up the whole screen
                let magic = 16.;
                match current_window {
                    // initially give back 0 if the window size is totally empty
                    None => 0.,
                    Some(window) => {
                        let apply_flex = |height| (flex as f32 / window.flex as f32) * height;

                        let prev_height =
                            TkCache::get_child_region_height(child_frame_id.as_ref()).unwrap_or(0.);
                        let remaining_height = window.remaining_size.1;
                        if remaining_height == 0. {
                            (prev_height + magic).max(min_height)
                        } else if remaining_height < 0. {
                            // this was resized to be smaller
                            (prev_height + apply_flex(remaining_height)).max(min_height)
                        } else {
                            // greater than
                            prev_height + apply_flex(remaining_height) + magic
                        }
                    }
                }
            }
            ChildRegionHeight::Pixels(pixels) => pixels as f32,
        };

        // TODO: this is hardcoded and can't be shared with wasm or any other system
        let default_bg = [0.5, 0.5, 0.5, 0.5];
        let f = 1.5;
        let brighter_bg = [default_bg[0] * f,
                           default_bg[1] * f,
                           default_bg[2] * f,
                           default_bg[3]];

        let color = if TkCache::is_focused(child_frame_id.as_ref()) {
            brighter_bg
        } else {
            default_bg
        };

        let token = self.ui
                        .push_style_colors(&[(StyleColor::Border, color),
                                             (StyleColor::ChildBg, [bg[0], bg[1], bg[2], bg[3]])]);

        imgui::ChildWindow::new(&child_frame_id)
            .size([0., height])
            .border(true)
            .horizontal_scrollbar(true)
            .build(self.ui, &|| {
                let child_region_height = self.ui.content_region_avail()[1];
                self.ui.group(draw_fn);
                let content_height = self.ui.item_rect_size()[1];

                if let Some(keypress) = self.keypress {
                    if self.ui.is_window_focused_with_flags(WindowFocusedFlags::CHILD_WINDOWS) {
                        if let Some(ref handle_keypress) = handle_keypress {
                            handle_keypress(keypress)
                        }
                    }
                }

                TkCache::set_child_region_info(child_frame_id.as_ref(),
                                               ChildRegion::new(self.ui.is_window_focused_with_flags(WindowFocusedFlags::CHILD_WINDOWS),
                                                                child_region_height,
                                                                content_height),
                                               flex);

                if let Some(draw_context_menu) = draw_context_menu {
                    let label = self.imlabel("draw_context_menu");
                    if unsafe {
                        let mouse_button = 1;
                        imgui_sys::igBeginPopupContextWindow(label.as_ptr(), mouse_button, false)
                    } {
                        draw_context_menu();
                        unsafe {
                            imgui_sys::igEndPopup();
                        }
                    }
                }
            });
        token.pop(self.ui);
    }

    fn draw_layout_with_bottom_bar(&self,
                                   draw_content_fn: &dyn Fn(),
                                   draw_bottom_bar_fn: &dyn Fn()) {
        let frame_height = unsafe { imgui_sys::igGetFrameHeightWithSpacing() };
        imgui::ChildWindow::new(&self.imlabel("")).size([0.0, -frame_height])
                                                  .build(self.ui, draw_content_fn);
        draw_bottom_bar_fn()
    }

    fn draw_separator(&self) {
        self.ui.separator();
    }

    fn draw_empty_line(&self) {
        self.ui.new_line();
    }

    // HAX: this is awfully specific to have in a UI toolkit library... whatever
    fn draw_code_line_separator(&self, plus_char: char, width: f32, height: f32, color: [f32; 4]) {
        self.ui
            .invisible_button(&self.imlabel("code_line_separator"), [width, height]);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };

        let draw_list = self.ui.get_window_draw_list();

        let center_of_line = [min.x, (min.y + max.y) / 2.];
        let mut line = (center_of_line, [max.x, center_of_line[1]]);
        // idk why exactly, but these -0.5s get the line to match up nicely with the circle
        (line.0)[1] -= 0.5;
        (line.1)[1] -= 0.5;
        draw_list.add_line(line.0, line.1, color).build();

        let mut buf = [0; 4];
        let plus_char = plus_char.encode_utf8(&mut buf);

        // draw the text second so it draws over the line
        let text_size = self.ui.calc_text_size(&im_str!("{}", plus_char), false, 0.);
        // XXX: this is magic that aligns the plus sign drawn with the line
        // when height is 15, magic is -5
        // when height is 20, magic is -8
        // y = (-3/5)x+4
        let y_align_magic = ((-3. / 5.) * text_size[1]) + 4.;
        let where_to_put_symbol_y = center_of_line[1] - ((2. / text_size[1]) - y_align_magic);
        let textpos = [center_of_line[0], where_to_put_symbol_y];

        draw_list.add_text(textpos, color, plus_char);
    }

    fn replace_on_hover(&self, draw_when_not_hovered: &dyn Fn(), draw_when_hovered: &dyn Fn()) {
        let replace_on_hover_label = self.imlabel("replace_on_hover_label");
        self.ui.group(&|| {
                   if TkCache::is_hovered(replace_on_hover_label.as_ref()) {
                       draw_when_hovered()
                   } else {
                       draw_when_not_hovered()
                   }
               });
        let label: &str = replace_on_hover_label.as_ref();
        TkCache::set_is_hovered(label.to_owned(), self.ui.is_item_hovered())
    }

    fn draw_box_around(&self, color: [f32; 4], draw_fn: &dyn Fn()) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui
            .get_window_draw_list()
            .add_rect(min.into(), max.into(), color)
            .filled(true)
            .build();
    }

    fn draw_top_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &dyn Fn()) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui
            .get_window_draw_list()
            .add_rect(min.into(), [max.x, min.y + thickness as f32 - 1.], color)
            .thickness(1.)
            .filled(true)
            .build();
    }

    fn draw_right_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &dyn Fn()) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui
            .get_window_draw_list()
            .add_rect([max.x - thickness as f32, min.y], max.into(), color)
            .thickness(1.)
            .filled(true)
            .build()
    }

    fn draw_left_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &dyn Fn()) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui
            .get_window_draw_list()
            .add_rect(min.into(), [min.x - thickness as f32, max.y], color)
            .thickness(1.)
            .filled(true)
            .build()
    }

    fn draw_bottom_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &dyn Fn()) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui
            .get_window_draw_list()
            .add_rect([min.x, max.y - thickness as f32], max.into(), color)
            .thickness(1.)
            .filled(true)
            .build()
    }

    // holy shit it's a mess, but it works.
    // draw buttony text because an actual button will trigger all sorts of hover and action
    // behaviors in imgui, and sometimes we want buttony things grouped together, looking like
    // a single button, not individual buttons. hence draw_buttony_text.
    fn draw_buttony_text(&self, label: &str, color: [f32; 4]) {
        let style = self.ui.clone_style();
        let padding = style.frame_padding;

        let original_cursor_pos = self.ui.cursor_pos();
        let label = im_str!("{}", label);
        let text_size = self.ui.calc_text_size(&label, false, 0.);
        let total_size = [text_size[0] + (padding[0] * 2.),
                          text_size[1] + (padding[1] * 2.)];

        let draw_cursor_pos = self.ui.cursor_screen_pos();
        let end_of_button_bg_rect = [draw_cursor_pos[0] + total_size[0],
                                     draw_cursor_pos[1] + total_size[1]];
        self.ui
            .get_window_draw_list()
            .add_rect(draw_cursor_pos, end_of_button_bg_rect, color)
            .filled(true)
            .build();

        let draw_cursor_pos = self.ui.cursor_screen_pos();
        let buttony_text_start_cursor_pos = [draw_cursor_pos[0] + padding[0],
                                             draw_cursor_pos[1] + padding[1]];
        let draw_list = self.ui.get_window_draw_list();
        let text_color = style.colors[StyleColor::Text as usize];
        draw_list.add_text(buttony_text_start_cursor_pos, text_color, label);
        self.ui.set_cursor_pos(original_cursor_pos);

        self.ui.invisible_button(&self.imlabel(""), total_size);
    }

    fn draw_button<F: Fn() + 'static>(&self, label: &str, color: [f32; 4], on_button_activate: F) {
        let token = self.ui.push_style_color(StyleColor::Button, color);
        if self.ui.button(&self.imlabel(label), BUTTON_SIZE) {
            on_button_activate();
        }
        token.pop(self.ui);
    }

    // XXX: why do i have the small button look like a normal button again????
    // maybe it's because the code icons looked like this
    fn draw_small_button<F: Fn() + 'static>(&self,
                                            label: &str,
                                            color: [f32; 4],
                                            on_button_activate: F) {
        let token = self.ui.push_style_color(StyleColor::Button, color);
        if self.ui.small_button(&self.imlabel(label)) {
            on_button_activate()
        }
        token.pop(self.ui);
    }

    fn draw_text_box(&self, text: &str) {
        let token = self.ui.push_text_wrap_pos(0.);
        self.ui.text(text);
        token.pop(self.ui);
        // GHETTO: text box is always scrolled to the bottom
        unsafe { imgui_sys::igSetScrollHereY(1.0) };
    }

    // cribbed from https://github.com/ocornut/imgui/issues/1388
    fn draw_whole_line_console_text_input(&self, ondone: impl Fn(&str) + 'static) {
        let draw_fn = &|| {
            let token = self.ui.push_item_width(-1.);
            // this is a copy and paste of draw_text_input
            let mut box_input = buf("");
            let enter_pressed = self.ui
                                    .input_text(&self.imlabel(""), &mut box_input)
                                    .enter_returns_true(true)
                                    .always_insert_mode(true)
                                    .build();
            if enter_pressed {
                ondone(box_input.as_ref() as &str);
            }
            token.pop(self.ui);
        };

        let is_mouse_clicked = unsafe { imgui_sys::igIsMouseClicked(0, false) };
        let is_any_item_active = unsafe { imgui_sys::igIsAnyItemActive() };
        if self.ui.is_window_focused() && !is_mouse_clicked && !is_any_item_active {
            self.focused(draw_fn)
        } else {
            draw_fn()
        }
    }

    fn draw_text_input<F: Fn(&str) -> () + 'static, D: Fn() + 'static>(&self,
                                                                       existing_value: &str,
                                                                       onchange: F,
                                                                       ondone: D) {
        self.draw_text_input_with_label("", existing_value, onchange, ondone)
    }

    fn draw_multiline_text_input_with_label<F: Fn(&str) -> () + 'static>(&self,
                                                                         label: &str,
                                                                         existing_value: &str,
                                                                         onchange: F) {
        let mut box_input = buf(existing_value);
        self.ui
            .input_text_multiline(&self.imlabel(label), &mut box_input, [0., 100.])
            .build();
        if box_input.as_ref() as &str != existing_value {
            onchange(box_input.as_ref() as &str)
        }
    }

    fn draw_text_input_with_label<F: Fn(&str) -> () + 'static, D: Fn() + 'static>(&self,
                                                                                  label: &str,
                                                                                  existing_value: &str,
                                                                                  onchange: F,
                                                                                  ondone: D) {
        let mut box_input = buf(existing_value);

        let enter_pressed = self.ui
                                .input_text(&self.imlabel(label), &mut box_input)
                                .enter_returns_true(true)
                                .always_insert_mode(true)
                                .build();
        if enter_pressed {
            ondone();
            return;
        }

        if box_input.as_ref() as &str != existing_value {
            onchange(box_input.as_ref() as &str)
        }
    }

    fn draw_color_picker_with_label(&self,
                                    label: &str,
                                    existing_value: Color,
                                    onchange: impl Fn(Color) + 'static) {
        let mut edited_value = existing_value.clone();
        let was_color_changed = imgui::ColorEdit::new(&self.imlabel(label), &mut edited_value)
                                    .alpha(true)
                                    .alpha_bar(true)
                                    .display_mode(ColorEditDisplayMode::HEX)
                                    .build(self.ui);
        if was_color_changed && existing_value != edited_value {
            onchange(edited_value)
        }
    }

    fn draw_combo_box_with_label<F, G, H, T>(&self,
                                             label: &str,
                                             is_item_selected: G,
                                             format_item: H,
                                             items: &[&T],
                                             onchange: F)
                                             -> Self::DrawResult
        where T: Clone,
              F: Fn(&T) -> () + 'static,
              G: Fn(&T) -> bool,
              H: Fn(&T) -> String
    {
        let mut selected_item_in_combo_box =
            items.into_iter().position(|i| is_item_selected(i)).unwrap();
        let previous_selection = selected_item_in_combo_box.clone();

        let label = self.imlabel(label);
        ComboBox::new(&label).build_simple(self.ui,
                                           &mut selected_item_in_combo_box,
                                           &items,
                                           &move |item| im_str!("{}", format_item(item)).into());
        if selected_item_in_combo_box != previous_selection {
            onchange(items[selected_item_in_combo_box as usize])
        }
    }

    fn draw_selectables<F, G, H, T>(&self,
                                    is_item_selected: G,
                                    format_item: H,
                                    items: &[&T],
                                    onchange: F)
        where T: 'static,
              F: Fn(&T) -> () + 'static,
              G: Fn(&T) -> bool,
              H: Fn(&T) -> &str
    {
        for item in items {
            if imgui::Selectable::new(&self.imlabel(&format_item(item)))
                .selected(is_item_selected(item))
                .flags(SelectableFlags::empty())
                .size([0., 0.])
                .build(self.ui) {
                onchange(item)
            }
        }
    }

    fn draw_selectables2<T, F: Fn(&T) -> () + 'static>(&self,
                                                       items: Vec<SelectableItem<T>>,
                                                       onselect: F)
                                                       -> Self::DrawResult {
        for selectable in items {
            match selectable {
                SelectableItem::GroupHeader(label) => {
                    self.draw_all_on_same_line(&[&|| self.draw_text("-"), &|| {
                                                   self.draw_text(label)
                                               }])
                }
                SelectableItem::Selectable { item,
                                             label,
                                             is_selected, } => {
                    let label = self.imlabel(&label);
                    let selectable = imgui::Selectable::new(&label).selected(is_selected)
                                                                   .size([0., 0.]);
                    if selectable.build(self.ui) {
                        onselect(&item)
                    }
                }
            }
        }
    }

    fn draw_checkbox_with_label<F: Fn(bool) + 'static>(&self,
                                                       label: &str,
                                                       value: bool,
                                                       onchange: F) {
        let mut val = value;
        self.ui.checkbox(&self.imlabel(label), &mut val);
        if val != value {
            onchange(val);
        }
    }

    fn draw_main_menu_bar(&self, draw_menus: &[DrawFnRef<Self>]) {
        self.ui.main_menu_bar(&|| self.draw_all(draw_menus))
    }

    fn draw_menu(&self, label: &str, draw_menu_items: &dyn Fn()) {
        self.ui.menu(&self.imlabel(label), true, draw_menu_items)
    }

    fn draw_menu_item<F: Fn() + 'static>(&self, label: &str, onselect: F) {
        if imgui::MenuItem::new(&self.imlabel(label)).build(self.ui) {
            onselect()
        }
    }

    fn draw_columns<const N: usize>(&self, draw_fn_groups: &[[DrawFnRef<Self>; N]]) {
        self.ui.columns(N as i32, &self.imlabel(""), false);
        for draw_fns in draw_fn_groups {
            for draw_fn in draw_fns.iter() {
                draw_fn();
                self.ui.next_column();
            }
        }
        self.ui.columns(1, &self.imlabel(""), false);
    }

    fn context_menu(&self, draw_fn: DrawFnRef<Self>, draw_context_menu: DrawFnRef<Self>) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        if unsafe {
            let mouse_button = 1;
            let label = self.imlabel("item_context_menu");
            imgui_sys::igBeginPopupContextItem(label.as_ptr(), mouse_button)
        } {
            draw_context_menu();
            unsafe { imgui_sys::igEndPopup() };
            self.ui
                .get_window_draw_list()
                .add_rect(min.into(), max.into(), colorscheme!(button_active_color))
                .filled(true)
                .build();
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
