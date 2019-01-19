#![feature(trait_alias)]
#![feature(unboxed_closures)]
#![feature(specialization)]
#![feature(nll)]
#![feature(arbitrary_self_types)]
#![feature(slice_concat_ext)]
#![feature(refcell_replace_swap)]
#![feature(box_patterns)]
#![feature(await_macro, async_await, futures_api)]
#![feature(try_from)]
#![feature(slice_patterns)]
#![recursion_limit="256"]
#![feature(fnbox)]

#[cfg(feature = "default")]
mod imgui_support;
#[cfg(feature = "default")]
mod imgui_toolkit;
#[cfg(feature = "javascript")]
mod yew_toolkit;

mod asynk;
pub mod builtins;
pub mod lang;
mod structs;
mod enums;
pub mod env;
mod env_genie;
mod code_loading;
mod editor;
mod insert_code_menu;
mod code_editor;
mod code_editor_renderer;
mod edit_types;
mod json;
mod undo;
mod json_http_client;
mod code_generation;
mod code_validation;
mod function;
mod code_function;
mod external_func;
#[cfg(feature = "default")]
mod pystuff;

#[cfg(feature = "javascript")]
mod fakepystuff;

#[cfg(feature = "javascript")]
mod pystuff {
    pub use super::fakepystuff::*;
}

#[cfg(feature = "javascript")]
mod jsstuff;

#[cfg(feature = "default")]
mod fakejsstuff;

#[cfg(feature = "default")]
mod jsstuff {
    pub use super::fakejsstuff::*;
}


#[cfg(feature = "default")]
mod tokio_executor;

#[cfg(feature = "default")]
mod async_executor {
    pub use super::tokio_executor::*;
}

#[cfg(feature = "javascript")]
mod stdweb_executor;

#[cfg(feature = "javascript")]
mod async_executor {
    pub use super::stdweb_executor::*;
}

#[cfg(feature = "javascript")]
mod wasm_http_client;

#[cfg(feature = "javascript")]
mod http_client {
    pub use super::wasm_http_client::*;
}


#[cfg(feature = "default")]
mod native_http_client;

#[cfg(feature = "default")]
mod http_client {
    pub use super::native_http_client::*;
}


use std::cell::RefCell;
use std::rc::Rc;

use self::editor::{Controller,UiToolkit};
use self::env::{ExecutionEnvironment};
//use debug_cell::RefCell;

#[cfg(feature = "default")]
pub fn draw_app(app: Rc<RefCell<App>>) {
    imgui_toolkit::draw_app(app);
}

#[cfg(feature = "javascript")]
pub fn draw_app(app: Rc<RefCell<App>>) {
    yew_toolkit::draw_app(app);
}

#[cfg(feature = "default")]
fn load_externalfuncs(env: &mut ExecutionEnvironment, world: &code_loading::TheWorld) {
    for pyfunc in world.pyfuncs.iter() {
        env.add_function(pyfunc.clone());
    }
}

#[cfg(feature = "javascript")]
fn load_externalfuncs(env: &mut ExecutionEnvironment, world: &code_loading::TheWorld) {
    for jsfunc in world.jsfuncs.iter() {
        env.add_function(jsfunc.clone());
    }
}

fn load_structs(env: &mut ExecutionEnvironment, world: &code_loading::TheWorld) {
    for strukt in world.structs.iter() {
        env.add_typespec(strukt.clone());
    }
    for eneom in world.enums.iter() {
        env.add_typespec(eneom.clone());
    }
}

pub fn main() {
    let app = App::new_rc();
    draw_app(app);
}

fn init_controller(interpreter: &env::Interpreter) -> Controller {
    let mut controller = Controller::new();
    // TODO: controller can load the world as well as saving it, i don't think the code should
    // be in here
    let codestring = include_str!("../codesample.json");
    let the_world: code_loading::TheWorld = code_loading::deserialize(codestring).unwrap();
//    for code in &the_world.codes {
//        controller.load_code(code);
//    }

    let env = interpreter.env();
    let mut env = env.borrow_mut();
    load_externalfuncs(&mut env, &the_world);
    load_structs(&mut env, &the_world);
    env.add_function(builtins::Print{});
    env.add_function(builtins::Capitalize{});
    env.add_function(builtins::HTTPGet{});

    controller
}

pub struct App {
    pub interpreter: env::Interpreter,
    command_buffer: Rc<RefCell<editor::CommandBuffer>>,
    controller: Controller,
}

impl App {
    pub fn new() -> Self {
        let interpreter = env::Interpreter::new();
        let command_buffer =
            Rc::new(RefCell::new(editor::CommandBuffer::new()));
        let controller = init_controller(&interpreter);
        Self { interpreter, command_buffer, controller }
    }

    pub fn new_rc() -> Rc<RefCell<App>> {
        Rc::new(RefCell::new(Self::new()))
    }

    pub fn draw<T: UiToolkit>(&mut self, ui_toolkit: &mut T) -> T::DrawResult {
        let command_buffer = Rc::clone(&self.command_buffer);
        let env = self.interpreter.env();
        let env = env.borrow();
        let env_genie = env_genie::EnvGenie::new(&env);
        let renderer = editor::Renderer::new(
            ui_toolkit,
            &self.controller,
            Rc::clone(&command_buffer),
            &env_genie);
        renderer.render_app()
    }

    pub fn flush_commands(&mut self) {
        let mut command_buffer = self.command_buffer.borrow_mut();
        if !command_buffer.has_queued_commands() {
            return;
        }
        command_buffer.flush_to_controller(&mut self.controller);
        command_buffer.flush_to_interpreter(&mut self.interpreter);
        command_buffer.flush_integrating(&mut self.controller,
                                         &mut self.interpreter.env().borrow_mut());
        code_validation::validate_and_fix(&mut self.interpreter.env().borrow_mut());
    }
}
