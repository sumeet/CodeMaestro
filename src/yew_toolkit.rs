use super::{CSApp, UiToolkit,UiToolkit2,Key as AppKey};
use yew::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::slice::SliceConcatExt;
use stdweb::Value;

pub struct Model {
    app: Option<Rc<CSApp>>,
    link: Rc<RefCell<ComponentLink<Model>>>,
    keyboard_input_service: KeyboardInputService,
}

pub enum Msg {
    SetApp(Rc<CSApp>),
    SetKeypressHandler(Rc<Fn(AppKey)>),
    Redraw,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, link: ComponentLink<Self>) -> Self {
        Model {
            app: None,
            link: Rc::new(RefCell::new(link)),
            keyboard_input_service: KeyboardInputService::new(),
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::SetApp(app) => {
                self.app = Some(app);
            }
            Msg::SetKeypressHandler(keypress_handler) => {
                let app = self.app.as_ref();
                if app.is_none() {
                    return true;
                }
                let app2 = Rc::clone(app.unwrap());
                let link2 = Rc::clone(&self.link);
                let callback = move |keystring : String| {
                    let key = map_key(&keystring);
                    if let(Some(key)) = key {
                        keypress_handler(key)
                    }
                    let cb = link2.borrow_mut().send_back(|_: ()| {Msg::Redraw});
                    cb.emit(());
                };
                self.keyboard_input_service.register(Callback::from(callback));
            }
            Msg::Redraw =>   {
            }
        }
        true
    }
}

const WINDOW_BG_COLOR: [f32; 4] = [0.090, 0.090, 0.090, 0.75];
const WINDOW_TITLE_BG_COLOR: [f32; 4] = [0.408, 0.408, 0.678, 1.0];

struct YewToolkit {
    current_window: RefCell<Vec<Html<Model>>>,
    windows: RefCell<Vec<Html<Model>>>,
    draw_next_on_same_line_was_set: RefCell<bool>,
    last_drawn_element_id: RefCell<u32>,
    javascript_to_run_after_render: RefCell<Vec<String>>,
}

impl UiToolkit2 for YewToolkit {
    type DrawResult = Html<Model>;

    fn draw_all(&self, draw_results: Vec<Self::DrawResult>) -> Self::DrawResult {
        html! {
            <div>
                { for draw_results.into_iter() }
                { for self.after_render_javascripts().into_iter().map(|js| html! {
                    <script>{ js }</script>
                })}
            </div>
        }
    }

    fn draw_window(&self, window_name: &str, f: &Fn() -> Self::DrawResult) -> Self::DrawResult {
        html! {
            <div style={ format!("background-color: {}", self.rgba(WINDOW_BG_COLOR)) },
                id={ self.incr_last_drawn_element_id().to_string() }, >
                <h4 style={ format!("background-color: {}; color: white", self.rgba(WINDOW_TITLE_BG_COLOR)) },>{ window_name }</h4>
                { f() }
            </div>
        }
    }

    fn draw_layout_with_bottom_bar(&self, draw_content_fn: &Fn() -> Self::DrawResult, draw_bottom_bar_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult {
        // JANK, make it better
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

    fn draw_button<F: Fn() + 'static>(&self, label: &str, color: [f32; 4], on_button_press_callback: F) -> Self::DrawResult {
        html! {
            <button style={"display: block;"}, id={ self.incr_last_drawn_element_id().to_string() },
                 style=format!("color: white; background-color: {};", self.rgba(color)),
                 onclick=|_| { on_button_press_callback(); Msg::Redraw }, >
            { label }
            </button>
        }
    }

    fn draw_text_box(&self, text: &str) -> Self::DrawResult {
        html! {
            <textarea id={ self.incr_last_drawn_element_id().to_string() },>{ text }</textarea>
        }
    }

    fn draw_all_on_same_line(&self, draw_fns: Vec<&Fn() -> Self::DrawResult>) -> Self::DrawResult {
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

    fn draw_text_input<F: Fn(&str) -> () + 'static, D: Fn() + 'static>(&self, existing_value: &str, onchange: F, ondone: D) -> Self::DrawResult {
        let ondone = Rc::new(ondone);
        let ondone2 = Rc::clone(&ondone);
        html! {
            <input type="text",
               id={ self.incr_last_drawn_element_id().to_string() },
               value=existing_value,
               oninput=|e| {onchange(&e.value) ; Msg::Redraw},
               onkeypress=|e| { if e.key() == "Enter" { ondone() } ; Msg::Redraw },
               onblur=|_| {ondone2(); Msg::Redraw}, />
        }
    }

    fn draw_border_around(&self, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult {
        let mut html = draw_fn();
        match html {
            yew::virtual_dom::VNode::VTag(mut vtag) => {
                let default = &"".to_string();
                let style = vtag.attributes.get(&"style".to_string()).unwrap_or(default);
                vtag.add_attribute(
                    &"style".to_string(),
                    &format!("{}; border: 1px solid black;", style)
                );
                yew::virtual_dom::VNode::VTag(vtag)
            }
            _ => html
        }
    }

    fn focused(&self, draw_fn: &Fn() -> Html<Model>) -> Self::DrawResult {
        let html = draw_fn();
        self.focus_last_drawn_element();
        html
    }
}

impl YewToolkit {
    fn new(on_key_press: Rc<Fn(String)>) -> Self {
        YewToolkit {
            current_window: RefCell::new(Vec::new()),
            windows: RefCell::new(Vec::new()),
            draw_next_on_same_line_was_set: RefCell::new(false),
            last_drawn_element_id: RefCell::new(0),
            javascript_to_run_after_render: RefCell::new(Vec::new()),
        }
    }

    fn focus_last_drawn_element(&self) {
        let mut javascripts = self.javascript_to_run_after_render.borrow_mut();
        javascripts.push(format!("document.getElementById({}).focus();", self.get_last_drawn_element_id()))
    }

    fn after_render_javascripts(&self) -> Vec<String> {
        self.javascript_to_run_after_render.borrow().clone()
    }

    fn rgba(&self, color: [f32; 4]) -> String {
       format!("rgba({}, {}, {}, {})", color[0]*255.0, color[1]*255.0, color[2]*255.0, color[3])
    }

    fn incr_last_drawn_element_id(&self) -> u32 {
        let next_id = *self.last_drawn_element_id.borrow() + 1;
        self.last_drawn_element_id.replace(next_id);
        next_id
    }

    fn get_last_drawn_element_id(&self) -> u32 {
        *self.last_drawn_element_id.borrow()
    }
}


impl Renderable<Model> for Model {
    fn view(&self) -> Html<Self> {
        if let(Some(ref app)) = self.app {
            let app2 = Rc::clone(&app);
            let mut tk = YewToolkit::new(Rc::new(move |_| {}));
            app.draw(&mut tk)
        } else {
            html! { <p> {"No app"} </p> }
        }
    }
}

fn map_key(key: &str) -> Option<AppKey> {
    match key.as_ref() {
        "a" => Some(AppKey::A),
        "b" => Some(AppKey::B),
        "c" => Some(AppKey::C),
        "d" => Some(AppKey::D),
        "w" => Some(AppKey::W),
        "x" => Some(AppKey::X),
        "r" => Some(AppKey::R),
        _ => None
    }
}

pub fn draw_app(app: Rc<CSApp>) {
    yew::initialize();
    let mut yew_app = App::<Model>::new().mount_to_body();
    yew_app.send_message(Msg::SetApp(Rc::clone(&app)));
    let app2 = Rc::clone(&app);
    yew_app.send_message(Msg::SetKeypressHandler(Rc::new(move |key| {
        app2.controller.borrow_mut().handle_key_press(key)
    })));
    yew::run_loop()
}


// copied from https://github.com/DenisKolodin/yew/issues/333#issuecomment-407585000
//
// the example there uses a Drop to undo the binding, but i don't think we'll need that.
// pretty sure we'll be fine here if we just register once
pub struct KeyboardInputService {}

pub struct KeyboardInputTask(Option<Value>);

impl KeyboardInputService {
    pub fn new() -> Self {
        Self {}
    }

    pub fn register(&mut self, callback: Callback<(String)>) -> KeyboardInputTask {
        let callback = move |key| {
            callback.emit(key);
        };
        let listener = js! {
            console.log("registering callback");
            var callback = @{callback};
            var listener = function(e) {
                callback(e.key || "NOT FOUND");
            };
            window.addEventListener("keyup", listener);
            return listener;
        };
        return KeyboardInputTask(Some(listener))
    }
}

