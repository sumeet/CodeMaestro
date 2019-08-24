// TODO: move Keypress
use super::editor::Keypress;

pub type Color = [f32; 4];

pub trait UiToolkit {
    type DrawResult;

    fn handle_global_keypress(&self, handle_keypress: impl Fn(Keypress) + 'static);
    fn draw_code_line_separator(&self,
                                plus_char: char,
                                width: f32,
                                height: f32,
                                color: Color)
                                -> Self::DrawResult;
    fn replace_on_hover(&self,
                        draw_when_not_hovered: &dyn Fn() -> Self::DrawResult,
                        draw_when_hovered: &dyn Fn() -> Self::DrawResult)
                        -> Self::DrawResult;
    fn draw_spinner(&self) -> Self::DrawResult;
    //    fn draw_iter(&self, i: impl Iterator<Item = DrawFn<Self>>) -> Self::DrawResult {
    //        self.draw_all(&[])
    //    }
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
    fn draw_child_region<F: Fn(Keypress) + 'static>(&self,
                                                    bg: Color,
                                                    draw_fn: &dyn Fn() -> Self::DrawResult,
                                                    height: ChildRegionHeight,
                                                    draw_context_menu: Option<&dyn Fn() -> Self::DrawResult>,
                                                    handle_keypress: Option<F>)
                                                    -> Self::DrawResult;
    fn draw_x_scrollable_list<'a>(&'a self,
                                  items: impl ExactSizeIterator<Item = (&'a dyn Fn()
                                                                   -> Self::DrawResult,
                                                            bool)>,
                                  lines_height: usize)
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
                                                                 onchange: F,
                                                                 ondone: D)
                                                                 -> Self::DrawResult;
    fn draw_text_input_with_label<F: Fn(&str) + 'static, D: Fn() + 'static>(&self,
                                                                            label: &str,
                                                                            existing_value: &str,
                                                                            onchange: F,
                                                                            ondone: D)
                                                                            -> Self::DrawResult;
    fn draw_multiline_text_input_with_label<F: Fn(&str) -> () + 'static>(&self,
                                                                         label: &str,
                                                                         existing_value: &str,
                                                                         onchange: F)
                                                                         -> Self::DrawResult;
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
    fn draw_main_menu_bar(&self, draw_menus: &dyn Fn() -> Self::DrawResult) -> Self::DrawResult;
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
    fn scrolled_to_y_if_not_visible(&self,
                                    scroll_hash: String,
                                    draw_fn: &dyn Fn() -> Self::DrawResult)
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

#[derive(Copy, Clone)]
pub enum ChildRegionHeight {
    ExpandFill { min_height: f32 },
    Pixels(usize),
}

pub type DrawFnRef<'a, T> = &'a dyn Fn() -> <T as UiToolkit>::DrawResult;

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
