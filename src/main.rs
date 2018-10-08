#![feature(pattern_parentheses)]
#![feature(unboxed_closures)]
#![feature(specialization)]
#![feature(nll)]
#![feature(arbitrary_self_types)]
#![feature(slice_concat_ext)]

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
extern crate stdweb;

#[cfg(feature = "javascript")]
#[macro_use]
extern crate yew;

#[cfg(feature = "javascript")]
mod yew_toolkit;

#[macro_use]
extern crate objekt;

extern crate uuid;

#[macro_use]
extern crate failure;
use failure::{err_msg};
use failure::Error as Error;

#[macro_use]
extern crate serde_derive;

extern crate serde;
extern crate serde_json;

#[macro_use]
extern crate erased_serde;

extern crate debug_cell;
use debug_cell::RefCell;

//use std::cell::RefCell;
use std::rc::Rc;

mod lang;
mod env;
mod code_loading;

use self::env::{ExecutionEnvironment};
use self::lang::{
    Value,CodeNode,Function,FunctionCall,FunctionReference,StringLiteral,ID,Error as LangError,Assignment,Block,
    VariableReference};

const BLUE_COLOR: [f32; 4] = [0.196, 0.584, 0.721, 1.0];
const RED_COLOR: [f32; 4] = [0.858, 0.180, 0.180, 1.0];
const GREY_COLOR: [f32; 4] = [0.521, 0.521, 0.521, 1.0];
const PURPLE_COLOR: [f32; 4] = [0.486, 0.353, 0.952, 1.0];
const CLEAR_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

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
            _ => Value::Result(Result::Err(LangError::ArgumentError))
        }
    }

    fn name(&self) -> &str {
        "Print"
    }

    fn id(&self) -> ID {
        uuid::Uuid::parse_str("b5c18d63-f9a0-4f08-8ee7-e35b3db9122d").unwrap()
    }
}

pub struct Controller {
    execution_environment: ExecutionEnvironment,
    selected_node_id: Option<ID>,
    editing: bool,
    loaded_code: Option<CodeNode>,
    error_console: String,
}

#[derive(Debug)]
pub enum Key {
    A,
    B,
    C,
    D,
    W,
    X,
    R
}

impl<'a> Controller {
    fn new() -> Controller {
        Controller {
            execution_environment: ExecutionEnvironment::new(),
            selected_node_id: None,
            loaded_code: None,
            error_console: String::new(),
            editing: false,
        }
    }

    fn handle_key_press(&mut self, key: Key) {
        // don't perform any commands when in edit mode
        if self.editing { return }
        match key {
            Key::B => {
                self.try_select_back_one_node()
            },
            Key::W => {
                self.try_select_forward_one_node()
            },
            Key::C => {
                self.editing = true
            },
            Key::R => {
                self.run(&self.loaded_code.as_ref().unwrap().clone())
            }
            _ => {},
        }
    }

    fn try_select_back_one_node(&mut self) {
        let root_node_was_selected = self.select_loaded_code_if_nothing_selected();
        if root_node_was_selected.is_err() || root_node_was_selected.unwrap() {
            // if nothing was selected, and we selected the root node, then our job is done.
            return
        }

        let selected_node_id = self.get_selected_node_id().unwrap();
        let mut loaded_code = self.loaded_code.as_ref().unwrap().clone();
        let parent = loaded_code.find_parent(selected_node_id);
        if parent.is_none() {
            return
        }
        let mut parent = parent.unwrap();

        // first try selecting the previous sibling
        if let(Some(mut previous_sibling)) = parent.previous_child(selected_node_id) {
            // but since we're going back, if the previous sibling has children, then let's
            // select the last one. that feels more ergonomic while moving backwards
            let children = previous_sibling.children();
            if children.len() > 0 {
                self.set_selected_node_id(Some(children[0].id()))
            } else {
                self.set_selected_node_id(Some(previous_sibling.id()));
            }
            return
        }

        // if there is no previous sibling, select the parent
        self.set_selected_node_id(Some(parent.id()));
    }

    fn try_select_forward_one_node(&mut self) {
        let root_node_was_selected = self.select_loaded_code_if_nothing_selected();
        if root_node_was_selected.is_err() || root_node_was_selected.unwrap() {
            // if nothing was selected, and we selected the root node, then our job is done.
            return
        }

        let selected_node_id = self.get_selected_node_id().unwrap();
        let mut loaded_code = self.loaded_code.as_ref().unwrap().clone();

        let mut selected_code = loaded_code.find_node(selected_node_id).as_mut().unwrap().clone();
        let children = selected_code.children();
        let first_child = children.get(0);

        if let(Some(first_child)) = first_child {
            self.set_selected_node_id(Some(first_child.id()));
            return
        }

        let parent = loaded_code.find_parent(selected_node_id);
        if parent.is_none() {
            return
        }
        let parent = parent.unwrap();
        if let(Some(next_sibling)) = loaded_code.next_child(parent.id()) {
            self.set_selected_node_id(Some(next_sibling.id()));
            return
        }
    }

    fn select_loaded_code_if_nothing_selected(&mut self) -> Result<bool,Error> {
        if self.loaded_code.is_none() { return Err(err_msg("No code loaded")) }
        let loaded_code = self.loaded_code.as_ref().unwrap().clone();
        if self.get_selected_node_id().is_none() {
            self.set_selected_node_id(Some(loaded_code.id()));
            return Ok(true)
        }
        Ok(false)
    }

    fn load_function(&mut self, function: Box<Function>) {
        self.execution_environment.add_function(function.clone())
    }

    fn find_function(&self, id: ID) -> Option<&Box<Function>> {
        self.execution_environment.find_function(id)
    }

    fn load_code(&mut self, code_node: &CodeNode) {
        self.loaded_code = Some(code_node.clone())
    }

    // should run the loaded code node
    fn run(&mut self, code_node: &CodeNode) {
        match self.execution_environment.evaluate(code_node) {
            Value::Result(Err(e)) => {
                self.error_console.push_str(&format!("{:?}", e));
                self.error_console.push_str("\n");
            }
            _ => { }
        }
    }

    fn read_console(&self) -> &str {
        &self.execution_environment.console
    }

    fn read_error_console(&self) -> &str {
        &self.error_console
    }

    fn set_selected_node_id(&mut self, code_node_id: Option<ID>) {
        self.selected_node_id = code_node_id;
    }

    fn get_selected_node_id(&self) -> &Option<ID> {
        &self.selected_node_id
    }
}

pub struct CSApp {
    pub controller: Rc<RefCell<Controller>>,
}

trait UiToolkit {
    type DrawResult;

    fn draw_all(&self, draw_results: Vec<Self::DrawResult>) -> Self::DrawResult;
    fn draw_window(&self, window_name: &str, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_layout_with_bottom_bar(&self, draw_content_fn: &Fn() -> Self::DrawResult, draw_bottom_bar_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_empty_line(&self) -> Self::DrawResult;
    fn draw_button<F: Fn() + 'static>(&self, label: &str, color: [f32; 4], f: F) -> Self::DrawResult;
    fn draw_text_box(&self, text: &str) -> Self::DrawResult;
    fn draw_text_input<F: Fn(&str) -> () + 'static, D: Fn() + 'static>(&self, existing_value: &str, onchange: F, ondone: D) -> Self::DrawResult;
    fn draw_all_on_same_line(&self, draw_fns: Vec<&Fn() -> Self::DrawResult>) -> Self::DrawResult;
    fn draw_border_around(&self, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn focused(&self, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
}

struct Renderer<'a, T> {
    ui_toolkit: &'a mut T,
    controller: Rc<RefCell<Controller>>,
}

impl<'a, T: UiToolkit> Renderer<'a, T> {
    fn render_app(&self) -> T::DrawResult {
        self.ui_toolkit.draw_all(vec![
            self.render_code_window(),
            self.render_console_window(),
            self.render_error_window(),
        ])
    }

    fn render_console_window(&self) -> T::DrawResult {
        let controller = self.controller.clone();
        self.ui_toolkit.draw_window("Console", &|| {
            self.ui_toolkit.draw_text_box(controller.borrow().read_console())
        })
    }

    fn render_error_window(&self) -> T::DrawResult {
        let controller = self.controller.clone();
        self.ui_toolkit.draw_window("Errors", &|| {
            self.ui_toolkit.draw_text_box(controller.borrow().read_error_console())
        })
    }

    fn render_code_window(&self) -> T::DrawResult {
        let loaded_code = self.controller.borrow().loaded_code.clone();
        match loaded_code {
            None => {
                self.ui_toolkit.draw_button("No code loaded", CLEAR_COLOR, &||{})
            },
            // TODO this just looks weird now. we should put the code in a child frame, and
            // the run button at the bottom, like in this example: https://github.com/ocornut/imgui/issues/425
            Some(ref code) => {
                self.ui_toolkit.draw_window(&code.description(), &|| {
                    self.ui_toolkit.draw_layout_with_bottom_bar(
                        &||{ self.render_code(code) },
                        &||{ self.render_run_button(code) }
                    )})
            }
        }
    }

    fn render_code(&self, code_node: &CodeNode) -> T::DrawResult {
        if self.is_editing(code_node) {
            return self.ui_toolkit.focused(&|| { self.draw_inline_editor(code_node) })
        }
        let draw = ||{
            match code_node {
                CodeNode::FunctionCall(function_call) => {
                    self.render_function_call(&function_call)
                }
                CodeNode::StringLiteral(string_literal) => {
                    self.render_string_literal(&string_literal)
                }
                CodeNode::Assignment(assignment) => {
                    self.render_assignment(&assignment)
                }
                CodeNode::Block(block) => {
                    self.render_block(&block)
                }
                CodeNode::VariableReference(variable_reference) => {
                    self.render_variable_reference(&variable_reference)
                }
                CodeNode::FunctionDefinition(function_definition) => {
                    self.ui_toolkit.draw_button(
                        &"Function defs are unimplemented",
                        RED_COLOR,
                        ||{}
                    )
                }
                CodeNode::FunctionReference(function_reference) => {
                    self.render_function_reference(&function_reference)
//                    self.ui_toolkit.draw_button(
//                        &"Function refs are unimplemented",
//                        RED_COLOR,
//                        ||{}
//                    )
                }
            }
        };
        if self.is_selected(code_node) {
            self.ui_toolkit.draw_border_around(&draw)
        } else {
            draw()
        }
    }

    fn render_assignment(&self, assignment: &Assignment) -> T::DrawResult {
        self.ui_toolkit.draw_all_on_same_line(vec![
            &|| {
                self.render_inline_editable_button(
                    &assignment.name,
                    PURPLE_COLOR,
                    &CodeNode::Assignment(assignment.clone())
                )
            },
            &|| { self.ui_toolkit.draw_button("=", CLEAR_COLOR, &|| {}) },
            &|| { self.render_code(assignment.expression.as_ref()) }
        ])
    }

    fn render_variable_reference(&self, variable_reference: &VariableReference) -> T::DrawResult {
        let mut controller = self.controller.borrow_mut();
        let loaded_code = controller.loaded_code.as_mut().unwrap();
        let assignment = loaded_code.find_node(variable_reference.assignment_id);
        if let(Some(CodeNode::Assignment(assignment))) = assignment {
            self.ui_toolkit.draw_button(&assignment.name, PURPLE_COLOR, &|| {})
        } else {
            self.ui_toolkit.draw_button("Variable reference not found", RED_COLOR, &|| {})
        }
    }

    fn render_block(&self, block: &Block) -> T::DrawResult {
        self.ui_toolkit.draw_all(
            block.expressions.iter().map(|code| self.render_code(code)).collect())
    }

    fn render_function_reference(&self, function_reference: &FunctionReference) -> T::DrawResult {
        let function_id = function_reference.function_id;

        // TODO: don't do validation in here. this is just so i can see what this error looks
        // like visually. for realz, i would probably be better off having a separate validation
        // step. and THEN show the errors in here. or maybe overlay something on the codenode that
        // contains the error
        let mut color = RED_COLOR;
        let mut function_name = format!("Error: function ID {} not found", function_id);

        if let(Some(function)) = self.controller.borrow_mut().find_function(function_id) {
            color = BLUE_COLOR;
            function_name = function.name().to_string();
        }
        self.ui_toolkit.draw_button(&function_name, color, &|| {})
    }

    fn render_function_call(&self, function_call: &FunctionCall) -> T::DrawResult {
        let render_function_reference_fn = || {
            self.render_function_reference(&function_call.function_reference)
        };

        let mut arg_renderers : Vec<Box<Fn() -> T::DrawResult>> = vec![Box::new(render_function_reference_fn)];
        let args = function_call.args.clone();
        for arg in args {
            let render_fn = move || { self.render_code(&arg) };
            arg_renderers.push(Box::new(render_fn));
        }
        self.ui_toolkit.draw_all_on_same_line(
            arg_renderers.iter()
                .map(|b| {
                    b.as_ref()
                })
                .collect())
    }

    fn render_string_literal(&self, string_literal: &StringLiteral) -> T::DrawResult {
        self.render_inline_editable_button(
            &string_literal.value,
            CLEAR_COLOR,
            &CodeNode::StringLiteral(string_literal.clone())
        )
    }

    fn render_run_button(&self, code_node: &CodeNode) -> T::DrawResult {
        let controller = self.controller.clone();
        let code_node = code_node.clone();
        self.ui_toolkit.draw_button("Run", GREY_COLOR, move ||{
            let mut controller = controller.borrow_mut();
            controller.run(&code_node);
        })
    }

    fn render_inline_editable_button(&self, label: &str, color: [f32; 4], code_node: &CodeNode) -> T::DrawResult {
        let controller = self.controller.clone();
        let id = code_node.id();
        self.ui_toolkit.draw_button(label, color, move || {
            let mut controller = controller.borrow_mut();
            controller.set_selected_node_id(Some(id));
            controller.editing = true;
        })
    }

    fn is_selected(&self, code_node: &CodeNode) -> bool {
        Some(code_node.id()) == *self.controller.borrow().get_selected_node_id()
    }

    fn is_editing(&self, code_node: &CodeNode) -> bool {
        self.is_selected(code_node) && self.controller.borrow().editing
    }

    fn draw_inline_editor(&self, code_node: &CodeNode) -> T::DrawResult {
        match code_node {
            CodeNode::StringLiteral(string_literal) => {
                let sl = string_literal.clone();
                self.draw_inline_text_editor(
                    &string_literal.value,
                    move |new_value| {
                        let mut new_literal = sl.clone();
                        new_literal.value = new_value.to_string();
                        CodeNode::StringLiteral(new_literal)
                    })
            },
            CodeNode::Assignment(assignment) => {
                let a = assignment.clone();
                self.draw_inline_text_editor(
                    &assignment.name,
                    move |new_value| {
                        let mut new_assignment = a.clone();
                        new_assignment.name = new_value.to_string();
                        CodeNode::Assignment(new_assignment)
                    })
            },
            _ => {
                self.controller.borrow_mut().editing = false;
                self.ui_toolkit.draw_button(&format!("Not possible to edit {:?}", code_node), RED_COLOR, &||{})
            }
        }
    }

    fn draw_inline_text_editor<F: Fn(&str) -> CodeNode + 'static>(&self, initial_value: &str, new_node_fn: F) -> T::DrawResult {
        let controller = Rc::clone(&self.controller);
        let controller2 = Rc::clone(&self.controller);
        self.ui_toolkit.draw_text_input(
            initial_value,
            move |new_value| {
                let new_node = new_node_fn(new_value);
                controller.borrow_mut().loaded_code.as_mut().unwrap().replace(&new_node)
            },
            move || {
                controller2.borrow_mut().editing = false
            }

        )
    }
}

impl CSApp {
    fn new() -> CSApp {
        let codestring = include_str!("../codesample");
        let loaded_code = code_loading::deserialize(codestring).unwrap();
        let app = CSApp {
            controller: Rc::new(RefCell::new(Controller::new())),
        };
        app.controller.borrow_mut().load_code(&loaded_code);
        app.controller.borrow_mut().load_function(Box::new(Print{}));
        app
    }

    fn draw<T: UiToolkit>(self: &Rc<CSApp>, ui_toolkit: &mut T) -> T::DrawResult {
        let renderer = Renderer {
            ui_toolkit: ui_toolkit,
            controller: self.controller.clone(),
        };
        renderer.render_app()
    }
}
