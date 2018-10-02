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

extern crate debug_cell;
use debug_cell::RefCell;

//use std::cell::RefCell;
use std::rc::Rc;

mod lang;
mod env;

use self::env::{ExecutionEnvironment};
use self::lang::{Value,CodeNode,Function,FunctionCall,StringLiteral,ID,Error};

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
                Value::Null
            }
            _ => Value::Result(Result::Err(Error::ArgumentError))
        }
    }

    fn name(&self) -> &str {
        "Print"
    }
}

pub struct Controller {
    execution_environment: ExecutionEnvironment,
    selected_node_id: Option<ID>,
    loaded_code: Option<CodeNode>,
}

impl<'a> Controller {
    fn new() -> Controller {
        Controller {
            execution_environment: ExecutionEnvironment::new(),
            selected_node_id: None,
            loaded_code: None,
        }
    }

    fn load_code(&mut self, code_node: &CodeNode) {
        self.loaded_code = Some(code_node.clone())
    }

    // should run the loaded code node
    fn run(&mut self, code_node: &CodeNode) {
        code_node.evaluate( &mut self.execution_environment);
    }

    fn read_console(&self) -> &str {
        &self.execution_environment.console
    }

    fn set_selected_node_id(&mut self, code_node_id: Option<ID>) {
        self.selected_node_id = code_node_id;
    }

    fn get_selected_node_id(&self) -> &Option<ID> {
        &self.selected_node_id
    }
}

pub struct CSApp {
    pub loaded_code: CodeNode,
    pub controller: Rc<RefCell<Controller>>,
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
    controller: Rc<RefCell<Controller>>,
}

impl<'a, T: UiToolkit> AppRenderer<'a, T> {
    fn render_console_window(&self) {
        let controller = self.controller.clone();
        self.ui_toolkit.draw_window("Console", &|| {
            self.ui_toolkit.draw_text_box(controller.borrow().read_console());
        })
    }

    fn render_code_window(&self) {
        let loaded_code = self.controller.borrow().loaded_code.clone();
        match loaded_code {
            None => {},
            Some(ref code) => {
                self.ui_toolkit.draw_window(&code.description(), &|| {
                    self.render_code(code);
                    self.ui_toolkit.draw_empty_line();
                    self.render_run_button(code);
                })
            }
        }
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
        if Some(string_literal.id) != *self.controller.borrow().get_selected_node_id() {
            self.render_string_literal_when_unselected_no_editing_intended(string_literal);
        } else {
            self.render_string_literal_inline_for_editing(string_literal)
        }
    }

    fn render_string_literal_when_unselected_no_editing_intended(&self, string_literal: &StringLiteral) {
        let controller = self.controller.clone();
        let id = string_literal.id;
        self.ui_toolkit.draw_button(&string_literal.value, CLEAR_BACKGROUND_COLOR, move || {
            let mut controller = controller.borrow_mut();
            controller.set_selected_node_id(Some(id))
        });
    }

    fn render_string_literal_inline_for_editing(&self, string_literal: &StringLiteral) {
        let controller = self.controller.clone();
        let sl = string_literal.clone();
        self.ui_toolkit.draw_text_input(&string_literal.value,
            move |new_value| {
                let mut new_node = sl.clone();
                new_node.value = new_value.to_string();
                //let mut controller = controller.borrow_mut();
                //controller.update_node(string_literal.id, new_node)
            },
            move |_|{
                let mut controller = controller.borrow_mut();
                controller.set_selected_node_id(None)
            });
        self.ui_toolkit.focus_last_drawn_element();
    }

    fn render_run_button(&self, code_node: &CodeNode) {
        let controller = self.controller.clone();
        let code_node = code_node.clone();
        self.ui_toolkit.draw_button("Run", GREY_COLOR, move ||{
            let mut controller = controller.borrow_mut();
            controller.run(&code_node);
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

        let app = CSApp {
            loaded_code: print_hello_world.clone(),
            controller: Rc::new(RefCell::new(Controller::new())),
        };
        app.controller.borrow_mut().load_code(&print_hello_world);
        app
    }

    fn draw<T: UiToolkit>(self: &Rc<CSApp>, ui_toolkit: &mut T) {
        let app_renderer = AppRenderer {
            ui_toolkit: ui_toolkit,
            controller: self.controller.clone(),
        };

        app_renderer.render_code_window();
        app_renderer.render_console_window();
    }
}
