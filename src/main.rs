#![feature(pattern_parentheses)]
#![feature(unboxed_closures)]
#![feature(specialization)]
#![feature(nll)]
#![feature(arbitrary_self_types)]
#![feature(slice_concat_ext)]
#![recursion_limit="128"]


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
extern crate lazy_static;

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

#[macro_use] extern crate itertools;

#[cfg(feature = "default")]
extern crate pyo3;

extern crate indexmap;

#[macro_use] extern crate downcast_rs;

use itertools::Itertools;

extern crate debug_cell;
use debug_cell::RefCell;

//use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;

mod lang;
mod env;
mod code_loading;
mod editor;
mod editor_views;
mod validation;
mod code_generation;
#[cfg(feature = "default")]
mod pystuff;


#[cfg(feature = "javascript")]
mod fakepystuff;

#[cfg(feature = "javascript")]
mod pystuff {
    pub use super::fakepystuff::*;
}


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
    fn call(&self, env: &mut ExecutionEnvironment, args: HashMap<ID, Value>) -> Value {
        match args.get(&self.takes_args()[0].id) {
            Some(Value::String(ref string)) =>  {
                env.println(string);
                Value::Null
            }
            _ => Value::Error(LangError::ArgumentError)
        }
    }

    fn name(&self) -> &str {
        "Print"
    }

    fn id(&self) -> ID {
        uuid::Uuid::parse_str("b5c18d63-f9a0-4f08-8ee7-e35b3db9122d").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new(
                uuid::Uuid::parse_str("feff08f0-7319-4b47-964e-1f470eca81df").unwrap(),
                lang::Type::from_spec(&lang::STRING_TYPESPEC),
                "String to print".to_string()
            )
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&lang::NULL_TYPESPEC)
    }
}

#[derive(Clone)]
struct Capitalize {}

impl Function for Capitalize {
    fn call(&self, env: &mut ExecutionEnvironment, args: HashMap<ID, Value>) -> Value {
        match args.get(&self.takes_args()[0].id) {
            Some(Value::String(ref string)) =>  {
                Value::String(string.to_uppercase())
            }
            _ => Value::Error(LangError::ArgumentError)
        }
    }

    fn name(&self) -> &str {
        "Capitalize"
    }

    fn id(&self) -> ID {
        uuid::Uuid::parse_str("86ae2a51-5538-436f-b48e-3aa6c873b189").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new(
                uuid::Uuid::parse_str("94e81ddc-843b-426d-847e-a215125c9593").unwrap(),
                lang::Type::from_spec(&lang::STRING_TYPESPEC),
                "String to capitalize".to_string(),
            )
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&lang::STRING_TYPESPEC)
    }
}

#[cfg(feature = "default")]
fn load_builtins(controller: &mut Controller) {
    controller.load_function(Box::new(pystuff::PyFunc::new()));
    controller.load_function(Box::new(pystuff::PyFunc::new()));
}

#[cfg(feature = "javascript")]
fn load_builtins(controller: &mut Controller) {}

pub struct CSApp {
    pub controller: Rc<RefCell<Controller>>,
}

impl CSApp {
    fn new() -> CSApp {
        let codestring = include_str!("../codesample.json");
        let loaded_code = code_loading::deserialize(codestring).unwrap();
        let app = CSApp {
            controller: Rc::new(RefCell::new(Controller::new())),
        };
        app.controller.borrow_mut().load_code(&loaded_code);
        app.controller.borrow_mut().load_function(Box::new(Print{}));
        app.controller.borrow_mut().load_function(Box::new(Capitalize{}));
        load_builtins(&mut app.controller.borrow_mut());
        app
    }

    fn draw<T: UiToolkit>(self: &Rc<CSApp>, ui_toolkit: &mut T) -> T::DrawResult {
        let renderer = Renderer::new(ui_toolkit, Rc::clone(&self.controller));
        renderer.render_app()
    }
}
