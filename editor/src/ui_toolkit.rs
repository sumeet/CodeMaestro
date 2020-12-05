// TODO: move Keypress
use super::editor::Keypress;
use lazy_static::lazy_static;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

lazy_static! {
    static ref TK_CACHE: Arc<Mutex<TkCache>> = Arc::new(Mutex::new(TkCache::new()));
}

struct TkCache {
    forms: HashMap<u64, Vec<u8>>,
}

impl TkCache {
    fn new() -> Self {
        TkCache { forms: HashMap::new() }
    }
}

#[macro_export]
macro_rules! draw_all_iter {
    // creates a new scope so the variable doesn't leak out
    ($t:ident::$ui_toolkit:expr, $iterator:expr) => {{
        use itertools::Itertools;
        let __boxeds = $iterator.map(|f| {
                                    let b: Box<dyn Fn() -> $t::DrawResult> =
                                        std::boxed::Box::new(f);
                                    b
                                })
                                .collect_vec();
        $ui_toolkit.draw_all(&__boxeds.iter()
                                      .map(|boxed_draw_fn| boxed_draw_fn.as_ref())
                                      .collect_vec())
    }};
}

pub type Color = [f32; 4];

pub trait UiToolkit {
    type DrawResult;

    fn handle_global_keypress(&self, handle_keypress: impl Fn(Keypress) + 'static);
    fn open_file_open_dialog(callback: impl Fn(&[u8]) + 'static);
    fn open_file_save_dialog(filename_suggestion: &str, contents: &[u8], mimetype: &str);
    fn draw_code_line_separator(&self, width: f32, height: f32) -> Self::DrawResult;
    fn replace_on_hover(&self,
                        draw_when_not_hovered: &dyn Fn() -> Self::DrawResult,
                        draw_when_hovered: &dyn Fn() -> Self::DrawResult)
                        -> Self::DrawResult;
    fn draw_spinner(&self) -> Self::DrawResult;
    //    fn draw_iter(&self, i: impl Iterator<Item = DrawFn<Self>>) -> Self::DrawResult {
    //        self.draw_all(&[])
    //    }
    fn draw_all_with_no_spacing_in_between(&self,
                                           draw_fns: &[DrawFnRef<Self>])
                                           -> Self::DrawResult {
        // TODO: if this was implemented inside individual toolkit level, probably wouldn't have
        // to allocate with draw_all_iter! (which uses a box) but whatever
        let len = draw_fns.len();
        let to_draw =
            draw_fns.iter().enumerate().map(|(i, draw_fn)| {
                                           let is_last = i == len - 1;
                                           move || {
                                               if is_last {
                                                   draw_fn()
                                               } else {
                                                   self.draw_with_no_spacing_afterwards(draw_fn)
                                               }
                                           }
                                       });
        draw_all_iter!(Self::self, to_draw)
    }
    fn draw_all(&self, draw_fns: &[DrawFnRef<Self>]) -> Self::DrawResult;
    // if there's no `onclose` specified, then the window isn't closable and won't show a close button
    fn draw_centered_popup<F: Fn(Keypress) + 'static>(&self,
                                                      draw_fn: &dyn Fn() -> Self::DrawResult,
                                                      handle_keypress: Option<F>)
                                                      -> Self::DrawResult;
    fn draw_top_left_overlay(&self, draw_fn: &dyn Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_top_right_overlay(&self, draw_fn: &dyn Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_window<F: Fn(Keypress) + 'static, G: Fn() + 'static, H>(&self,
                                                                    window_name: &str,
                                                                    size: (usize, usize),
                                                                    pos: (isize, isize),
                                                                    draw_fn: &dyn Fn() -> Self::DrawResult,
                                                                    handle_keypress: Option<F>,
                                                                    onclose: Option<G>,
                                                                    onwindowchange: H)
                                                                    -> Self::DrawResult
        where H: Fn((isize, isize), (usize, usize)) + 'static;
    fn with_y_padding(&self, amount_px: u32, draw_fn: DrawFnRef<Self>) -> Self::DrawResult;
    fn draw_child_region<F: Fn(Keypress) + 'static, G: Fn() + 'static>(&self,
                                                                       bg: Color,
                                                                       draw_fn: &dyn Fn() -> Self::DrawResult,
                                                                       style: ChildRegionStyle,
                                                                       draw_context_menu: Option<&dyn Fn() -> Self::DrawResult>,
                                                                       handle_keypress: Option<F>,
                                                                       drag_selection_occurred: G)
                                                                       -> Self::DrawResult;
    fn draw_layout_with_bottom_bar(&self,
                                   draw_content_fn: &dyn Fn() -> Self::DrawResult,
                                   draw_bottom_bar_fn: &dyn Fn() -> Self::DrawResult)
                                   -> Self::DrawResult;
    fn draw_empty_line(&self) -> Self::DrawResult;
    fn draw_separator(&self) -> Self::DrawResult;
    //fn draw_in_columns<'a>(&self, draw_fns: &[Column<'a, Self>]) -> Self::DrawResult;
    fn draw_wrapped_text(&self, color: Color, text: &str) -> Self::DrawResult;
    fn draw_taking_up_full_width(&self, draw_fn: DrawFnRef<Self>) -> Self::DrawResult;
    fn draw_with_bgcolor(&self, bgcolor: Color, draw_fn: DrawFnRef<Self>) -> Self::DrawResult;
    fn draw_with_no_spacing_afterwards(&self, draw_fn: DrawFnRef<Self>) -> Self::DrawResult;
    fn draw_full_width_heading(&self,
                               bgcolor: Color,
                               inner_padding: (f32, f32),
                               text: &str)
                               -> Self::DrawResult;
    fn draw_text(&self, text: &str) -> Self::DrawResult;
    fn draw_with_margin(&self, padding: (f32, f32), draw_fn: DrawFnRef<Self>) -> Self::DrawResult;
    fn draw_text_with_label(&self, text: &str, label: &str) -> Self::DrawResult;
    fn buttonize<F: Fn() + 'static>(&self,
                                    draw_fn: &dyn Fn() -> Self::DrawResult,
                                    onclick: F)
                                    -> Self::DrawResult;
    fn draw_buttony_text(&self, label: &str, color: Color) -> Self::DrawResult;
    fn draw_disabled_button(&self, label: &str, color: Color) -> Self::DrawResult {
        self.draw_box_around([1., 1., 1., 0.2], &|| self.draw_buttony_text(label, color))
    }
    fn draw_button<F: Fn() + 'static>(&self,
                                      label: &str,
                                      color: Color,
                                      onclick: F)
                                      -> Self::DrawResult;
    fn draw_small_button<F: Fn() + 'static>(&self,
                                            label: &str,
                                            color: Color,
                                            onclick: F)
                                            -> Self::DrawResult;
    fn draw_text_box(&self, text: &str) -> Self::DrawResult;
    fn draw_whole_line_console_text_input(&self,
                                          ondone: impl Fn(&str) + 'static)
                                          -> Self::DrawResult;
    fn draw_text_input<F: Fn(&str) + 'static, D: Fn() + 'static>(&self,
                                                                 existing_value: &str,
                                                                 fit_input_width: bool,
                                                                 onchange: F,
                                                                 ondone: D,
                                                                 onkeypress: impl Fn(Keypress)
                                                                     + 'static)
                                                                 -> Self::DrawResult;
    fn callback_when_drag_intersects<F: Fn() + 'static>(&self,
                                                        draw_fn: DrawFnRef<Self>,
                                                        callback: F)
                                                        -> Self::DrawResult;
    fn drag_drop_source(&self,
                        source_id: impl std::hash::Hash + Clone + Debug,
                        draw_fn: DrawFnRef<Self>,
                        draw_preview_fn: DrawFnRef<Self>,
                        payload: impl Serialize)
                        -> Self::DrawResult;
    fn drag_drop_target<D: DeserializeOwned>(&self,
                                             draw_fn: DrawFnRef<Self>,
                                             draw_when_hovered: DrawFnRef<Self>,
                                             accepts_payload: impl Fn(D) + 'static)
                                             -> Self::DrawResult;
    fn draw_color_picker_with_label(&self,
                                    label: &str,
                                    existing_value: Color,
                                    onchange: impl Fn(Color) + 'static)
                                    -> Self::DrawResult;
    fn draw_text_input_with_label<F: Fn(&str) + 'static, D: Fn() + 'static>(&self,
                                                                            label: &str,
                                                                            existing_value: &str,
                                                                            onchange: F,
                                                                            ondone: D)
                                                                            -> Self::DrawResult;
    fn draw_multiline_text_input_with_label<F: Fn(&str) -> () + 'static, E: Fn() -> () + 'static>(
        &self,
        label: &str,
        existing_value: &str,
        onchange: F,
        onenter: E)
        -> Self::DrawResult;

    fn draw_form<T: Serialize + DeserializeOwned + 'static, R>(&self,
                                                               form_id: u64,
                                                               initial_values: T,
                                                               draw_form_fn: &dyn Fn(&T) -> R)
                                                               -> R {
        let mut cache = TK_CACHE.lock().unwrap();
        let entry = cache.forms
                         .entry(form_id)
                         .or_insert_with(|| bincode::serialize(&initial_values).unwrap());
        draw_form_fn(&bincode::deserialize(entry.as_slice()).unwrap())
    }

    fn submit_form<T: Serialize + DeserializeOwned + 'static>(form_id: u64) -> T {
        let t = TK_CACHE.lock().unwrap().forms.remove(&form_id).unwrap();
        bincode::deserialize(t.as_slice()).unwrap()
    }

    fn change_form<T: Serialize + DeserializeOwned + 'static>(form_id: u64, to: T) {
        TK_CACHE.lock()
                .unwrap()
                .forms
                .insert(form_id, bincode::serialize(&to).unwrap());
    }

    fn draw_combo_box_with_label<F, G, H, T>(&self,
                                             label: &str,
                                             is_item_selected: G,
                                             format_item: H,
                                             items: &[&T],
                                             onchange: F)
                                             -> Self::DrawResult
        where T: Clone + 'static,
              F: Fn(&T) -> () + 'static,
              G: Fn(&T) -> bool,
              H: Fn(&T) -> String;
    fn draw_selectables<F, G, H, T>(&self,
                                    is_item_selected: G,
                                    format_item: H,
                                    items: &[&T],
                                    onchange: F)
                                    -> Self::DrawResult
        where T: Clone + 'static,
              F: Fn(&T) -> () + 'static,
              G: Fn(&T) -> bool,
              H: Fn(&T) -> &str;
    fn draw_selectables2<T, F: Fn(&T) -> () + 'static>(&self,
                                                       items: Vec<SelectableItem<T>>,
                                                       onselect: F)
                                                       -> Self::DrawResult;
    fn draw_checkbox_with_label<F: Fn(bool) + 'static>(&self,
                                                       label: &str,
                                                       value: bool,
                                                       onchange: F)
                                                       -> Self::DrawResult;
    fn draw_all_on_same_line(&self, draw_fns: &[DrawFnRef<Self>]) -> Self::DrawResult;
    fn draw_box_around(&self,
                       color: [f32; 4],
                       draw_fn: &dyn Fn() -> Self::DrawResult)
                       -> Self::DrawResult;
    fn draw_top_border_inside(&self,
                              color: [f32; 4],
                              thickness: u8,
                              draw_fn: &dyn Fn() -> Self::DrawResult)
                              -> Self::DrawResult;
    fn draw_right_border_inside(&self,
                                color: [f32; 4],
                                thickness: u8,
                                draw_fn: &dyn Fn() -> Self::DrawResult)
                                -> Self::DrawResult;
    fn draw_left_border_inside(&self,
                               color: [f32; 4],
                               thickness: u8,
                               draw_fn: &dyn Fn() -> Self::DrawResult)
                               -> Self::DrawResult;
    fn draw_bottom_border_inside(&self,
                                 color: [f32; 4],
                                 thickness: u8,
                                 draw_fn: &dyn Fn() -> Self::DrawResult)
                                 -> Self::DrawResult;
    fn draw_statusbar(&self, draw_fn: &dyn Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_main_menu_bar(&self, draw_menus: &[DrawFnRef<Self>]) -> Self::DrawResult;
    fn draw_menu(&self,
                 label: &str,
                 draw_menu_items: &dyn Fn() -> Self::DrawResult)
                 -> Self::DrawResult;
    fn draw_menu_item<F: Fn() + 'static>(&self, label: &str, onselect: F) -> Self::DrawResult;
    //    fn draw_tree_node(&self, label: &str, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    //    fn draw_tree_leaf(&self, label: &str, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn focused(&self, draw_fn: &dyn Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn indent(&self, px: i16, draw_fn: &dyn Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn align(&self,
             lhs: &dyn Fn() -> Self::DrawResult,
             rhs: &[&dyn Fn() -> Self::DrawResult])
             -> Self::DrawResult;
    fn align_fill_lhs(&self,
                      x_padding_left_block_hack: i8,
                      lhs: &dyn Fn() -> Self::DrawResult,
                      lhs_color: Color,
                      rhs: &[&dyn Fn() -> Self::DrawResult])
                      -> Self::DrawResult;
    fn scrolled_to_y_if_not_visible(&self,
                                    scroll_hash: String,
                                    draw_fn: &dyn Fn() -> Self::DrawResult)
                                    -> Self::DrawResult;
    fn context_menu(&self,
                    draw_fn: DrawFnRef<Self>,
                    draw_context_menu: DrawFnRef<Self>)
                    -> Self::DrawResult;
    fn draw_columns<const N: usize>(&self,
                                    draw_fn_groups: &[[DrawFnRef<Self>; N]])
                                    -> Self::DrawResult;
}

pub enum SelectableItem<T: 'static> {
    GroupHeader(&'static str),
    Selectable {
        item: T,
        label: String,
        is_selected: bool,
    },
}

#[derive(Debug, Copy, Clone)]
pub struct ChildRegionStyle {
    pub height: ChildRegionHeight,
    pub width: ChildRegionWidth,
    pub frame_style: ChildRegionFrameStyle,
    pub top_padding: ChildRegionTopPadding,
}

#[derive(Debug, Copy, Clone)]
pub enum ChildRegionTopPadding {
    Default,
    None,
}

#[derive(Debug, Copy, Clone)]
pub enum ChildRegionHeight {
    FitContent,
    Max(usize),
    ExpandFill { min_height: f32 },
    Pixels(usize),
}

#[derive(Debug, Copy, Clone)]
pub enum ChildRegionWidth {
    FitContent,
    All,
}

#[derive(Debug, Copy, Clone)]
pub enum ChildRegionFrameStyle {
    Framed,
    NoFrame,
}

pub type DrawFnRef<'a, T> = &'a dyn Fn() -> <T as UiToolkit>::DrawResult;

#[macro_export]
macro_rules! align {
    // creates a new scope so the variable doesn't leak out
    ($t:ident::$ui_toolkit:expr, $lhs_draw_fn_ref: expr, $rhs_iterator:expr) => {{
        use itertools::Itertools;
        let __boxeds = $rhs_iterator.map(|f| {
                                        let b: Box<dyn Fn() -> $t::DrawResult> =
                                            std::boxed::Box::new(f);
                                        b
                                    })
                                    .collect_vec();
        $ui_toolkit.align($lhs_draw_fn_ref,
                          &__boxeds.iter()
                                   .map(|boxed_draw_fn| boxed_draw_fn.as_ref())
                                   .collect_vec())
    }};
}

//pub struct Column<'a, T: UiToolkit + ?Sized> {
//    pub draw_fn: DrawFnRef<'a, T>,
//    pub percentage: f32,
//}
//
//impl<'a, T: UiToolkit> Column<'a, T> {
//    pub fn _new(percentage: f32, draw_fn: DrawFnRef<'a, T>) -> Self {
//        Self { draw_fn,
//               percentage }
//    }
//}
