use super::app::App;
use super::async_executor;
use super::editor::Keypress;
use super::imgui_support;
use super::ui_toolkit::{Color, SelectableItem, UiToolkit};
use nfd;

use crate::colorscheme;
use crate::ui_toolkit::{
    ChildRegionFrameStyle, ChildRegionHeight, ChildRegionStyle, ChildRegionTopPadding,
    ChildRegionWidth, DrawFnRef,
};
use imgui::*;
use imgui_sys::{
    igAcceptDragDropPayload, igBeginDragDropTarget, igEndDragDropSource, igEndDragDropTarget,
    igSetDragDropPayload, ImGuiPopupFlags_MouseButtonRight,
    ImGuiPopupFlags_NoOpenOverExistingPopup, ImGuiPopupFlags_NoOpenOverItems, ImVec2,
};
use lazy_static::lazy_static;
use objekt::private::ptr::slice_from_raw_parts;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::cell::RefCell;
use std::collections::hash_map::{DefaultHasher, HashMap};
use std::collections::HashSet;
use std::fmt::Debug;
use std::fs::File;
use std::hash::{Hash, Hasher};
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
    content_size: [f32; 2],
    height: f32,
    dragged_rect: Option<Rect>,
}

impl ChildRegion {
    fn new(is_focused: bool,
           height: f32,
           content_size: [f32; 2],
           dragged_rect: Option<Rect>)
           -> Self {
        Self { is_focused,
               height,
               content_size,
               dragged_rect }
    }
}

fn calculate_hash<T: std::hash::Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    s.finish()
}

struct TkCache {
    was_mouse_pressed_when_nothing_was_hovered: bool,

    is_drag_drop_active: bool,
    current_window_label: Option<ImString>,
    child_regions: HashMap<String, ChildRegion>,
    windows: HashMap<String, Window>,
    replace_on_hover: HashMap<String, bool>,
    drag_drop_source_clicked: HashMap<u64, bool>,
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
               elements_focused_in_this_iteration: HashSet::new(),
               is_drag_drop_active: false,
               was_mouse_pressed_when_nothing_was_hovered: false,
               drag_drop_source_clicked: HashMap::new() }
    }

    pub fn is_drag_drop_active() -> bool {
        TK_CACHE.lock().unwrap().is_drag_drop_active
    }

    pub fn mouse_was_pressed_when_nothing_was_hovered() -> bool {
        TK_CACHE.lock()
                .unwrap()
                .was_mouse_pressed_when_nothing_was_hovered
    }

    pub fn set_drag_drop_active() {
        TK_CACHE.lock().unwrap().is_drag_drop_active = true;
    }

    pub fn set_drag_drop_inactive() {
        TK_CACHE.lock().unwrap().is_drag_drop_active = false;
    }

    pub fn cleanup_after_iteration(toolkit: &mut ImguiToolkit) {
        {
            let mut cache = TK_CACHE.lock().unwrap();
            cache.elements_focused_in_prev_iteration =
                cache.elements_focused_in_this_iteration.clone();
            cache.elements_focused_in_this_iteration.clear();

            if toolkit.ui.is_mouse_clicked(MouseButton::Left) {
                cache.was_mouse_pressed_when_nothing_was_hovered =
                    !toolkit.ui.is_any_item_hovered();
            }
        }
        // TODO: jank... we're already unlocking TK_CACHE above, but then doing it again down here
        TkCache::set_drag_drop_inactive();
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

    pub fn get_child_region_dragged_rect(child_window_id: &str) -> Option<Rect> {
        TK_CACHE.lock()
                .unwrap()
                .child_regions
                .get(child_window_id)
                .and_then(|cr| cr.dragged_rect)
    }

    pub fn get_child_region_content_height(child_window_id: &str) -> Option<f32> {
        TK_CACHE.lock()
                .unwrap()
                .child_regions
                .get(child_window_id)
                .map(|cr| cr.content_size[1])
    }

    pub fn get_child_region_content_width(child_window_id: &str) -> Option<f32> {
        TK_CACHE.lock()
                .unwrap()
                .child_regions
                .get(child_window_id)
                .map(|cr| cr.content_size[0])
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

    pub fn set_was_drag_drop_source_clicked(id: impl std::hash::Hash, is_hovered: bool) {
        TK_CACHE.lock()
                .unwrap()
                .drag_drop_source_clicked
                .insert(calculate_hash(&id), is_hovered);
    }

    pub fn was_drag_drop_source_clicked(id: impl std::hash::Hash) -> bool {
        Some(&true)
        == TK_CACHE.lock()
                   .unwrap()
                   .drag_drop_source_clicked
                   .get(&calculate_hash(&id))
    }
}

pub fn draw_app(app: Rc<RefCell<App>>, mut async_executor: async_executor::AsyncExecutor) {
    imgui_support::run("cs".to_string(), move |ui, keypress| {
        let mut app = app.borrow_mut();
        app.flush_commands(&mut async_executor);
        async_executor.turn();
        let mut toolkit = ImguiToolkit::new(ui, keypress);
        app.draw(&mut toolkit);

        TkCache::cleanup_after_iteration(&mut toolkit);

        true
    });
}

struct State {
    used_labels: HashMap<String, i32>,
    in_child_window: Vec<String>,
}

fn buf(text: &str) -> ImString {
    let mut imstr = ImString::with_capacity(text.len() + 500);
    imstr.push_str(text);
    imstr
}

impl State {
    fn new() -> Self {
        State { used_labels: HashMap::new(),
                in_child_window: vec![] }
    }
}

pub struct ImguiToolkit<'a> {
    ui: &'a Ui<'a>,
    keypress: Option<Keypress>,
    state: RefCell<State>,
}

impl<'a> ImguiToolkit<'a> {
    fn get_current_dragged_rect(&self) -> Option<Rect> {
        let label = self.current_child_window_label()?;
        TkCache::get_child_region_dragged_rect(&label)
    }

    fn hovered_on_prev_frame(&self, replace_on_hover_label: &ImString) -> bool {
        TkCache::is_hovered(replace_on_hover_label.as_ref())
        && !self.is_left_mouse_button_dragging()
    }

    fn prev_item_hovered_on_this_frame(&self) -> bool {
        let is_left_mouse_dragging = self.is_left_mouse_button_dragging();
        self.ui.is_item_hovered() && !is_left_mouse_dragging
    }

    fn is_left_mouse_button_dragging(&self) -> bool {
        TkCache::mouse_was_pressed_when_nothing_was_hovered()
        && self.ui.is_mouse_dragging(MouseButton::Left)
        && !TkCache::is_drag_drop_active()
    }

    fn current_bg_color(&self) -> [f32; 4] {
        let style = self.ui.clone_style();
        if self.is_in_child_window() {
            style.colors[StyleColor::ChildBg as usize]
        } else {
            style.colors[StyleColor::WindowBg as usize]
        }
    }

    fn is_in_child_window(&self) -> bool {
        !(*self.state.borrow()).in_child_window.is_empty()
    }

    // TODO: this should call get_child_region_dragged_rect inside
    fn current_child_window_label(&self) -> Option<String> {
        (*self.state.borrow()).in_child_window.last().cloned()
    }

    fn set_in_child_window(&self, label: String) {
        self.state.borrow_mut().in_child_window.push(label);
    }

    fn set_not_in_child_window(&self) {
        self.state.borrow_mut().in_child_window.pop();
    }

    fn blank_out_previously_drawn_item(&self) {
        let (min, max) = self.get_item_rect();
        // without the scope here we get a crash for loading drawlist twice, idk what the deal is
        // tbh
        {
            let drawlist = self.ui.get_window_draw_list();
            drawlist.add_rect(min.into(), max.into(), self.current_bg_color())
                    .filled(true)
                    .build();
        }
    }

    fn get_item_rect(&self) -> (ImVec2, ImVec2) {
        let mut min = ImVec2::zero();
        let mut max = ImVec2::zero();
        unsafe {
            imgui_sys::igGetItemRectMin(&mut min);
            imgui_sys::igGetItemRectMax(&mut max);
        };
        (min, max)
    }

    fn get_item_content_region_max(&self) -> ImVec2 {
        let mut max = ImVec2::zero();
        unsafe {
            imgui_sys::igGetContentRegionMax(&mut max);
        }
        max
    }

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

        let (item_min, item_max) = self.get_item_rect();

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
        if self.is_left_mouse_button_dragging() {
            return false;
        }

        let (min, max) = self.get_item_rect();
        let mouse_pos = self.ui.io().mouse_pos;
        self.is_left_button_down()
        && Rect { min: min.into(),
                  max: max.into() }.contains(mouse_pos)
    }

    fn mouse_released_in_last_drawn_element(&self) -> bool {
        if self.is_left_mouse_button_dragging() {
            return false;
        }
        if self.get_current_dragged_rect().is_some() {
            return false;
        }

        let (min, max) = self.get_item_rect();
        let mouse_pos = self.ui.io().mouse_pos;
        self.was_left_button_released()
        && Rect { min: min.into(),
                  max: max.into() }.contains(mouse_pos)
    }

    fn was_left_button_released(&self) -> bool {
        self.ui.is_mouse_released(MouseButton::Left)
    }

    fn is_left_button_down(&self) -> bool {
        self.ui.is_mouse_down(MouseButton::Left)
    }

    fn make_last_item_look_active(&self) {
        let (min, max) = self.get_item_rect();
        self.ui
            .get_window_draw_list()
            .add_rect(min.into(), max.into(), colorscheme!(button_active_color))
            .filled(true)
            .build();
    }
}

#[derive(Debug, Clone, Copy)]
struct Rect {
    min: (f32, f32),
    max: (f32, f32),
}

impl Rect {
    #[allow(unused)]
    pub fn overlaps(&self, other: &Self) -> bool {
        (self.min.0 < other.max.0
         && self.max.0 > other.min.0
         && self.max.1 > other.min.1
         && self.min.1 < other.max.1)
    }

    #[allow(unused)]
    pub fn from_screen_coords(mut min: [f32; 2], mut max: [f32; 2]) -> Self {
        if min[0] > max[0] {
            std::mem::swap(&mut min[0], &mut max[0])
        }
        if min[1] > max[1] {
            std::mem::swap(&mut min[1], &mut max[1])
        }

        Self { min: (min[0], min[1]),
               max: (max[0], max[1]) }
    }

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

    fn callback_when_drag_intersects<F: Fn() + 'static>(&self,
                                                        draw_fn: DrawFnRef<Self>,
                                                        callback: F) {
        self.ui.group(draw_fn);
        if let Some(dragged_rect) = self.get_current_dragged_rect() {
            let (min, max) = self.get_item_rect(); // this thing should return a rect
            if Rect::from_screen_coords([min.x, min.y], [max.x, max.y]).overlaps(&dragged_rect) {
                callback();
            }
        }
    }

    fn drag_drop_source(&self,
                        source_id: impl Hash + Clone + Debug,
                        draw_fn: DrawFnRef<Self>,
                        draw_preview_fn: DrawFnRef<Self>,
                        payload: impl Serialize) {
        self.ui.group(draw_fn);
        if self.ui.is_mouse_clicked(MouseButton::Left) {
            TkCache::set_was_drag_drop_source_clicked(source_id.clone(), self.ui.is_item_hovered());
        }
        if !TkCache::was_drag_drop_source_clicked(source_id.clone()) {
            return;
        }

        unsafe {
            let flags = ImGuiDragDropFlags::SourceAllowNullID.bits();
            if imgui_sys::igBeginDragDropSource(flags) {
                TkCache::set_drag_drop_active();

                self.ui.group(draw_preview_fn);
                let payload_bytes = bincode::serialize(&payload).unwrap();
                igSetDragDropPayload(b"_ITEM\0".as_ptr() as *const _,
                                     payload_bytes.as_ptr() as *const _,
                                     payload_bytes.len(),
                                     0);

                igEndDragDropSource();
            }
        }
    }

    fn drag_drop_target<D: DeserializeOwned>(&self,
                                             draw_fn: DrawFnRef<Self>,
                                             draw_when_hovered: DrawFnRef<Self>,
                                             accepts_payload: impl Fn(D) + 'static) {
        let orig_cursor_pos = self.ui.cursor_pos();
        self.ui.group(draw_fn);
        let is_being_dropped_on = unsafe { igBeginDragDropTarget() };
        unsafe {
            // TODO: crashes when nested drag drop targets
            if is_being_dropped_on {
                let flags = ImGuiDragDropFlags::AcceptNoDrawDefaultRect.bits();
                let payload = igAcceptDragDropPayload(b"_ITEM\0".as_ptr() as *const _, flags);
                if !payload.is_null() {
                    let payload = *payload;
                    let data =
                        slice_from_raw_parts(payload.Data as *const u8, payload.DataSize as usize);
                    accepts_payload(bincode::deserialize(&*data).unwrap())
                }

                igEndDragDropTarget()
            }
        }

        if is_being_dropped_on {
            // XXX: draws over the non-hovered way drag drop target
            // JANK: not sure how else to do this, imgui api requires that drag drop target
            // is drawn before making it as the target
            self.blank_out_previously_drawn_item();
            self.ui.set_cursor_pos(orig_cursor_pos);
            draw_when_hovered();
        }
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

        if self.prev_item_hovered_on_this_frame() {
            // grabbed this code from draw_box_around
            let (min, max) = self.get_item_rect();
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

        let (min, max) = self.get_item_rect();
        let width = self.get_item_content_region_max().x - frame_padding[0] * 2.;
        self.ui.dummy([width, max.y - min.y])
    }

    fn draw_with_bgcolor(&self, bgcolor: Color, draw_fn: DrawFnRef<Self>) {
        // haxxx: draw twice:
        // first to get the bounding box, then draw over it with the bgcolor, and then
        // draw over it again
        let orig_cursor_pos = self.ui.cursor_pos();

        self.ui.group(draw_fn);
        self.blank_out_previously_drawn_item();
        let (min, max) = self.get_item_rect();
        {
            let drawlist = self.ui.get_window_draw_list();
            drawlist.add_rect(min.into(), max.into(), bgcolor)
                    .filled(true)
                    .build();
        }

        self.ui.set_cursor_pos(orig_cursor_pos);
        draw_fn();
    }

    fn draw_with_no_spacing_afterwards(&self, draw_fn: DrawFnRef<Self>) {
        self.ui.group(draw_fn);
        let (min, max) = self.get_item_rect();
        self.ui.set_cursor_screen_pos([min.x, max.y]);
    }

    fn draw_full_width_heading(&self, bgcolor: Color, inner_padding: (f32, f32), text: &str) {
        // copy and paste of draw_buttony_text lol
        let style = self.ui.clone_style();
        let padding = style.frame_padding;

        let full_width = self.ui.content_region_avail()[0];

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
        if px == 0 {
            return draw_fn();
        }
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

    // TODO: mostly copy and paste of align()
    fn align_fill_lhs(&self,
                      x_padding_left_block_hack: u8,
                      lhs: &dyn Fn() -> Self::DrawResult,
                      lhs_color: Color,
                      rhss: &[&dyn Fn() -> Self::DrawResult])
                      -> Self::DrawResult {
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

        let (lhs_rect_min, lhs_rect_max) = self.get_item_rect();
        let bottom_of_lhs = [lhs_rect_min.x, lhs_rect_max.y];
        let right_of_lhs = lhs_rect_max.x - 1. + x_padding_left_block_hack as f32;

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
        self.ui.group(|| last_rhs());

        let bottom_of_last_item = self.ui.item_rect_max()[1];

        let draw_list = self.ui.get_window_draw_list();
        draw_list.add_rect(bottom_of_lhs,
                           [right_of_lhs, bottom_of_last_item],
                           lhs_color)
                 .filled(true)
                 .build();
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
                              if self.ui
                       .is_window_focused_with_flags(WindowFocusedFlags::CHILD_WINDOWS)
                {
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

    fn draw_child_region<F: Fn(Keypress) + 'static, G: Fn() + 'static>(&self,
                                                                       bg: Color,
                                                                       draw_fn: &dyn Fn(),
                                                                       style: ChildRegionStyle,
                                                                       draw_context_menu: Option<&dyn Fn()>,
                                                                       handle_keypress: Option<F>,
                                                                       drag_selection_occurred: G)
    {
        let child_frame_id = self.imlabel("");

        let y_padding = self.ui.clone_style().window_padding[1];

        let frame_style = style.frame_style;
        let height = style.height;
        let width = style.width;

        let mut flex = 0;

        // MAGICNUMBER: some pixels of padding, for some reason the remaining height gives
        // a few pixels less than we actually need to use up the whole screen
        let magic = {
            match (frame_style, style.top_padding) {
                (ChildRegionFrameStyle::Framed, ChildRegionTopPadding::Default) => 16.,
                (ChildRegionFrameStyle::Framed, ChildRegionTopPadding::None) => 8.,
                (ChildRegionFrameStyle::NoFrame, ChildRegionTopPadding::Default) => 8.,
                (ChildRegionFrameStyle::NoFrame, ChildRegionTopPadding::None) => 0.,
            }
        };

        let height = match height {
            ChildRegionHeight::FitContent => {
                let current_window = TkCache::get_current_window();
                match current_window {
                    // initially give back 0 if the window size is totally empty
                    None => 0.,
                    Some(_) => {
                        TkCache::get_child_region_content_height(child_frame_id.as_ref()).unwrap_or(0.) + magic
                    }
                }
            }
            ChildRegionHeight::ExpandFill { min_height } => {
                flex = 1;

                // TODO: use imgui stack layout once it's in (https://github.com/ocornut/imgui/pull/846)
                // ^^ i'm kind of implementing my own layout code, maybe we won't end up needing this ^
                let current_window = TkCache::get_current_window();
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
            ChildRegionHeight::Max(pixels) => {
                let current_window = TkCache::get_current_window();
                let prev_height = match current_window {
                    // initially give back 0 if the window size is totally empty
                    None => pixels as f32,
                    Some(_) => {
                        TkCache::get_child_region_content_height(child_frame_id.as_ref()).unwrap_or(0.) + magic
                    }
                };
                prev_height.min(pixels as _)
            }
        };

        let width = match width {
            ChildRegionWidth::FitContent => {
                let current_window = TkCache::get_current_window();
                match current_window {
                    // initially give back 0 if the window size is totally empty
                    None => 0.,
                    Some(_) => {
                        TkCache::get_child_region_content_width(child_frame_id.as_ref()).unwrap_or(0.) + magic
                    }
                }
            }
            ChildRegionWidth::All => 0.,
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

        // TODO: more hax, but if we're nested inside of a child region, then change up the margins
        // a bit... this just looks better when it's intermixed with other stuff... might need to make
        // this a config setting, but probably not... seems more like an issue/bug with imgui
        if self.is_in_child_window() {
            let current_pos = self.ui.cursor_pos();
            self.ui
                .set_cursor_pos([current_pos[0] - 1., current_pos[1] + 1.]);
        }

        let token = self.ui
                        .push_style_colors(&[(StyleColor::Border, color),
                                             (StyleColor::ChildBg, [bg[0], bg[1], bg[2], bg[3]])]);
        let mut builder = imgui::ChildWindow::new(&child_frame_id).size([width, height]);

        match frame_style {
            ChildRegionFrameStyle::Framed => {
                builder = builder.border(true).horizontal_scrollbar(true)
            }
            ChildRegionFrameStyle::NoFrame => {
                builder = builder.border(false)
                                 .scrollable(true)
                                 .scroll_bar(false)
                                 .horizontal_scrollbar(true)
            }
        }

        builder.build(self.ui, || {
                   match style.top_padding {
                       ChildRegionTopPadding::Default => (),
                       ChildRegionTopPadding::None => {
                           let current_pos = self.ui.cursor_pos();
                           self.ui
                               .set_cursor_pos([current_pos[0], current_pos[1] - y_padding]);
                       }
                   }

                   let is_child_focused =
                       self.ui
                           .is_window_focused_with_flags(WindowFocusedFlags::CHILD_WINDOWS);

                   self.set_in_child_window(child_frame_id.to_string());

                   let mut screen_coords = (self.ui.cursor_screen_pos(), [0., 0.]);

                   let child_region_height = self.ui.content_region_avail()[1];
                   self.ui.group(draw_fn);

                   let mut dragged_rect = None;
                   if is_child_focused && self.is_left_mouse_button_dragging() {
                       if self.ui.is_any_item_hovered() {
                           self.ui.reset_mouse_drag_delta(MouseButton::Left);
                       } else {
                           let current_mouse_pos = self.ui.io().mouse_pos;
                           let delta = self.ui.mouse_drag_delta(MouseButton::Left);
                           let draw_list = self.ui.get_window_draw_list();
                           let initial_mouse_pos = [current_mouse_pos[0] - delta[0],
                                                    current_mouse_pos[1] - delta[1]];
                           dragged_rect =
                               Some(Rect::from_screen_coords(current_mouse_pos, initial_mouse_pos));
                           draw_list.add_rect(current_mouse_pos,
                                              initial_mouse_pos,
                                              [1., 1., 1., 0.1])
                                    .filled(true)
                                    .build();
                           drag_selection_occurred();
                       }
                   }

                   let content_size = self.ui.item_rect_size();
                   screen_coords.1 = self.ui.cursor_screen_pos();

                   if let Some(keypress) = self.keypress {
                       if is_child_focused {
                           if let Some(ref handle_keypress) = handle_keypress {
                               handle_keypress(keypress)
                           }
                       }
                   }

                   TkCache::set_child_region_info(child_frame_id.as_ref(),
                                                  ChildRegion::new(is_child_focused,
                                                                   child_region_height,
                                                                   content_size,
                                                                   dragged_rect),
                                                  flex);

                   if let Some(draw_context_menu) = draw_context_menu {
                       let label = self.imlabel("draw_context_menu");
                       if unsafe {
                           let flags =
                               ImGuiPopupFlags_NoOpenOverItems | ImGuiPopupFlags_MouseButtonRight;
                           imgui_sys::igBeginPopupContextWindow(label.as_ptr(), flags as i32)
                       } {
                           draw_context_menu();
                           unsafe {
                               imgui_sys::igEndPopup();
                           }
                       }
                   }

                   self.set_not_in_child_window();
               });
        token.pop(self.ui);
    }

    fn with_y_padding(&self, amount_px: u32, draw_fn: DrawFnRef<Self>) {
        let style_var = self.ui
                            .push_style_var(StyleVar::ItemSpacing([0.0, amount_px as f32]));
        draw_fn();
        style_var.pop(&self.ui);
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
    fn draw_code_line_separator(&self, width: f32, height: f32) {
        self.ui
            .invisible_button(&self.imlabel("code_line_separator"), [width, height]);
    }

    fn replace_on_hover(&self, draw_when_not_hovered: &dyn Fn(), draw_when_hovered: &dyn Fn()) {
        let replace_on_hover_label = self.imlabel("replace_on_hover_label");
        self.ui.group(&|| {
                   if self.hovered_on_prev_frame(&replace_on_hover_label) {
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
        let (min, max) = self.get_item_rect();
        self.ui
            .get_window_draw_list()
            .add_rect(min.into(), max.into(), color)
            .filled(true)
            .build();
    }

    fn draw_top_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &dyn Fn()) {
        self.ui.group(draw_fn);
        let (min, max) = self.get_item_rect();
        self.ui
            .get_window_draw_list()
            .add_rect(min.into(), [max.x, min.y + thickness as f32 - 1.], color)
            .thickness(1.)
            .filled(true)
            .build();
    }

    fn draw_right_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &dyn Fn()) {
        self.ui.group(draw_fn);
        let (min, max) = self.get_item_rect();
        self.ui
            .get_window_draw_list()
            .add_rect([max.x - thickness as f32, min.y], max.into(), color)
            .thickness(1.)
            .filled(true)
            .build()
    }

    fn draw_left_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &dyn Fn()) {
        self.ui.group(draw_fn);
        let (min, max) = self.get_item_rect();
        self.ui
            .get_window_draw_list()
            .add_rect(min.into(), [min.x - thickness as f32, max.y], color)
            .thickness(1.)
            .filled(true)
            .build()
    }

    fn draw_bottom_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &dyn Fn()) {
        self.ui.group(draw_fn);
        let (min, max) = self.get_item_rect();
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
                                                                       fit_input_width: bool,
                                                                       onchange: F,
                                                                       ondone: D) {
        if fit_input_width {
            let text_size = self.ui
                                .calc_text_size(&im_str!("{}", existing_value), false, 0.);
            let padding = 10.;
            self.ui.set_next_item_width(text_size[0] + padding);
        }
        self.draw_text_input_with_label("", existing_value, onchange, ondone)
    }

    fn draw_multiline_text_input_with_label<F: Fn(&str) -> () + 'static,
                                                E: Fn() -> () + 'static>(
        &self,
        label: &str,
        existing_value: &str,
        onchange: F,
        onenter: E) {
        let mut box_input = buf(existing_value);
        self.ui
            .input_text_multiline(&self.imlabel(label), &mut box_input, [0., 100.])
            .resize_buffer(true)
            .build();
        if self.ui.is_item_active() {
            match self.keypress {
                Some(Keypress { key: crate::editor::Key::Enter,
                                ctrl: false,
                                shift: false, }) => return onenter(),
                _ => {}
            }
        }
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
        let (min, max) = self.get_item_rect();
        if unsafe {
            let label = self.imlabel("item_context_menu");
            let flags = ImGuiPopupFlags_MouseButtonRight | ImGuiPopupFlags_NoOpenOverExistingPopup;
            imgui_sys::igBeginPopupContextItem(label.as_ptr(), flags as i32)
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
