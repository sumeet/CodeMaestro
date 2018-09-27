#![feature(pattern_parentheses)]
#![feature(unboxed_closures)]
#![feature(specialization)]
#![feature(nll)]

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

mod lang;
mod env;

use self::env::{ExecutionEnvironment};
use self::lang::{Value,CodeNode,Function,FunctionCall,StringLiteral};

const BLUE_COLOR: [f32; 4] = [0.196, 0.584, 0.721, 1.0];
const GREY_COLOR: [f32; 4] = [0.521, 0.521, 0.521, 1.0];
const CLEAR_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

#[cfg(feature = "default")]
pub fn draw_app(app: CSApp) {
    imgui_toolkit::draw_app(app)
}

#[cfg(feature = "javascript")]
pub fn draw_app(app: CSApp) {
    yew_toolkit::draw_app(app);
}


fn main() {
    let app = CSApp::new();
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
    execution_environment: RefCell<ExecutionEnvironment>
}

impl Controller {
    fn new() -> Controller {
        Controller {
            execution_environment: RefCell::new(ExecutionEnvironment::new())
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
}

pub struct CSApp {
    pub loaded_code: CodeNode,
    pub controller: Controller,
}

trait UiToolkit {
    fn draw_window(&self, window_name: &str, f: &Fn());
    fn draw_empty_line(&self);
    fn draw_button(&self, label: &str, color: [f32; 4], f: &Fn());
    fn draw_text_box(&self, text: &str);
    fn draw_next_on_same_line(&self);
}

struct AppRenderer<'a, T> {
    ui_toolkit: &'a mut T,
    controller: &'a Controller,
}

impl<'a, T: UiToolkit> AppRenderer<'a, T> {
    fn render_console_window(&self) {
        self.ui_toolkit.draw_window("Console", &|| {
            self.ui_toolkit.draw_text_box(&self.controller.read_console());
        })
    }

    fn render_code_window(&self, code_node: &CodeNode) {
        self.ui_toolkit.draw_window(&code_node.description(), &|| {
            self.render_code(code_node);
            self.ui_toolkit.draw_empty_line();
            self.render_run_button(code_node);
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
        self.ui_toolkit.draw_button(&string_literal.value, CLEAR_BACKGROUND_COLOR, &|| {});
    }

    fn render_run_button(&self, code_node: &CodeNode) {
        self.ui_toolkit.draw_button("Run", GREY_COLOR, &||{
            self.controller.run(code_node);
        })
    }
}

impl CSApp {
    fn new() -> CSApp {
        // code
        let mut args: Vec<CodeNode> = Vec::new();
        let string_literal = StringLiteral { value: "HW".to_string()};
        args.push(CodeNode::StringLiteral(string_literal));
        let function_call = FunctionCall{function: Box::new(Print {}), args: args};
        let print_hello_world: CodeNode = CodeNode::FunctionCall(function_call);

        CSApp {
            loaded_code: print_hello_world,
            controller: Controller::new()
        }
    }

    fn draw<T: UiToolkit>(&self, ui_toolkit: &mut T) {
        let app_renderer = AppRenderer {
            ui_toolkit: ui_toolkit,
            controller: &self.controller
        };

        app_renderer.render_code_window(&self.loaded_code);
        app_renderer.render_console_window();
    }
}
