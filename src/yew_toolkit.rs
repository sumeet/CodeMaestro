use super::{CSApp, UiToolkit};
use yew::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct Model {
    app: Option<Rc<CSApp>>,
}

pub enum Msg {
    SetApp(Rc<CSApp>),
    Redraw,
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_: Self::Properties, _: ComponentLink<Self>) -> Self {
        Model { app: None }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::SetApp(app) => self.app = Some(app),
            Msg::Redraw =>  (),
        }
        true
    }
}

struct YewToolkit {
    current_window: RefCell<Vec<Html<Model>>>,
    windows: RefCell<Vec<Html<Model>>>,
    draw_next_on_same_line_was_set: RefCell<bool>,
}

// maybe make this mutable?
impl UiToolkit for YewToolkit {
    fn draw_window(&self, window_name: &str, draw_inside_window: &Fn()) {
        draw_inside_window();
        let window_contents = self.gather_html_for_window();

        self.add_window(html! {
            <div>
                <h3>{ window_name }</h3>
                { window_contents }
            </div>
        });
    }

    // BROKEN: currently it's a very tiny line
    fn draw_empty_line(&self) {
        self.push_html_into_current_window(html!{ <br />})
    }

    fn draw_button<F: Fn() + 'static>(&self, label: &str, color: [f32; 4], on_button_press_callback: F) {
        self.draw_newline_if_necessary();

        self.push_html_into_current_window(html! {
            <button
                 style=format!("background-color: rgba({}, {}, {}, {});", color[0]*255.0, color[1]*255.0, color[2]*255.0, color[3]*255.0),
                 onclick=|_| { on_button_press_callback(); Msg::Redraw }, >
            { label }
            </button>
        })
    }

    fn draw_text_box(&self, text: &str) {
        self.draw_newline_if_necessary();

        self.push_html_into_current_window(html! {
            <textarea>{ text }</textarea>
        });
    }

    fn draw_next_on_same_line(&self) {
        self.draw_next_on_same_line_was_set.replace(true);
    }

}

impl YewToolkit {
    fn new() -> Self {
        YewToolkit {
            current_window: RefCell::new(Vec::new()),
            windows: RefCell::new(Vec::new()),
            draw_next_on_same_line_was_set: RefCell::new(false),
        }
    }

    // BROKEN: sometimes it's too big
    fn draw_newline(&self) {
        self.push_html_into_current_window(html!{ <br />})
    }


    fn draw_newline_if_necessary(&self) {
        if !self.draw_next_on_same_line_was_set.replace(false) {
            self.draw_newline()
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

pub fn draw_app(app: Rc<CSApp>) {
    yew::initialize();
    let msg = Msg::SetApp(app.clone());
    App::<Model>::new().mount_to_body()
        .send_message(msg);
    yew::run_loop()
}
