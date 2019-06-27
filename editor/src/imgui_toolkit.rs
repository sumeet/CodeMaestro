use super::app::App;
use super::async_executor;
use super::editor::Keypress;
use super::imgui_support;
use super::ui_toolkit::{Color, SelectableItem, UiToolkit};
use itertools::Itertools;

use crate::code_editor_renderer::{BLUE_COLOR, PURPLE_COLOR};
use crate::imgui_support::{BUTTON_ACTIVE_COLOR, BUTTON_HOVERED_COLOR};
use crate::ui_toolkit::ChildRegionHeight;
use imgui::*;
use lazy_static::lazy_static;
use std::cell::RefCell;
use std::collections::hash_map::HashMap;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;

pub const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const TRANSPARENT_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 0.0];
const BUTTON_SIZE: (f32, f32) = (0.0, 0.0);
const INITIAL_WINDOW_SIZE: (f32, f32) = (400.0, 500.0);

lazy_static! {
    static ref TK_CACHE: Arc<Mutex<TkCache>> = Arc::new(Mutex::new(TkCache::new()));
}

#[derive(Clone, Copy, PartialEq)]
struct Window {
    pos: (f32, f32),
    size: (f32, f32),
}

//enum ScrollStatus {
//    FirstFocus,
//    AlreadyFocused,
//}

struct TkCache {
    focused_child_regions: HashMap<String, bool>,
    windows: HashMap<String, Window>,
    replace_on_hover: HashMap<String, bool>,
    //scroll_for_keyboard_nav: HashMap<String, ScrollStatus>,
    elements_focused_in_prev_iteration: HashSet<String>,
    elements_focused_in_this_iteration: HashSet<String>,
}

impl TkCache {
    fn new() -> Self {
        Self { focused_child_regions: HashMap::new(),
               replace_on_hover: HashMap::new(),
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

    pub fn is_new_focus_for_scrolling(scroll_hash: String) -> bool {
        let mut cache = TK_CACHE.lock().unwrap();
        let is_new_focus = !cache.elements_focused_in_prev_iteration
                                 .contains(&scroll_hash);
        cache.elements_focused_in_this_iteration.insert(scroll_hash);
        is_new_focus
    }

    pub fn is_focused(child_window_id: &str) -> bool {
        *TK_CACHE.lock()
                 .unwrap()
                 .focused_child_regions
                 .get(child_window_id)
                 .unwrap_or(&false)
    }

    pub fn set_is_focused(child_window_id: &str, is_focused: bool) {
        TK_CACHE.lock()
                .unwrap()
                .focused_child_regions
                .insert(child_window_id.to_string(), is_focused);
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
    imgui_support::run("cs".to_string(), CLEAR_COLOR, |ui, keypress| {
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
        let window_min = self.ui.get_window_pos();
        let window_size = self.ui.get_window_size();
        let window_max = (window_min.0 + window_size.0, window_min.1 + window_size.1);

        let item_min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let item_max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };

        ((window_min.0 <= item_min.x)
         && (item_min.x <= window_max.0)
         && (window_min.0 <= item_max.x)
         && (item_max.x <= window_max.0)
         && (window_min.1 <= item_min.y)
         && (item_min.y <= window_max.1)
         && (window_min.1 <= item_max.y)
         && (item_max.y <= window_max.1))
    }

    fn mouse_clicked_in_last_drawn_element(&self) -> bool {
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() }.into();
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() }.into();
        let mouse_pos = self.ui.imgui().mouse_pos();
        self.is_left_button_down() && Rect { min, max }.contains(mouse_pos)
    }

    fn mouse_released_in_last_drawn_element(&self) -> bool {
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() }.into();
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() }.into();
        let mouse_pos = self.ui.imgui().mouse_pos();
        self.was_left_button_released() && Rect { min, max }.contains(mouse_pos)
    }

    fn was_left_button_released(&self) -> bool {
        self.ui.imgui().is_mouse_released(ImMouseButton::Left)
    }

    fn is_left_button_down(&self) -> bool {
        self.ui.imgui().is_mouse_down(ImMouseButton::Left)
    }
}

struct Rect {
    min: (f32, f32),
    max: (f32, f32),
}

impl Rect {
    pub fn contains(&self, p: (f32, f32)) -> bool {
        return self.min.0 <= p.0 && p.0 <= self.max.0 && self.min.1 <= p.1 && p.1 <= self.max.1;
    }
}

impl<'a> UiToolkit for ImguiToolkit<'a> {
    type DrawResult = ();

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

    // TODO: these should be draw funcs that we execute in here
    fn draw_all(&self, _draw_results: Vec<()>) {}

    fn focused(&self, draw_fn: &dyn Fn()) {
        draw_fn();
        unsafe {
            // HACK: the igIsAnyItemHovered allows me to click buttons while focusing a text field.
            // good enough for now.
            if !imgui_sys::igIsAnyItemActive() && !imgui_sys::igIsAnyItemHovered() {
                imgui_sys::igSetKeyboardFocusHere(-1)
            }
        }
    }

    fn buttonize<F: Fn() + 'static>(&self, draw_fn: &dyn Fn(), onclick: F) {
        // HAXXX: disable buttons that were drawn with `draw_button` from displaying as hovered.
        // i think in actually this is very very very messy because what happens if someone clicks
        // an inner button, do we run all the click handlers???
        //        self.ui.with_color_vars(&[(ImGuiCol::ButtonHovered, (1., 1., 1., 0.)),
        //                                            (ImGuiCol::ButtonActive, (1., 1., 1., 0.))], &|| {
        self.ui.group(draw_fn);
        //                self.ui.with_color_vars(&[(ImGuiCol::ButtonHovered, (1., 1., 1., 0.)),
        //                                                    (ImGuiCol::ButtonActive, (1., 1., 1., 0.))], draw_fn));

        // grabbed this code from draw_box_around
        if self.ui.is_item_hovered() {
            let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
            let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
            self.ui
                .get_window_draw_list()
                .add_rect(min, max, BUTTON_HOVERED_COLOR)
                .filled(true)
                .build();
        }

        if self.mouse_clicked_in_last_drawn_element() {
            let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
            let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
            self.ui
                .get_window_draw_list()
                .add_rect(min, max, BUTTON_ACTIVE_COLOR)
                .filled(true)
                .build();
        }

        if self.mouse_released_in_last_drawn_element() {
            onclick()
        }

        //        })
    }

    fn draw_statusbar(&self, draw_fn: &dyn Fn()) {
        let x_padding = 4.0;
        let y_padding = 5.0;
        let font_size = unsafe { imgui_sys::igGetFontSize() };
        // cribbed status bar implementation from
        // https://github.com/ocornut/imgui/issues/741#issuecomment-233288320
        let status_height = (y_padding * 2.0) + font_size;

        let display_size = self.ui.imgui().display_size();
        let window_pos = (0.0, display_size.1 - status_height);
        let window_size = (display_size.0, status_height);

        self.ui.with_style_vars(&[StyleVar::WindowRounding(0.0),
                                  StyleVar::WindowPadding(ImVec2::new(x_padding, y_padding))],
                                &|| {
                                    self.ui
                                        .window(&self.imlabel("statusbar"))
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
                                })
    }

    fn draw_text(&self, text: &str) {
        self.ui
            .with_color_vars(&[(ImGuiCol::ButtonHovered, TRANSPARENT_COLOR),
                               (ImGuiCol::ButtonActive, TRANSPARENT_COLOR)],
                             &|| self.draw_button(text, TRANSPARENT_COLOR, &|| {}))
    }

    fn draw_text_with_label(&self, text: &str, label: &str) -> Self::DrawResult {
        self.ui
            .label_text(im_str!("{}", label), im_str!("{}", text))
    }

    fn draw_all_on_same_line(&self, draw_fns: &[&dyn Fn()]) {
        if let Some((last_draw_fn, first_draw_fns)) = draw_fns.split_last() {
            for draw_fn in first_draw_fns {
                draw_fn();
                self.ui.same_line_spacing(0.0, 0.0);
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

        unsafe {
            imgui_sys::igPushStyleVarVec2(imgui_sys::ImGuiStyleVar::ItemSpacing, (0.0, -1.0).into())
        };
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

    fn draw_centered_popup<F: Fn(Keypress) + 'static>(&self,
                                                      draw_fn: &dyn Fn(),
                                                      handle_keypress: Option<F>)
                                                      -> Self::DrawResult {
        let (display_size_x, display_size_y) = self.ui.imgui().display_size();
        self.ui
            .window(&self.imlabel("draw_centered_popup"))
            .size(INITIAL_WINDOW_SIZE, ImGuiCond::Always)
            .position((display_size_x * 0.5, display_size_y * 0.5),
                      ImGuiCond::Always)
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

                // i think this is a good place to fuck around
                self.draw_all_on_same_line(&[
                &|| {
                    self.ui
                        .with_color_vars(&[(ImGuiCol::FrameBg, BLUE_COLOR)], &|| {
                            self.draw_text("some other shiat")
                        })
                },
                &|| {
                    self.ui
                        .with_color_vars(&[(ImGuiCol::FrameBg, PURPLE_COLOR)], &|| {
                            self.draw_text("some shiatr")
                        })
                },
            ])
            });
    }

    // stolen from https://github.com/ocornut/imgui/blob/master/imgui_demo.cpp#L3944
    fn draw_top_right_overlay(&self, draw_fn: &dyn Fn()) {
        let distance = 10.0;
        let (display_size_x, _) = self.ui.imgui().display_size();
        self.ui
            .window(&self.imlabel("top_right_overlay"))
            .flags(ImGuiWindowFlags::NoNav)
            .position(// 2.5: HARDCODE HAX
                      (display_size_x - distance, distance * 2.5),
                      ImGuiCond::Always)
            .position_pivot((1.0, 0.0))
            .movable(false)
            .title_bar(false)
            .resizable(false)
            .always_auto_resize(true)
            .save_settings(false)
            .no_focus_on_appearing(true)
            .build(draw_fn)
    }

    // taken from https://github.com/ocornut/imgui/issues/1901#issuecomment-400563921
    fn draw_spinner(&self) {
        let time = self.ui.imgui().get_time();
        self.ui
            .text(["|", "/", "-", "\\"][(time / 0.05) as usize & 3])
    }

    fn draw_window<F: Fn(Keypress) + 'static, G: Fn() + 'static, H>(&self,
                                                                    window_name: &str,
                                                                    size: (usize, usize),
                                                                    pos: (isize, isize),
                                                                    f: &dyn Fn(),
                                                                    handle_keypress: Option<F>,
                                                                    onclose: Option<G>,
                                                                    onwindowchange: H)
        where H: Fn((isize, isize), (usize, usize)) + 'static
    {
        let window_name = self.imlabel(window_name);
        let window_name_str: &str = window_name.as_ref();

        let window_after_prev_draw = {
            let cache = TK_CACHE.lock().unwrap();
            cache.windows.get(window_name_str).cloned()
        };

        let mut window_builder = self.ui.window(&window_name).movable(true).scrollable(true);

        if let Some(window) = window_after_prev_draw {
            if window.size != (size.0 as f32, size.1 as f32) {
                window_builder =
                    window_builder.size((size.0 as f32, size.1 as f32), ImGuiCond::Always)
            }
            if window.pos != (pos.0 as f32, pos.1 as f32) {
                window_builder =
                    window_builder.position((pos.0 as f32, pos.1 as f32), ImGuiCond::Always);
            }
        } else {
            window_builder =
                window_builder.size((size.0 as f32, size.1 as f32), ImGuiCond::FirstUseEver)
                              .position((pos.0 as f32, pos.1 as f32), ImGuiCond::FirstUseEver);
        }

        let mut should_stay_open = true;

        if onclose.is_some() {
            window_builder = window_builder.opened(&mut should_stay_open);
        }

        window_builder.build(&|| {
                          f();
                          let mut cache = TK_CACHE.lock().unwrap();
                          let prev_window = cache.windows.get(window_name_str).cloned();
                          let drawn_window = Window { pos: self.ui.get_window_pos(),
                                                      size: self.ui.get_window_size() };
                          cache.windows
                               .insert(window_name_str.to_string(), drawn_window);
                          if prev_window.is_some() && prev_window.unwrap() != drawn_window {
                              onwindowchange((drawn_window.pos.0 as isize,
                                              drawn_window.pos.1 as isize),
                                             (drawn_window.size.0 as usize,
                                              drawn_window.size.1 as usize))
                          }

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
                                                    // TODO: actually use this bg color lol
                                                    _bg: Color,
                                                    draw_fn: &dyn Fn(),
                                                    height: ChildRegionHeight,
                                                    draw_context_menu: Option<&dyn Fn()>,
                                                    handle_keypress: Option<F>) {
        let height = match height {
            ChildRegionHeight::Percentage(height_percentage) => {
                height_percentage * unsafe { imgui_sys::igGetWindowHeight() }
            }
            ChildRegionHeight::Pixels(pixels) => pixels as f32,
        };

        // TODO: this is hardcoded and can't be shared with wasm or any other system
        let default_bg = (0.5, 0.5, 0.5, 0.5);
        let f = 1.5;
        let brighter_bg = (default_bg.0 * f, default_bg.1 * f, default_bg.2 * f, default_bg.3);

        let child_frame_id = self.imlabel("");
        let color = if TkCache::is_focused(child_frame_id.as_ref()) {
            brighter_bg
        } else {
            default_bg
        };

        self.ui.with_color_var(ImGuiCol::Border, color, &|| {
                   self.ui
                       .child_frame(&child_frame_id, (0., height))
                       .show_borders(true)
                       .scrollbar_horizontal(true)
                       .build(&|| {
                           draw_fn();
                           if let Some(keypress) = self.keypress {
                               if self.ui.is_child_window_focused() {
                                   if let Some(ref handle_keypress) = handle_keypress {
                                       handle_keypress(keypress)
                                   }
                               }
                           }

                           TkCache::set_is_focused(child_frame_id.as_ref(),
                                                   self.ui.is_child_window_focused());

                           if let Some(draw_context_menu) = draw_context_menu {
                               let label = self.imlabel("draw_context_menu");
                               if unsafe {
                                   let mouse_button = 1;
                                   imgui_sys::igBeginPopupContextWindow(label.as_ptr(),
                                                                        mouse_button,
                                                                        false)
                               } {
                                   draw_context_menu();
                                   unsafe {
                                       imgui_sys::igEndPopup();
                                   }
                               }
                           }
                       });
               });
    }

    fn draw_x_scrollable_list<'b>(&'b self,
                                  items: impl ExactSizeIterator<Item = (&'b dyn Fn(), bool)>,
                                  lines_height: usize) {
        let height = self.ui.get_text_line_height_with_spacing();
        let first_element_screen_x = Rc::new(RefCell::new(0.));

        let length = items.len();
        if length == 0 {
            return;
        }
        let last_element_index = length - 1;
        let items: Vec<Box<dyn Fn()>> =
            items.enumerate()
                 .map(|(i, (draw_fn, is_focused))| {
                     let first_element_screen_x = Rc::clone(&first_element_screen_x);
                     let x: Box<dyn Fn()> = Box::new(move || {
                         if i == 0 {
                             first_element_screen_x.replace(self.ui.get_cursor_pos().0);
                         }
                         let (focused_element_x, _) = self.ui.get_cursor_screen_pos();
                         draw_fn();
                         if is_focused {
                             if i == 0 {
                                 unsafe { imgui_sys::igSetScrollX(0.) };
                             } else if i == last_element_index {
                                 unsafe { imgui_sys::igSetScrollX(imgui_sys::igGetScrollMaxX()) };
                             } else if !self.is_last_drawn_item_totally_visible() {
                                 // TODO: this thing is still wonky, but it works ok enough for now
                                 //let set_to = focused_element_x - *first_element_screen_x.borrow();
                                 let set_to = {
                                     if focused_element_x < 0.0 {
                                         (unsafe { imgui_sys::igGetScrollX() }) - 5.
                                     } else {
                                         (unsafe { imgui_sys::igGetScrollX() }) + 5.
                                     }
                                 };
                                 unsafe { imgui_sys::igSetScrollX(set_to) };
                             }
                         }
                     });
                     x
                 })
                 .collect_vec();

        self.ui
            .child_frame(&self.imlabel(""), (0., lines_height as f32 * height))
            .show_borders(false)
            .always_show_vertical_scroll_bar(false)
            .scrollbar_horizontal(false)
            .always_show_horizontal_scroll_bar(false)
            .build(&|| {
                self.draw_all_on_same_line(&items.iter().map(|i| i.as_ref()).collect_vec());
            })
    }

    fn draw_layout_with_bottom_bar(&self, draw_content_fn: &dyn Fn(), draw_bottom_bar_fn: &dyn Fn()) {
        let frame_height = unsafe { imgui_sys::igGetFrameHeightWithSpacing() };
        self.ui
            .child_frame(&self.imlabel(""), (0.0, -frame_height))
            .build(draw_content_fn);
        draw_bottom_bar_fn()
    }

    fn draw_separator(&self) {
        self.ui.separator();
    }

    fn draw_empty_line(&self) {
        self.ui.new_line();
    }

    // HAX: this is awfully specific to have in a UI toolkit library... whatever
    fn draw_code_line_separator(&self, width: f32, height: f32, color: [f32; 4]) {
        self.ui
            .invisible_button(&self.imlabel("code_line_separator"), (width, height));
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };

        let center_of_circle = (min.x, (min.y + max.y) / 2.);
        let radius = (max.y - min.y) / 2.;

        let mut line = (center_of_circle, (max.x, center_of_circle.1));
        // idk why exactly, but these -0.5s get the line to match up nicely with the circle
        (line.0).1 -= 0.5;
        (line.1).1 -= 0.5;

        let draw_list = self.ui.get_window_draw_list();
        draw_list.add_circle(center_of_circle, radius, color)
                 .filled(true)
                 .build();
        draw_list.add_line(line.0, line.1, color).build();
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
            .add_rect(min, max, color)
            .filled(true)
            .build();
    }

    fn draw_top_border_inside(&self, color: [f32; 4], thickness: u8, draw_fn: &dyn Fn()) {
        self.ui.group(draw_fn);
        let min = unsafe { imgui_sys::igGetItemRectMin_nonUDT2() };
        let max = unsafe { imgui_sys::igGetItemRectMax_nonUDT2() };
        self.ui
            .get_window_draw_list()
            .add_rect(min, (max.x, min.y + thickness as f32 - 1.), color)
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
            .add_rect((max.x - thickness as f32, min.y), max, color)
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
            .add_rect(min, (min.x - thickness as f32, max.y), color)
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
            .add_rect((min.x, max.y - thickness as f32), max, color)
            .thickness(1.)
            .filled(true)
            .build()
    }

    // holy shit it's a mess, but it works.
    // draw buttony text because an actual button will trigger all sorts of hover and action
    // behaviors in imgui, and sometimes we want buttony things grouped together, looking like
    // a single button, not individual buttons. hence draw_buttony_text.
    fn draw_buttony_text(&self, label: &str, color: [f32; 4]) {
        let style = self.ui.imgui().style();
        let padding = style.frame_padding;

        let original_cursor_pos = self.ui.get_cursor_pos();
        let label = im_str!("{}", label);
        let text_size = self.ui.calc_text_size(&label, false, 0.);
        let total_size = (text_size.x + (padding.x * 2.), text_size.y + (padding.y * 2.));

        let draw_cursor_pos = self.ui.get_cursor_screen_pos();
        let end_of_button_bg_rect =
            (draw_cursor_pos.0 + total_size.0, draw_cursor_pos.1 + total_size.1);
        self.ui
            .get_window_draw_list()
            .add_rect(draw_cursor_pos, end_of_button_bg_rect, color)
            .filled(true)
            .build();

        let draw_cursor_pos = self.ui.get_cursor_screen_pos();
        let buttony_text_start_cursor_pos =
            (draw_cursor_pos.0 + padding.x, draw_cursor_pos.1 + padding.y);
        let draw_list = self.ui.get_window_draw_list();
        let text_color = style.colors[ImGuiCol::Text as usize];
        draw_list.add_text(buttony_text_start_cursor_pos, text_color, label);
        self.ui.set_cursor_pos(original_cursor_pos);

        self.ui.invisible_button(&self.imlabel(""), total_size);
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
    fn draw_small_button<F: Fn() + 'static>(&self,
                                            label: &str,
                                            color: [f32; 4],
                                            on_button_activate: F) {
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
            .input_text_multiline(&self.imlabel(label), &mut box_input, (0., 100.))
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

        let mut flags = ImGuiInputTextFlags::empty();
        flags.set(ImGuiInputTextFlags::EnterReturnsTrue, true);

        let enter_pressed = self.ui
                                .input_text(&self.imlabel(label), &mut box_input)
                                .flags(flags)
                                .build();
        if enter_pressed {
            ondone();
            return;
        }

        if box_input.as_ref() as &str != existing_value {
            onchange(box_input.as_ref() as &str)
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
            items.into_iter().position(|i| is_item_selected(i)).unwrap() as i32;
        let previous_selection = selected_item_in_combo_box.clone();

        let formatted_items = items.into_iter()
                                   .map(|s| im_str!("{}", format_item(s)).clone())
                                   .collect_vec();

        self.ui.combo(&self.imlabel(label),
                      &mut selected_item_in_combo_box,
                      &formatted_items.iter().map(|s| s.as_ref()).collect_vec(),
                      5);
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
            if self.ui.selectable(&self.imlabel(&format_item(item)),
                                  is_item_selected(item),
                                  ImGuiSelectableFlags::empty(),
                                  (0., 0.))
            {
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
                    if self.ui.selectable(&self.imlabel(&label),
                                          is_selected,
                                          ImGuiSelectableFlags::empty(),
                                          (0., 0.))
                    {
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

    fn draw_main_menu_bar(&self, draw_menus: &dyn Fn()) {
        self.ui.main_menu_bar(draw_menus)
    }

    fn draw_menu(&self, label: &str, draw_menu_items: &dyn Fn()) {
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
