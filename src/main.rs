#![feature(unboxed_closures)]
#![feature(specialization)]
#![feature(nll)]

extern crate glium;

#[macro_use]
extern crate imgui;

extern crate imgui_sys;

#[macro_use]
extern crate objekt;

extern crate imgui_glium_renderer;

use imgui::*;
use std::cell::RefCell;

mod support;
mod lang;
mod env;

use self::env::{ExecutionEnvironment};
use self::lang::{Value,CodeNode,Function,FunctionCall,StringLiteral};

const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const BLUE_COLOR: [f32; 4] = [0.196, 0.584, 0.721, 1.0];
const GREY_COLOR: [f32; 4] = [0.521, 0.521, 0.521, 1.0];
const CLEAR_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
const BUTTON_SIZE: (f32, f32) = (0.0, 0.0);

fn main() {
    let app = App::new();

    support::run("cs".to_owned(), CLEAR_COLOR, |ui| {
        app.draw(ui);
        true
    });
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

struct Controller {
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

struct App {
    loaded_code: CodeNode,
    controller: Controller,
}

trait UiToolkit {
    fn draw_window(&self, window_name: &str, f: &Fn());
    fn draw_empty_line(&self);
    fn draw_button(&self, label: &str, color: [f32; 4], f: &Fn());
    fn draw_text_box(&self, text: &str);
    fn draw_next_on_same_line(&self);
}

struct ImguiToolkit<'a> {
    ui: &'a Ui<'a>,
}

impl<'a> ImguiToolkit<'a> {
    fn new(ui: &'a Ui) -> ImguiToolkit<'a> {
        ImguiToolkit { ui: ui }
    }
}

impl<'a> UiToolkit for ImguiToolkit<'a> {
    fn draw_window(&self, window_name: &str, f: &Fn()) {
        self.ui.window(im_str!("{}", window_name))
            .size((300.0, 100.0), ImGuiCond::FirstUseEver)
            .build(f)
    }

    fn draw_empty_line(&self) {
        self.ui.new_line();
    }

    fn draw_button(&self, label: &str, color: [f32; 4], f: &Fn()) {
        self.ui.with_color_var(ImGuiCol::Button, color, || {
            if self.ui.button(im_str!("{}", label), BUTTON_SIZE) {
                f()
            }
        });
    }

    fn draw_next_on_same_line(&self) {
        self.ui.same_line_spacing(0.0, 1.0);
    }

    fn draw_text_box(&self, text: &str) {
        self.ui.text(text);
        // text box is always scrolled to the bottom
        unsafe { imgui_sys::igSetScrollHere(1.0) };
    }
}

struct AppRenderer<'a> {
    ui_toolkit: RefCell<Box<UiToolkit + 'a>>,
    controller: &'a Controller,
}

impl<'a> AppRenderer<'a> {
    fn render_console_window(&self) {
        self.ui_toolkit.borrow().draw_window("Console", &|| {
            self.ui_toolkit.borrow().draw_text_box(&self.controller.read_console());
        })
    }

    fn render_code_window(&self, code_node: &CodeNode) {
        self.ui_toolkit.borrow().draw_window(&code_node.description(), &|| {
            self.render_code(code_node);
            self.ui_toolkit.borrow().draw_empty_line();
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
        self.ui_toolkit.borrow().draw_button(function_call.function.name(), BLUE_COLOR, &|| {});
        for code_node in &function_call.args {
            self.ui_toolkit.borrow().draw_next_on_same_line();
            self.render_code(code_node)
        }
    }

    fn render_string_literal(&self, string_literal: &StringLiteral) {
        self.ui_toolkit.borrow().draw_button(&string_literal.value, CLEAR_BACKGROUND_COLOR, &|| {});
    }

    fn render_run_button(&self, code_node: &CodeNode) {
        self.ui_toolkit.borrow().draw_button("Run", GREY_COLOR, &||{
            self.controller.run(code_node);
        })
    }
}

impl App {
    fn new() -> App {
        // code
        let mut args: Vec<CodeNode> = Vec::new();
        let string_literal = StringLiteral { value: "Hello World".to_string()};
        args.push(CodeNode::StringLiteral(string_literal));
        let function_call = FunctionCall{function: Box::new(Print {}), args: args};
        let print_hello_world: CodeNode = CodeNode::FunctionCall(function_call);

        App {
            loaded_code: print_hello_world,
            controller: Controller::new()
        }
    }

    fn draw<'ui>(&self,ui: &'ui Ui) -> bool {
        let toolkit = ImguiToolkit::new(ui);
        let app_renderer = AppRenderer {
            ui_toolkit: RefCell::new(Box::new(toolkit)),
            controller: &self.controller
        };

        app_renderer.render_code_window(&self.loaded_code);
        app_renderer.render_console_window();
        true
    }
}
