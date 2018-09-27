use super::{CSApp, UiToolkit};
use yew::prelude::*;
use std::cell::RefCell;

pub struct Model {
    app: Option<CSApp>,
}

pub enum Msg {
    SetApp(CSApp)
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, _: ComponentLink<Self>) -> Self {
        Model { app: None }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::SetApp(app) => self.app = Some(app)
        }
        true
    }
}

struct YewToolkit {
    current_window: RefCell<Vec<Html<Model>>>,
    windows: RefCell<Vec<Html<Model>>>
}

impl UiToolkit for YewToolkit {
    fn draw_window(&self, window_name: &str, f: &Fn()) {
        f();
        let inner = self.gather_html_for_window();

        self.add_window(html! {
            <div>
                <h3>{ window_name }</h3>
                { inner }
            </div>
        });
    }

    fn draw_empty_line(&self) {
        self.push_html_into_current_window(html!{ <br />})
    }

    fn draw_button(&self, label: &str, color: [f32; 4], f: &Fn()) {
        self.push_html_into_current_window(html! {
            <button>{ label }</button>
        })
    }

    fn draw_text_box(&self, text: &str) {
        self.push_html_into_current_window(html! {
            <textarea>{ text }</textarea>
        });
    }

    fn draw_next_on_same_line(&self) {
    }
}

impl YewToolkit {
    fn new() -> Self {
        YewToolkit {
            current_window: RefCell::new(Vec::new()),
            windows: RefCell::new(Vec::new()),
        }
    }

    fn push_html_into_current_window(&self, node: Html<Model>) {
        let mut nodes = self.current_window.borrow_mut();
        nodes.push(node);
    }

    fn add_window(&self, node: Html<Model>) {
        let mut nodes = self.windows.borrow_mut();
        nodes.push(node);
    }

    // XXX: mutates YewToolkit.html_nodes, gathering up any rendering that's been already done
    fn gather_html_for_window(&self) -> Html<Model> {
        let nodes = self.current_window.replace(Vec::new());
        html! {
            { for nodes.into_iter() }
        }
    }

    fn windows(&self) -> Html<Model> {
        let nodes = self.windows.replace(Vec::new());
        html! {
            { for nodes.into_iter() }
        }
    }
}


impl Renderable<Model> for Model {
    fn view(&self) -> Html<Self> {
       if let(Some(app)) = &self.app {
           let mut tk = YewToolkit::new();
           app.draw(&mut tk);
           tk.windows()
       } else {
         html! { <p> {"No app"} </p> }
       }
    }
}

pub fn draw_app(app: CSApp) {
    yew::initialize();
    let msg = Msg::SetApp(app);
    App::<Model>::new().mount_to_body()
        .send_message(msg);
    yew::run_loop()
}