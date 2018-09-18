#![feature(unboxed_closures)]
#![feature(specialization)]

#[macro_use]
extern crate lazy_static;

extern crate glium;

#[macro_use]
extern crate imgui;

#[macro_use]
extern crate objekt;

extern crate imgui_glium_renderer;

extern crate pyo3;

#[macro_use(defer)] extern crate scopeguard;

use scopeguard::guard;

use pyo3::prelude::*;

use imgui::*;

mod support;

const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const BLUE_COLOR: [f32; 4] = [0.196, 0.584, 0.721, 1.0];
const GREY_COLOR: [f32; 4] = [0.521, 0.521, 0.521, 1.0];
const CLEAR_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const BUTTON_SIZE: (f32, f32) = (0.0, 0.0);

fn main() {
    support::run("cs".to_owned(), CLEAR_COLOR, hello_world);
}

trait Function {
    fn call(&self, env: &mut ExecutionEnvironment, args: Vec<Value>) -> Value;
    fn name(&self) -> &str;
}

struct Print {}

impl Function for Print {
    fn call(&self, env: &mut ExecutionEnvironment, args: Vec<Value>) -> Value {
        match args.as_slice() {
            [Value::String(string)] =>  {
                println!("trying to print {}", string);
                env.println(string);
                println!("console is now {}", env.console)
            }
            _ => panic!("wrong arguments")
        }
        Value::Null
    }

    fn name(&self) -> &str {
        "Print"
    }
}

enum CodeNode<'a> {
    FunctionCall(&'a FunctionCall<'a>),
    StringLiteral(&'a StringLiteral),
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

impl <'a> CodeNode<'a> {
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

struct StringLiteral {
    value: String
}

struct FunctionCall<'a> {
    function: Box<Function>,
    args: Vec<CodeNode<'a>>,
}

struct Controller {
    execution_environment: ExecutionEnvironment
}

impl Controller {
    fn new() -> Controller {
        Controller {
            execution_environment: ExecutionEnvironment::new()
        }
    }

    fn run(&mut self, code_node: &CodeNode) {
        println!("evaluating a codenode");
        code_node.evaluate(&mut self.execution_environment);
    }
}


struct ImguiRenderer<'a> {
    ui: &'a Ui<'a>,
    controller: Controller,
}

impl<'a> ImguiRenderer<'a> {
    fn render(&mut self, code_node: &CodeNode) {
        match code_node {
            CodeNode::FunctionCall(function_call) => {
                self.render_function_call(&function_call);
            }
            CodeNode::StringLiteral(string_literal) => {
                self.render_string_literal(&string_literal);
            }
        }
    }

    fn render_function_call(&mut self, function_call: &FunctionCall) {
        let button_text = im_str!("{}", function_call.function.name());
        self.draw_button(button_text, BLUE_COLOR, |s|{});
        for code_node in &function_call.args {
            self.draw_next_on_the_same_line();
            self.render(code_node)
        }
        self.ui.new_line();
        self.draw_button(im_str!("Run"), GREY_COLOR, |s| {
            println!("button has been hit!!!!");
            s.controller.run(&CodeNode::FunctionCall(function_call));
        })
    }

    fn render_string_literal(&mut self, string_literal: &StringLiteral) {
        self.draw_button(im_str!("{}", string_literal.value), CLEAR_BACKGROUND_COLOR, |s|{});
    }

    fn draw_next_on_the_same_line(&self) {
        self.ui.same_line_spacing(0.0, 1.0);
    }

    fn draw_button<F>(&mut self, button_text: &ImStr, color: [f32; 4], mut func: F)
        where F: FnMut(&mut Self)
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
    //loaded_code: CodeNode<'a>,
    controller: Controller,
}

impl App {
    fn new() -> App {
        // ee
        let env = ExecutionEnvironment::new();

        // code
//        let mut args: Vec<CodeNode> = Vec::new();
//        let string_literal = StringLiteral { value: "Hello World".to_string()};
//        args.push(CodeNode::StringLiteral(&string_literal));
//        let function_call = FunctionCall{function: Box::new(Print {}), args: args};
//        let print_hello_world: CodeNode = CodeNode::FunctionCall(&function_call);

        // renderer
        //let imgui_code_renderer = ImguiRenderer { ui: ui, controller: Controller::new() };

        App {
            env: env,
            //loaded_code: print_hello_world,
            controller: Controller::new()
        }
    }

    fn loaded_code(&self) -> CodeNode {
        let mut args: Vec<CodeNode> = Vec::new();
        let string_literal = StringLiteral { value: "Hello World".to_string()};
        let string_literal_code_node = CodeNode::StringLiteral(&string_literal);
        args.push(string_literal_code_node);
        let function_call = FunctionCall{function: Box::new(Print {}), args: args};
        CodeNode::FunctionCall(&function_call)
    }

    fn draw(&mut self, ui: &Ui) -> bool {
        let mut code_renderer =  ImguiRenderer{ ui: ui, controller: self.controller };

        ui.window(im_str!("hw"))
            .size((300.0, 100.0), ImGuiCond::FirstUseEver)
            .build(|| {
                code_renderer.render(&self.loaded_code());
            });
        ui.window(im_str!("output"))
            .size((300.0, 100.0), ImGuiCond::FirstUseEver)
            .build(|| {
                ui.text(self.env.console);
            });
        true
    }
}

fn hello_world<'a>(ui: &Ui<'a>) -> bool {
    true
}
