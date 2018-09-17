#![feature(specialization)]

#[macro_use]
extern crate lazy_static;

extern crate glium;

#[macro_use]
extern crate imgui;

extern crate imgui_glium_renderer;

extern crate pyo3;

use pyo3::prelude::*;

use imgui::*;

mod support;

const CLEAR_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
const GREEN_COLOR: [f32; 4] = [0.215, 0.525, 0.407, 1.0];
const CLEAR_BACKGROUND_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
const BUTTON_SIZE: (f32, f32) = (0.0, 0.0);

fn main() {
    support::run("cs".to_owned(), CLEAR_COLOR, hello_world);
}

trait Function {
    fn call(&self, args: Vec<Value>) -> Value;
}

struct Print {}

impl Function for Print {
    fn call(&self, args: Vec<Value>) -> Value {
        match args.as_slice() {
            [Value::String(string)] => println!("{}", string),
            _ => println!("FUCK"),
        }
        Value::Null
    }
}

enum CodeNode {
    FunctionCall(FunctionCall),
    StringLiteral(StringLiteral),
}

enum Value {
    Null,
    String(String),
}

impl CodeNode {
    fn value(&self) -> Value {
        match self {
            CodeNode::FunctionCall(function_call) => {
                let args: Vec<Value> = function_call.args.iter().map(|arg| arg.value()).collect();
                function_call.function.call(args)
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

struct FunctionCall {
    function: Box<Function>,
    args: Vec<CodeNode>,
}


struct ImguiRenderer<'a> {
    ui: &'a Ui<'a>
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
        // XXX: the function name will go here
        self.draw_button(im_str!("Print"), GREEN_COLOR);
        for code_node in &function_call.args {
            self.draw_next_on_the_same_line();
            self.render(code_node)
        }
    }

    fn render_string_literal(&self, string_literal: &StringLiteral) {
        self.draw_button(im_str!("{}", string_literal.value), CLEAR_BACKGROUND_COLOR);
    }

    fn draw_next_on_the_same_line(&self) {
        self.ui.same_line_spacing(0.0, 1.0);
    }

    fn draw_button(&self, button_text: &ImStr, color: [f32; 4]) {
        self.ui.with_color_var(ImGuiCol::Button, color, || {
            self.ui.button(button_text, BUTTON_SIZE);
        });
    }
}


fn hello_world<'a>(ui: &Ui<'a>) -> bool {
    let mut args: Vec<CodeNode> = Vec::new();
    args.push(CodeNode::StringLiteral(StringLiteral { value: "Hello World".to_string()}));
    let print_hello_world: CodeNode = CodeNode::FunctionCall(
        FunctionCall{function: Box::new(Print {}), args: args});


    let imgui_code_renderer = ImguiRenderer { ui: ui };
    ui.window(im_str!("hw"))
        .size((300.0, 100.0), ImGuiCond::FirstUseEver)
        .build(|| {
            imgui_code_renderer.render(&print_hello_world);
        });
    true
}

//
//
//trait Function {
//    fn call(&self, Vec<PyObject>);
//}
//
//struct PythonCodeRenderer<'a> {
//    python: Python<'a>
//}
//
//impl PythonCodeRenderer {
//    pub fn new() -> PythonCodeRenderer {
//        let gil = Python::acquire_gil();
//        let py = gil.python();
//        PythonCodeRenderer{python: py};
//    }
//}
//
//impl RenderFunctionCall for PythonCodeRenderer {
//    fn render(&self, function_call: &FunctionCall) -> String {
//
//    }
//}
//
//
//impl ToPyObject for StringLiteral {
//    fn to_object(&self, py: &Python) -> PyObject {
//        return self.value.to_object(*py);
//    }
//}
//
//struct PythonFunction {
//    module: String,
//    function_name: String,
//}
//
