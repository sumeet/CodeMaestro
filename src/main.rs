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
mod editor;


use self::editor::{Controller,Renderer,UiToolkit};
use self::env::{ExecutionEnvironment};
use self::lang::{
    Value,CodeNode,Function,FunctionCall,FunctionReference,StringLiteral,ID,Error as LangError,Assignment,Block,
    VariableReference};

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

pub struct CSApp {
    pub controller: Rc<RefCell<Controller>>,
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
        let renderer = Renderer::new(ui_toolkit, Rc::clone(&self.controller));
        renderer.render_app()
    }
}
