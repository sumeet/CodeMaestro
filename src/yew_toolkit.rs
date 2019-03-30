use super::async_executor::AsyncExecutor;
use super::editor;
use super::editor::{Key as AppKey, Keypress};
use super::{App as CSApp, UiToolkit};
use crate::ui_toolkit::SelectableItem;
use itertools::Itertools;
use std::cell::RefCell;
use std::rc::Rc;
use stdweb::traits::IEvent;
use stdweb::traits::IKeyboardEvent;
use stdweb::web::{document, IElement, IEventTarget};
//use stdweb::{__internal_console_unsafe, console};
use stdweb::{_js_impl, js};
use yew::html;
use yew::prelude::*;

pub struct Model {
    app: Option<Rc<RefCell<CSApp>>>,
    async_executor: Option<AsyncExecutor>,
    renderer_state: Option<Rc<RefCell<RendererState>>>,
}

pub enum Msg {
    Init(Rc<RefCell<CSApp>>,
         AsyncExecutor,
         Rc<RefCell<RendererState>>),
    Redraw,
    DontRedraw,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, _link: ComponentLink<Self>) -> Self {
        Model { app: None,
                async_executor: None,
                renderer_state: None }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::Init(app, mut async_executor, renderer_state) => {
                // flush commands initially before rendering for the first time
                app.borrow_mut().flush_commands(&mut async_executor);

                self.async_executor = Some(async_executor);
                self.app = Some(app);
                self.renderer_state = Some(renderer_state);
                true
            }
            Msg::Redraw => {
                if let (Some(app), Some(mut async_executor)) =
                    (self.app.as_ref(), self.async_executor.as_mut())
                {
                    app.borrow_mut().flush_commands(&mut async_executor);
                }
                true
            }
            Msg::DontRedraw => false,
        }
    }
}

// keep this in sync with the WindowBg color defined in imgui_support.rs
const WINDOW_BG_COLOR: [f32; 4] = [0.375, 0.375, 0.375, 1.0];
const WINDOW_TITLE_BG_COLOR: [f32; 4] = [0.408, 0.408, 0.678, 1.0];

struct YewToolkit {
    last_drawn_element_id: RefCell<u32>,
    focused_element_id: RefCell<u32>,
    renderer_state: Rc<RefCell<RendererState>>,
}

impl UiToolkit for YewToolkit {
    type DrawResult = Html<Model>;

    fn draw_top_right_overlay(&self, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult {
        html! {
            <div style={ format!("position: absolute; top: 10px; right: 10px; color: white; background-color: {}",self.rgba(WINDOW_BG_COLOR)) }, >
                {{ draw_fn() }}
            </div>
        }
    }

    fn draw_spinner(&self) -> Self::DrawResult {
        html! {
            <div class="spinner", >
                {" "}
            </div>
        }
    }

    fn draw_x_scrollable_list<'a>(&'a self,
                                  items: impl ExactSizeIterator<Item = (&'a Fn()
                                                                   -> Self::DrawResult,
                                                            bool)>,
                                  lines_height: usize)
                                  -> Self::DrawResult {
        let items = items.map(|(draw_fn, is_selected)| {
                             let drawn = draw_fn();
                             let last_drawn_id = *self.last_drawn_element_id.borrow();
                             if is_selected {
                                 self.renderer_state.borrow().add_run_after_render(move || {
                                    js! {
                                        var el = document.getElementById(@{last_drawn_id});
                                        if (el) {
                                            el.scrollIntoView({inline: "center"});
                                        }
                                    }
                                 });
                             }
                             drawn
                         })
                         .collect_vec();
        html! {
            //   TODO: margin-bottom is HAXXX
            <div style={format!("display: flex; height: {}em; max-height: {}em; overflow: hidden; margin-bottom: 0.2em;", lines_height, lines_height)}, >
                { for items.into_iter().map(|drawn| html! {
                    <div style="white-space: nowrap;", >
                        { drawn }
                    </div>
                })}
            </div>
        }
    }

    fn draw_centered_popup<F: Fn(Keypress) + 'static>(&self,
                                                      draw_fn: &Fn() -> Self::DrawResult,
                                                      handle_keypress: Option<F>)
                                                      -> Self::DrawResult {
        let handle_keypress_1 = Rc::new(move |keypress: Keypress| {
            if let Some(handle_keypress) = &handle_keypress {
                handle_keypress(keypress)
            }
        });
        let handle_keypress_2 = Rc::clone(&handle_keypress_1);
        let global_keydown_handler = self.global_keydown_handler();
        html! {
            <div style={ format!("background-color: {}; width: 300px; height: 200px; position: absolute; top: calc(50% - 300px); left: calc(50% - 300px); color: white; overflow: auto;", self.rgba(WINDOW_BG_COLOR)) },
                 id={ self.incr_last_drawn_element_id().to_string() },
                 tabindex=0,
                 onkeypress=|e| {
                     if let Some(keypress) = map_keypress_event(&e) {
                         handle_keypress_1(keypress);
                     }
                     e.prevent_default();
                     Msg::Redraw
                 },
                 onkeydown=|e| {
                     global_keydown_handler(&e);
                     // lol for these special keys we have to listen on keydown, but the
                     // rest we can do keypress :/
                     if e.key() == "Tab" || e.key() == "Escape" || e.key() == "Esc" ||
                         // LOL this is for ctrl+r
                         ((e.key() == "r" || e.key() == "R") && e.ctrl_key()) {
                         //console!(log, e.key());
                         if let Some(keypress) = map_keypress_event(&e) {
                             //console!(log, format!("{:?}", keypress));
                             handle_keypress_2(keypress);
                         }
                         e.prevent_default();
                         Msg::Redraw
                     } else {
                         Msg::DontRedraw
                     }
                 }, >
                {{ draw_fn() }}
            </div>
        }
    }

    fn draw_all(&self, draw_results: Vec<Self::DrawResult>) -> Self::DrawResult {
        html! {
            <div>
                { for draw_results.into_iter() }
            </div>
        }
    }

    fn draw_separator(&self) -> Self::DrawResult {
        html! {
            <hr>
        }
    }

    fn draw_text_input_with_label<F: Fn(&str) -> () + 'static, D: Fn() + 'static>(
        &self,
        label: &str,
        existing_value: &str,
        onchange: F,
        ondone: D)
        -> Self::DrawResult {
        html! {
            <div>
                {{ self.draw_text_input(existing_value, onchange, ondone) }}
                <label>{{ label }}</label>
            </div>
        }
    }

    fn draw_checkbox_with_label<F: Fn(bool) + 'static>(&self,
                                                       label: &str,
                                                       value: bool,
                                                       onchange: F)
                                                       -> Self::DrawResult {
        html! {
            <div>
                <input type="checkbox", checked=value, onclick=|_| { onchange(!value) ; Msg::Redraw }, />
                <label>{{ label }}</label>
            </div>
        }
    }

    fn draw_text_with_label(&self, text: &str, label: &str) -> Self::DrawResult {
        html! {
            <div>
                <p>{{ text }}</p>
                <label>{{ label }}</label>
            </div>
        }
    }

    fn draw_multiline_text_input_with_label<F: Fn(&str) -> () + 'static>(&self,
                                                                         label: &str,
                                                                         existing_value: &str,
                                                                         onchange: F)
                                                                         -> Self::DrawResult {
        html! {
            <div>
                <textarea rows=5, value=existing_value,
                          oninput=|e| { onchange(&e.value) ; Msg::Redraw }, >
                </textarea>
                <label>{{ label }}</label>
            </div>
        }
    }

    fn draw_window<F: Fn(Keypress) + 'static, G: Fn() + 'static, H>(&self,
                                                                    window_name: &str,
                                                                    size: (usize, usize),
                                                                    pos: (isize, isize),
                                                                    f: &Fn() -> Self::DrawResult,
                                                                    handle_keypress: Option<F>,
                                                                    onclose: Option<G>,
                                                                    onwindowchange: H)
                                                                    -> Self::DrawResult
        where H: Fn((isize, isize), (usize, usize)) + 'static
    {
        // if there's a keypress handler provided, then send those keypresses into the app, and like,
        // prevent the tab key from doing anything
        let handle_keypress_1 = Rc::new(move |keypress: Keypress| {
            if let Some(handle_keypress) = &handle_keypress {
                handle_keypress(keypress)
            }
        });
        let handle_keypress_2 = Rc::clone(&handle_keypress_1);
        let global_keydown_handler = self.global_keydown_handler();
        html! {
           <div class="window", style={ format!("color: white; background-color: {}", self.rgba(WINDOW_BG_COLOR)) },
                id={ self.incr_last_drawn_element_id().to_string() },
                tabindex=0,
                onkeypress=|e| {
                    if let Some(keypress) = map_keypress_event(&e) {
                        handle_keypress_1(keypress);
                    }
                    e.prevent_default();
                    Msg::Redraw
                },
                onkeydown=|e| {
                    global_keydown_handler(&e);
                    // lol for these special keys we have to listen on keydown, but the
                    // rest we can do keypress :/
                    if e.key() == "Tab" || e.key() == "Escape" || e.key() == "Esc" ||
                        // LOL this is for ctrl+r
                        ((e.key() == "r" || e.key() == "R") && e.ctrl_key()) {
                        //console!(log, e.key());
                        if let Some(keypress) = map_keypress_event(&e) {
                            //console!(log, format!("{:?}", keypress));
                            handle_keypress_2(keypress);
                        }
                        e.prevent_default();
                        Msg::Redraw
                    } else {
                        Msg::DontRedraw
                    }
                }, >

                <h4 class="window-title", style={ format!("background-color: {}; color: white", self.rgba(WINDOW_TITLE_BG_COLOR)) },>
                    { if let Some(onclose) = onclose {
                        html! {
                            <div style="float: right; cursor: pointer;", onclick=|_| { onclose(); Msg::Redraw }, >
                                { "X" }
                            </div>
                        }
                    } else {
                        html! { <div></div> }
                    } }
                    { window_name }
                </h4>
                <div class="window-content",>
                    { f() }
                </div>
            </div>
        }
    }

    // TODO: clean up bc code is duped between here and draw_code_window
    fn draw_child_region<F: Fn(Keypress) + 'static>(&self,
                                                    draw_fn: &Fn() -> Self::DrawResult,
                                                    height_percentage: f32,
                                                    handle_keypress: Option<F>)
                                                    -> Self::DrawResult {
        // if there's a keypress handler provided, then send those keypresses into the app, and like,
        // prevent the tab key from doing anything
        if let Some(handle_keypress) = handle_keypress {
            let handle_keypress_1 = Rc::new(handle_keypress);
            let handle_keypress_2 = Rc::clone(&handle_keypress_1);
            let global_keydown_handler = self.global_keydown_handler();
            html! {
                <div style={ format!("height: calc({}%); white-space: nowrap;", height_percentage * 100.) },
                    id={ self.incr_last_drawn_element_id().to_string() },
                    tabindex=0,
                    onkeypress=|e| {
                        if let Some(keypress) = map_keypress_event(&e) {
                            handle_keypress_1(keypress);
                        }
                        e.prevent_default();
                        Msg::Redraw
                    },
                    onkeydown=|e| {
                        global_keydown_handler(&e);
                        // lol for these special keys we have to listen on keydown, but the
                        // rest we can do keypress :/
                        if e.key() == "Tab" || e.key() == "Escape" || e.key() == "Esc" ||
                            // LOL this is for ctrl+r
                            ((e.key() == "r" || e.key() == "R") && e.ctrl_key()) {
                            //console!(log, e.key());
                            if let Some(keypress) = map_keypress_event(&e) {
                                //console!(log, format!("{:?}", keypress));
                                handle_keypress_2(keypress);
                            }
                            e.prevent_default();
                            Msg::Redraw
                        } else {
                            Msg::DontRedraw
                        }
                    }, >

                    { draw_fn() }
                </div>
            }
        } else {
            html! {
                <div style={ format!("height: {}%", height_percentage) },
                    id={ self.incr_last_drawn_element_id().to_string() },
                    tabindex=0, >
                    { draw_fn() }
                </div>
            }
        }
    }

    fn draw_layout_with_bottom_bar(&self,
                                   draw_content_fn: &Fn() -> Self::DrawResult,
                                   draw_bottom_bar_fn: &Fn() -> Self::DrawResult)
                                   -> Self::DrawResult {
        // TODO: this only renders the bottom bar directly under the content. the bottom bar needs
        // to be fixed at the bottom
        html! {
            <div id={ self.incr_last_drawn_element_id().to_string() }, >
                { draw_content_fn() }
                { self.draw_empty_line() }
                { self.draw_empty_line() }
                { draw_bottom_bar_fn() }
            </div>
        }
    }

    fn draw_empty_line(&self) -> Self::DrawResult {
        html! {
            <br id={ self.incr_last_drawn_element_id().to_string() }, />
        }
    }

    fn buttonize<F: Fn() + 'static>(&self,
                                    draw_fn: &Fn() -> Self::DrawResult,
                                    onclick: F)
                                    -> Self::DrawResult {
        html! {
            <div onclick=|_| { onclick(); Msg::Redraw }, >
                { draw_fn() }
            </div>
        }
    }

    fn draw_buttony_text(&self, label: &str, color: [f32; 4]) -> Self::DrawResult {
        self.draw_button(label, color, &|| {})
    }

    fn draw_button<F: Fn() + 'static>(&self,
                                      label: &str,
                                      color: [f32; 4],
                                      on_button_press_callback: F)
                                      -> Self::DrawResult {
        html! {
            <button id={ self.incr_last_drawn_element_id().to_string() },
                 style=format!("color: white; background-color: {}; display: block; border: none; outline: none;", self.rgba(color)),
                 onclick=|_| { on_button_press_callback(); Msg::Redraw }, >
            { symbolize_text(label) }
            </button>
        }
    }

    fn draw_small_button<F: Fn() + 'static>(&self,
                                            label: &str,
                                            color: [f32; 4],
                                            on_button_press_callback: F)
                                            -> Self::DrawResult {
        html! {
            <button id={ self.incr_last_drawn_element_id().to_string() },
                 style=format!("display: block; font-size: 75%; color: white; background-color: {}; border: none; outline: none;", self.rgba(color)),
                 onclick=|_| { on_button_press_callback(); Msg::Redraw }, >
            { label }
            </button>
        }
    }

    fn draw_text_box(&self, text: &str) -> Self::DrawResult {
        html! {
            <textarea readonly={true}, id={ self.incr_last_drawn_element_id().to_string() },>{ text }</textarea>
        }
    }

    fn draw_all_on_same_line(&self, draw_fns: &[&Fn() -> Self::DrawResult]) -> Self::DrawResult {
        html! {
            <div id={ self.incr_last_drawn_element_id().to_string() },
                 style={"display: flex;"}, >
                { for draw_fns.into_iter().map(|draw_fn| html! {
                    <div>
                        { draw_fn() }
                    </div>
                })}
            </div>
        }
    }

    fn draw_text_input<F: Fn(&str) -> () + 'static, D: Fn() + 'static>(&self,
                                                                       existing_value: &str,
                                                                       onchange: F,
                                                                       ondone: D)
                                                                       -> Self::DrawResult {
        let ondone = Rc::new(ondone);
        let ondone2 = Rc::clone(&ondone);
        html! {
            <input type="text",
               style="display: block;",
               autocomplete="off",
               id={ self.incr_last_drawn_element_id().to_string() },
               value=existing_value,
               oninput=|e| {onchange(&e.value) ; Msg::Redraw},
               onkeypress=|e| { if e.key() == "Enter" { ondone2() } ; Msg::Redraw }, />
        }
    }

    fn draw_text(&self, text: &str) -> Self::DrawResult {
        let text = text.replace(" ", " ");
        html! {
            <span>{ symbolize_text(&text) }</span>
        }
    }

    fn focused(&self, draw_fn: &Fn() -> Html<Model>) -> Self::DrawResult {
        let html = draw_fn();
        self.focus_last_drawn_element();
        html
    }

    fn draw_main_menu_bar(&self, draw_menus: &Fn() -> Self::DrawResult) -> Self::DrawResult {
        html! {
            <div>
                {{ draw_menus() }}
            </div>
        }
    }

    fn draw_menu(&self,
                 label: &str,
                 draw_menu_items: &Fn() -> Self::DrawResult)
                 -> Self::DrawResult {
        // TODO: implement this for realsies
        html! {
            <div style="display: flex;", >
                <p>{label}</p>
                {{ draw_menu_items() }}
            </div>
        }
    }

    fn draw_menu_item<F: Fn() + 'static>(&self, label: &str, onselect: F) -> Self::DrawResult {
        // TODO: do this for realsies
        html! {
            <div style="float: left;",>
                {{ self.draw_button(label, editor::GREY_COLOR, onselect) }}
            </div>
        }
    }

    fn draw_statusbar(&self, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult {
        html! {
            <div style="position: fixed; line-height: 1; width: 100%; bottom: 0; left: 0;",>
                {{ draw_fn() }}
            </div>
        }
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
              H: Fn(&T) -> String
    {
        let formatted_items = items.into_iter().map(|i| format_item(i)).collect_vec();
        let selected_item_in_combo_box = items.into_iter().position(|i| is_item_selected(i));
        let items = items.into_iter().map(|i| (*i).clone()).collect_vec();
        html! {
            <select onchange=|event| {
                        match event {
                            ChangeData::Select(elem) => {
                                if let Some(selected_index) = elem.selected_index() {
                                    onchange(&items[selected_index as usize]);
                                }
                                Msg::Redraw
                            }
                            _ => {
                                unreachable!();
                            }
                        }
                    },>
                { for formatted_items.into_iter().enumerate().map(|(index, item)| {
                    let selected = Some(index) == selected_item_in_combo_box;
                    if selected {
                        html! {
                            <option selected=true, >
                                { item }
                            </option>
                        }
                    } else {
                        html! {
                            <option>
                                { item }
                            </option>
                        }
                    }
                })}
            </select>
            <label>{ label }</label>
        }
    }

    // TODO: make this NOT a total copy and paste of draw_combo_box_with_label
    fn draw_selectables<F, G, H, T>(&self,
                                    is_item_selected: G,
                                    format_item: H,
                                    items: &[&T],
                                    onchange: F)
                                    -> Self::DrawResult
        where T: Clone + 'static,
              F: Fn(&T) -> () + 'static,
              G: Fn(&T) -> bool,
              H: Fn(&T) -> &str
    {
        let formatted_items = items.into_iter().map(|i| format_item(i)).collect_vec();
        let selected_item_in_combo_box = items.into_iter().position(|i| is_item_selected(i));
        let items = items.into_iter().map(|i| (*i).clone()).collect_vec();
        html! {
            <select size={items.len().to_string()}, onchange=|event| {
                        match event {
                            ChangeData::Select(elem) => {
                                if let Some(selected_index) = elem.selected_index() {
                                    onchange(&items[selected_index as usize]);
                                }
                                Msg::Redraw
                            }
                            _ => {
                                unreachable!();
                            }
                        }
                    },>
                { for formatted_items.into_iter().enumerate().map(|(index, item)| {
                    let selected = Some(index) == selected_item_in_combo_box;
                    if selected {
                        html! {
                            <option selected=true, >
                                { item }
                            </option>
                        }
                    } else {
                        html! {
                            <option>
                                { item }
                            </option>
                        }
                    }
                })}
            </select>
        }
    }

    fn draw_selectables2<T, F: Fn(&T) -> () + 'static>(&self,
                                                       items: Vec<SelectableItem<T>>,
                                                       onselect: F)
                                                       -> Self::DrawResult {
        let items = Rc::new(items);
        let items_rc = Rc::clone(&items);
        html! {
            <select style="overflow: hidden;", size={items_rc.len().to_string()}, onchange=|event| {
                match event {
                        ChangeData::Select(elem) => {
                            if let Some(selected_index) = elem.selected_index() {
                                match &items_rc[selected_index as usize] {
                                    SelectableItem::Selectable { item, .. } => onselect(item),
                                    SelectableItem::GroupHeader(_) => panic!("expected a selectable here"),
                                }
                            }
                            Msg::Redraw
                        }
                        _ => {
                            unreachable!();
                        }
                    }
            },>
                { for items.iter().map(|item| match item {
                    SelectableItem::Selectable { label, is_selected, .. } => {
                        if *is_selected {
                            html! {
                                <option selected=true, >
                                    { label }
                                </option>
                            }
                        } else {
                            html! {
                                <option>
                                    { label }
                                </option>
                            }
                        }
                    },
                    SelectableItem::GroupHeader(label) => {
                        html! {
                            <option disabled=true, >
                                { label }
                            </option>
                        }
                    }

                }) }
            </select>
        }
    }

    fn draw_box_around(&self,
                       color: [f32; 4],
                       draw_fn: &Fn() -> Self::DrawResult)
                       -> Self::DrawResult {
        html! {
            <div class={"overlay-wrapper"}, >
                <div>
                     { draw_fn() }
                 </div>
                 <div class={"overlay"},
                      style={ format!("top: 0, left: 0; height: 100%; background-color: {}", self.rgba(color)) }, >
                      {" "}
                 </div>
             </div>
        }
    }

    fn draw_top_border_inside(&self,
                              color: [f32; 4],
                              thickness: u8,
                              draw_fn: &Fn() -> Self::DrawResult)
                              -> Self::DrawResult {
        html! {
            <div class={"overlay-wrapper"}, >
                <div>
                     { draw_fn() }
                 </div>
                 <div class={"overlay"},
                      style={ format!("height: {}px; background-color: {}", thickness, self.rgba(color)) }, >
                      {" "}
                 </div>
             </div>
        }
    }

    fn draw_right_border_inside(&self,
                                color: [f32; 4],
                                thickness: u8,
                                draw_fn: &Fn() -> Self::DrawResult)
                                -> Self::DrawResult {
        html! {
            <div class={"overlay-wrapper"}, >
                <div>
                     { draw_fn() }
                 </div>
                 <div class={"overlay-bottom-right"},
                      style={ format!("height: 100%; width: {}px; background-color: {}", thickness, self.rgba(color)) }, >
                      {" "}
                 </div>
             </div>
        }
    }

    fn draw_left_border_inside(&self,
                               color: [f32; 4],
                               thickness: u8,
                               draw_fn: &Fn() -> Self::DrawResult)
                               -> Self::DrawResult {
        html! {
            <div class={"overlay-wrapper"}, >
                <div>
                     { draw_fn() }
                 </div>
                 <div class={"overlay"},
                      style={ format!("height: 100%; width: {}px; background-color: {}", thickness, self.rgba(color)) }, >
                      {" "}
                 </div>
             </div>
        }
    }

    fn draw_bottom_border_inside(&self,
                                 color: [f32; 4],
                                 thickness: u8,
                                 draw_fn: &Fn() -> Self::DrawResult)
                                 -> Self::DrawResult {
        html! {
            <div class={"overlay-wrapper"}, >
                <div>
                     { draw_fn() }
                 </div>
                 <div class={"overlay-bottom-right"},
                      style={ format!("width: 100%; height: {}px; background-color: {}", thickness, self.rgba(color)) }, >
                      {" "}
                 </div>
             </div>
        }
    }

    fn indent(&self, px: i16, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult {
        html! {
            <div style=format!("margin-left: {}px", px), >
                { draw_fn() }
            </div>
        }
    }

    fn align(&self,
             lhs: &Fn() -> Self::DrawResult,
             rhs: &[&Fn() -> Self::DrawResult])
             -> Self::DrawResult {
        html! {
            <div>
                <div style={"display: inline-block; vertical-align: top;"},>
                    { lhs() }
                </div>
                <div style={"display: inline-block; vertical-align: top;"}, >
                    { for rhs.into_iter().map(|draw_fn| html! {
                        { draw_fn() }
                    })}
                </div>
            </div>
        }
    }

    fn handle_global_keypress(&self, handle_keypress: impl Fn(Keypress) + 'static) {
        self.renderer_state
            .borrow()
            .set_global_keydown_handler(handle_keypress);
    }
}

impl YewToolkit {
    fn new(renderer_state: Rc<RefCell<RendererState>>) -> Self {
        YewToolkit { last_drawn_element_id: RefCell::new(0),
                     focused_element_id: RefCell::new(0),
                     renderer_state }
    }

    fn focus_last_drawn_element(&self) {
        self.focused_element_id
            .replace(self.get_last_drawn_element_id());
    }

    fn rgba(&self, color: [f32; 4]) -> String {
        format!("rgba({}, {}, {}, {})",
                color[0] * 255.0,
                color[1] * 255.0,
                color[2] * 255.0,
                color[3])
    }

    fn incr_last_drawn_element_id(&self) -> u32 {
        let next_id = *self.last_drawn_element_id.borrow() + 1;
        self.last_drawn_element_id.replace(next_id);
        next_id
    }

    fn get_last_drawn_element_id(&self) -> u32 {
        *self.last_drawn_element_id.borrow()
    }

    fn get_focused_element_id(&self) -> u32 {
        *self.focused_element_id.borrow()
    }

    fn global_keydown_handler(&self) -> impl Fn(&KeyDownEvent) + 'static {
        let renderer_state = Rc::clone(&self.renderer_state);
        move |e| {
            renderer_state.borrow().handle_global_key(e);
        }
    }
}

pub struct RendererState {
    pub global_key_handler: Rc<RefCell<Box<Fn(Keypress) + 'static>>>,
    pub yew_app: Rc<RefCell<html::Scope<Model>>>,
    pub funcs_to_run_after_render: Rc<RefCell<Vec<Box<Fn()>>>>,
}

impl RendererState {
    pub fn new(yew_app: html::Scope<Model>,
               funcs_to_run_after_render: Rc<RefCell<Vec<Box<Fn()>>>>)
               -> Self {
        Self { global_key_handler: Rc::new(RefCell::new(Box::new(|_| {}))),
               yew_app: Rc::new(RefCell::new(yew_app)),
               funcs_to_run_after_render }
    }

    pub fn run_all_after_render(&self) {
        for func in self.funcs_to_run_after_render.borrow_mut().drain(..) {
            func()
        }
    }

    pub fn add_run_after_render(&self, fun: impl Fn() + 'static) {
        self.funcs_to_run_after_render
            .borrow_mut()
            .push(Box::new(fun))
    }

    pub fn send_msg(&self, msg: Msg) {
        self.yew_app.borrow_mut().send_message(msg);
    }

    pub fn handle_global_key(&self, e: &KeyDownEvent) {
        // TODO: we know we have to capture C-o here because it can open the fuzzy finder
        // globally. unfortunately, for now, we have to manually bind all global hotkeys
        // like this.
        if (e.key() == "o" || e.key() == "O") && e.ctrl_key() {
            if let Some(keypress) = map_keypress_event(e) {
                (self.global_key_handler.borrow())(keypress);
                e.prevent_default();
                self.send_msg(Msg::Redraw)
            }
        }
    }

    pub fn set_global_keydown_handler(&self, handle_keypress: impl Fn(Keypress) + 'static) {
        self.global_key_handler.replace(Box::new(handle_keypress));
    }
}

impl Renderable<Model> for Model {
    fn view(&self) -> Html<Self> {
        if let (Some(app), Some(renderer_state)) = (&self.app, &self.renderer_state) {
            let mut tk = YewToolkit::new(Rc::clone(renderer_state));
            let drawn = app.borrow_mut().draw(&mut tk);
            document().body()
                      .unwrap()
                      .set_attribute("data-focused-id", &tk.get_focused_element_id().to_string())
                      .unwrap();

            drawn
        } else {
            html! { <p> {"No app"} </p> }
        }
    }
}

fn map_keypress_event<F: IKeyboardEvent>(keypress_event: &F) -> Option<Keypress> {
    let keystring_from_event = keypress_event.key();
    let appkey = map_key(&keystring_from_event)?;
    let was_shift_pressed =
        keypress_event.shift_key() || was_shift_key_pressed(&keystring_from_event);
    Some(Keypress::new(appkey, keypress_event.ctrl_key(), was_shift_pressed))
}

fn map_key(key: &str) -> Option<AppKey> {
    match key.to_lowercase().as_ref() {
        "a" => Some(AppKey::A),
        "b" => Some(AppKey::B),
        "c" => Some(AppKey::C),
        "h" => Some(AppKey::H),
        "l" => Some(AppKey::L),
        "d" => Some(AppKey::D),
        "w" => Some(AppKey::W),
        "x" => Some(AppKey::X),
        "r" => Some(AppKey::R),
        "o" => Some(AppKey::O),
        "u" => Some(AppKey::U),
        "v" => Some(AppKey::V),
        "tab" => Some(AppKey::Tab),
        "arrowleft" => Some(AppKey::LeftArrow),
        "arrowright" => Some(AppKey::RightArrow),
        "esc" | "escape" => Some(AppKey::Escape),
        _ => None,
    }
}

fn was_shift_key_pressed(key: &str) -> bool {
    key.len() == 1 && key.chars().next().unwrap().is_uppercase()
}

pub fn draw_app(app: Rc<RefCell<CSApp>>, mut async_executor: AsyncExecutor) {
    yew::initialize();

    let hoohaw: Rc<RefCell<Vec<Box<Fn()>>>> = Rc::new(RefCell::new(vec![]));

    // dirty hacks to focus something
    js! {
        var CS__PREVIOUS_FOCUSABLE_THAT_HAD_FOCUS = null;

        var findClosestFocusable = function(el) {
            return el.closest("[tabindex='0']");
        };

        var callback = function() {
            var focusedId = document.body.getAttribute("data-focused-id");
            if (focusedId && focusedId > 0) {
                var el = document.getElementById(focusedId);
                if (el) {
                   //console.log("focusing: " + el.id);
                   let closestFocusable = findClosestFocusable(el);
                   if (closestFocusable) {
                       CS__PREVIOUS_FOCUSABLE_THAT_HAD_FOCUS = closestFocusable;
                   }
                   el.focus();
                }
            } else if (CS__PREVIOUS_FOCUSABLE_THAT_HAD_FOCUS) {
                CS__PREVIOUS_FOCUSABLE_THAT_HAD_FOCUS.focus();
            }
        };
        var observer = new MutationObserver(callback);
        var config = {childList: true, subtree: true};
        observer.observe(window.document.documentElement, config);

        // if the user focuses an element, then let's mark that as the currently focused
        // element
        document.addEventListener("focusin", function(e) {
            document.body.setAttribute("data-focused-id", e.target.id);
        });
    }

    let yew_app = App::<Model>::new().mount_to_body();
    let renderer_state = Rc::new(RefCell::new(RendererState::new(yew_app, hoohaw)));

    let run_after_render = {
        let renderer_state = Rc::clone(&renderer_state);
        move || {
            renderer_state.borrow().run_all_after_render();
        }
    };
    js! {
        var observer = new MutationObserver(function() {
            @{run_after_render}();
        });
        var config = {childList: true, subtree: true};
        observer.observe(window.document.documentElement, config);

    }

    setup_ui_update_on_io_event_completion(&mut async_executor, Rc::clone(&renderer_state));
    add_global_keydown_event_listener(Rc::clone(&renderer_state));
    renderer_state.borrow().send_msg(Msg::Init(Rc::clone(&app),
                                               async_executor.clone(),
                                               Rc::clone(&renderer_state)));
    yew::run_loop();
}

fn setup_ui_update_on_io_event_completion(async_executor: &mut AsyncExecutor,
                                          renderer_state: Rc<RefCell<RendererState>>) {
    async_executor.setonupdate(Rc::new(move || {
                                   renderer_state.borrow().send_msg(Msg::Redraw);
                               }));
}

fn add_global_keydown_event_listener(renderer_state: Rc<RefCell<RendererState>>) {
    document().add_event_listener(move |e: KeyDownEvent| {
                  renderer_state.borrow().handle_global_key(&e);
              });
}

fn symbolize_text(text: &str) -> Html<Model> {
    html! {
        <span>
            { for text.chars().map(|char| {
                if is_in_symbol_range(char) {
                    html! {
                        <span style="display: inline-block; font-size: 57%; transform: translateY(-1px);",>
                          { char }
                        </span>
                    }
                } else {
                    html! {
                        <span>{ char }</span>
                    }
                }
            })}
        </span>
    }
}

fn is_in_symbol_range(c: char) -> bool {
    match c as u32 {
        0xf000...0xf72f => true,
        _ => false,
    }
}
