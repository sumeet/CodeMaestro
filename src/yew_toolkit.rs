use super::{CSApp, UiToolkit};
use yew::prelude::*;
use std::cell::RefCell;

pub struct Model<'a> {
    app: Option<&'a CSApp>,
}

pub enum Msg<'a> {
    SetApp(&'a CSApp)
}

impl Component for Model<'static> {
    type Message = Msg<'static>;
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

struct YewToolkit<'a> {
    phantom_str: &'a str,
    html_nodes: RefCell<Vec<Html<Model<'static>>>>
}

impl<'a> UiToolkit for YewToolkit<'a> {
    fn draw_window(&self, window_name: &str, f: &Fn()) {
        let node = html! {
            <p> { window_name }</p>
        };
        {
            let mut nodes = self.html_nodes.borrow_mut();
            nodes.push(node);
        }
    }

    fn draw_empty_line(&self) {
    }

    fn draw_button(&self, label: &str, color: [f32; 4], f: &Fn()) {
    }

    fn draw_text_box(&self, text: &str) {
    }

    fn draw_next_on_same_line(&self) {
    }
}

impl<'a> YewToolkit<'a> {
    fn new() -> Self {
        YewToolkit {
            phantom_str: "jiio",
            html_nodes: RefCell::new(Vec::new())
        }
    }

    fn html(&'a self) -> Html<Model<'static>> {
        let nodes = self.html_nodes.replace(Vec::new());
        html! {
            { for nodes.into_iter() }
        }
    }
}


impl Renderable<Model<'static>> for Model<'static> {
    fn view(&self) -> Html<Self> {
       if let(Some(app)) = self.app {
           let mut tk = YewToolkit::new();
           app.draw(&mut tk);
           tk.html()
       } else {
         html! { <p> {"No app"} </p> }
       }
    }
}

pub fn draw_app(app: &CSApp) {
    App::<Model>::new().mount_to_body()
        .send_message(Msg::SetApp(app));
    yew::initialize();
    yew::run_loop()
}