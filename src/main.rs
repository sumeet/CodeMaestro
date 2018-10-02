#![feature(pattern_parentheses)]
#![feature(unboxed_closures)]
#![feature(specialization)]
#![feature(nll)]
#![feature(arbitrary_self_types)]

#[cfg(feature = "default")]
extern crate glium;
#[cfg(feature = "default")]
#[macro_use]
extern crate imgui;
#[cfg(feature = "default")]
extern crate imgui_sys;
#[cfg(feature = "default")]
extern crate imgui_glium_renderer;
#[cfg(feature = "default")]
mod imgui_support;
#[cfg(feature = "default")]
mod imgui_toolkit;

#[cfg(feature = "javascript")]
#[macro_use]
extern crate yew;
#[cfg(feature = "javascript")]
mod yew_toolkit;

#[macro_use]
extern crate objekt;


use std::cell::RefCell;
use std::rc::Rc;

mod lang;
mod env;

use self::env::{ExecutionEnvironment};
use self::lang::{Value,CodeNode,Function,FunctionCall,StringLiteral,ID};

const BLUE_COLOR: [f32; 4] = [0.196, 0.584, 0.721, 1.0];
const GREY_COLOR: [f32; 4] = [0.521, 0.521, 0.521, 1.0];
const CLEAR_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

#[cfg(feature = "default")]
pub fn draw_app(app: Rc<CSApp>) {
    imgui_toolkit::draw_app(Rc::clone(&app));
}

#[cfg(feature = "javascript")]
pub fn draw_app(app: Rc<CSApp>) {
    yew_toolkit::draw_app(Rc::clone(&app));
}


fn main() {
    let app = Rc::new(CSApp::new());
    draw_app(app);
}

#[derive(Clone)]
struct Print {}

impl Function for Print {
    fn call(&self, env: &mut ExecutionEnvironment, args: Vec<Value>) -> Value {
        match args.as_slice() {
            [Value::String(string)] =>  {
                env.println(string);
            }
            _ => panic!("wrong arguments")
        }
        Value::Null
    }

    fn name(&self) -> &str {
        "Print"
    }
}

pub struct Controller {
    execution_environment: RefCell<ExecutionEnvironment>,
    selected_node_id: RefCell<Option<ID>>,
}

impl Controller {
    fn new() -> Controller {
        Controller {
            execution_environment: RefCell::new(ExecutionEnvironment::new()),
            selected_node_id: RefCell::new(None),
        }
    }

    fn run(&self, code_node: &CodeNode) {
        code_node.evaluate( &mut self.execution_environment.borrow_mut());
    }

    // idk why i have to clone this, can't i just give away a reference!?!?!?
    fn read_console(&self) -> String {
        let env = self.execution_environment.borrow();
        env.console.clone()
    }

    fn set_selected_node_id(&self, code_node_id: Option<ID>) {
        self.selected_node_id.replace(code_node_id);
    }

    fn get_selected_node_id(&self) -> Option<ID> {
        // TODO: not sure why we have to clone here
        self.selected_node_id.borrow().clone()
    }
}

pub struct CSApp {
    pub loaded_code: CodeNode,
    pub controller: Controller,
}

trait UiToolkit {
    fn draw_window(&self, window_name: &str, f: &Fn());
    fn draw_empty_line(&self);
    fn draw_button<F: Fn() + 'static>(&self, label: &str, color: [f32; 4], f: F);
    fn draw_text_box(&self, text: &str);
    fn draw_next_on_same_line(&self);
    fn draw_text_input<F: Fn(&str) -> () + 'static, D: Fn(&str) + 'static>(&self, existing_value: &str, onchange: F, ondone: D);
    fn focus_last_drawn_element(&self);
}

struct AppRenderer<'a, T> {
    ui_toolkit: &'a mut T,
    //controller: Rc<Controller>,
    // probably needs to be Rc<Refcell<>>
    app: Rc<CSApp>,
}

impl<'a, T: UiToolkit> AppRenderer<'a, T> {
    fn render_console_window(&self) {
        self.ui_toolkit.draw_window("Console", &|| {
            self.ui_toolkit.draw_text_box(&self.app.controller.read_console());
        })
    }

    fn render_code_window(&self, code_node: &CodeNode) {
        self.ui_toolkit.draw_window(&code_node.description(), &|| {
            self.render_code(code_node);
            self.ui_toolkit.draw_empty_line();
            self.render_run_button();
        })
    }

    fn render_code(&self, code_node: &CodeNode) {
        match code_node {
            CodeNode::FunctionCall(function_call) => {
                self.render_function_call(&function_call);
            }
            CodeNode::StringLiteral(string_literal) => {
                self.render_string_literal(&string_literal);
            }
        }
    }

    fn render_function_call(&self, function_call: &FunctionCall) {
        self.ui_toolkit.draw_button(function_call.function.name(), BLUE_COLOR, &|| {});
        for code_node in &function_call.args {
            self.ui_toolkit.draw_next_on_same_line();
            self.render_code(code_node)
        }
    }

    fn render_string_literal(&self, string_literal: &StringLiteral) {
        if Some(string_literal.id) != self.app.controller.get_selected_node_id() {
            self.render_string_literal_when_unselected_no_editing_intended(string_literal);
        } else {
            self.render_string_literal_inline_for_editing(string_literal)
        }
    }

    fn render_string_literal_when_unselected_no_editing_intended(&self, string_literal: &StringLiteral) {
        let app = self.app.clone();
        let id = string_literal.id;
        self.ui_toolkit.draw_button(&string_literal.value, CLEAR_BACKGROUND_COLOR, move || {
            app.controller.set_selected_node_id(Some(id))
        });
    }

    fn render_string_literal_inline_for_editing(&self, string_literal: &StringLiteral) {
        let app = self.app.clone();
        self.ui_toolkit.draw_text_input(&string_literal.value,
            |new_value| {
                println!("{:?}", new_value);
                app.controller.update()
            },
            move |_|{
                app.controller.set_selected_node_id(None)
            });
        self.ui_toolkit.focus_last_drawn_element();
    }

    fn render_run_button(&self) {
        let app = self.app.clone();
        self.ui_toolkit.draw_button("Run", GREY_COLOR, move ||{
            app.controller.run(&app.loaded_code);
        })
    }
}

impl CSApp {
    fn new() -> CSApp {
        // code
        let mut args: Vec<CodeNode> = Vec::new();
        let string_literal = StringLiteral { value: "HW".to_string(), id: 1};
        args.push(CodeNode::StringLiteral(string_literal));
        let function_call = FunctionCall{
            function: Box::new(Print {}),
            args: args,
            id: 2,
        };
        let print_hello_world: CodeNode = CodeNode::FunctionCall(function_call);

        CSApp {
            loaded_code: print_hello_world,
            controller: Controller::new()
        }
    }

    fn draw<T: UiToolkit>(self: &Rc<CSApp>, ui_toolkit: &mut T) {
        let app_renderer = AppRenderer {
            ui_toolkit: ui_toolkit,
            app: self.clone(),
        };

        app_renderer.render_code_window(&self.loaded_code);
        app_renderer.render_console_window();
    }
}
