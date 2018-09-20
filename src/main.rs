#![feature(unboxed_closures)]
#![feature(specialization)]

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

const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const BLUE_COLOR: [f32; 4] = [0.196, 0.584, 0.721, 1.0];
const GREY_COLOR: [f32; 4] = [0.521, 0.521, 0.521, 1.0];
const CLEAR_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
const BUTTON_SIZE: (f32, f32) = (0.0, 0.0);

fn main() {
//    let controller = Controller::new();
    let app = App::new();

    support::run("cs".to_owned(), CLEAR_COLOR, |ui| {
        app.draw(ui);
        true
    });
}

trait Function: objekt::Clone {
    fn call(&self, env: &mut ExecutionEnvironment, args: Vec<Value>) -> Value;
    fn name(&self) -> &str;
}

clone_trait_object!(Function);

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

#[derive(Clone)]
enum CodeNode {
    FunctionCall(FunctionCall),
    StringLiteral(StringLiteral),
}

enum Value {
    Null,
    String(String),
}

struct ExecutionEnvironment {
    console: String,
}

impl ExecutionEnvironment {
    fn new() -> ExecutionEnvironment {
        return ExecutionEnvironment {
            console: String::new(),
        }
    }
    fn println(&mut self, ln: &str) {
        self.console.push_str(ln);
        self.console.push_str("\n")
    }
}

impl CodeNode {
    fn evaluate(&self, env: &mut ExecutionEnvironment) -> Value {
        match self {
            CodeNode::FunctionCall(function_call) => {
                let args: Vec<Value> = function_call.args.iter().map(|arg| arg.evaluate(env)).collect();
                function_call.function.call(env, args)
            }
            CodeNode::StringLiteral(string_literal) => {
                // xxx: can i get rid of this clone?
                Value::String(string_literal.value.clone())
            }
        }
    }
}

#[derive(Clone)]
struct StringLiteral {
    value: String
}

#[derive(Clone)]
struct FunctionCall {
    function: Box<Function>,
    args: Vec<CodeNode>,
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
}


struct ImguiRenderer<'a> {
    ui: &'a Ui<'a>,
    controller: &'a Controller,
}

impl<'a> ImguiRenderer<'a> {
    fn render(&self, code_node: &CodeNode) {
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
        let button_text = im_str!("{}", function_call.function.name());
        self.draw_button(button_text, BLUE_COLOR, |_s|{});
        for code_node in &function_call.args {
            self.draw_next_on_the_same_line();
            self.render(code_node)
        }
        self.ui.new_line();
        let code_node = CodeNode::FunctionCall(function_call.clone());
        self.draw_button(im_str!("Run"), GREY_COLOR, |s| {
            s.controller.run(&code_node);
        })
    }

    fn render_string_literal(&self, string_literal: &StringLiteral) {
        self.draw_button(
            im_str!("{}", string_literal.value),
           CLEAR_BACKGROUND_COLOR,
           |_s|{});
    }

    fn draw_next_on_the_same_line(&self) {
        self.ui.same_line_spacing(0.0, 1.0);
    }

    fn draw_button<F>(&self, button_text: &ImStr, color: [f32; 4], func: F)
        where F: Fn(&Self)
    {
        self.ui.with_color_var(ImGuiCol::Button, color, || {
            if self.ui.button(button_text, BUTTON_SIZE) {
                func(self)
            }
        });
    }
}

struct App {
    env: ExecutionEnvironment,
    loaded_code: CodeNode,
    controller: Controller,
}

trait UiToolkit {
    fn draw_window(&self, window_name: &str, f: &Fn());
    fn draw_empty_line(&self);
    fn draw_button(&self, text: &str, color: [f32; 4], f: &Fn());
    fn draw_next_on_same_line(&mut self);
}

struct AppRenderer<'a> {
    ui_toolkit: RefCell<Box<UiToolkit>>,
    controller: &'a Controller,
}

impl<'a> AppRenderer<'a> {
    fn render_code_window(&self, code_node: &CodeNode) {
        self.ui_toolkit.borrow().draw_window("replace with code node name", &|| {
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
            self.ui_toolkit.borrow_mut().draw_next_on_same_line();
            self.render_code(code_node)
        }
    }

    fn render_string_literal(&self, string_literal: &StringLiteral) {
        self.ui_toolkit.borrow().draw_button(&string_literal.value, CLEAR_BACKGROUND_COLOR, &|| {});
    }

    fn render_run_button(&self, code_node: &CodeNode) {
        self.ui_toolkit.borrow().draw_button("Run", GREY_COLOR, &||{ println!("Run!") })
    }
}

impl App {
    fn new() -> App {
        // ee
        let env = ExecutionEnvironment::new();

        // code
        let mut args: Vec<CodeNode> = Vec::new();
        let string_literal = StringLiteral { value: "Hello World".to_string()};
        args.push(CodeNode::StringLiteral(string_literal));
        let function_call = FunctionCall{function: Box::new(Print {}), args: args};
        let print_hello_world: CodeNode = CodeNode::FunctionCall(function_call);

        App {
            env: env,
            loaded_code: print_hello_world,
            controller: Controller::new()
        }
    }

    fn draw_generic(&self, renderer: &AppRenderer) {
        renderer.render_code_window(&self.loaded_code);
    }

    fn draw(&self, ui: &Ui) -> bool {
        let code_renderer =  ImguiRenderer{ ui: ui, controller: &self.controller };

        ui.window(im_str!("hw"))
            .size((300.0, 100.0), ImGuiCond::FirstUseEver)
            .build(|| {
                code_renderer.render(&self.loaded_code);
            });
        ui.window(im_str!("output"))
            .size((300.0, 100.0), ImGuiCond::FirstUseEver)
            .build(|| {
                ui.text(&self.env.console);
                unsafe { imgui_sys::igSetScrollHere(1.0) };
            });
        true
    }
}
