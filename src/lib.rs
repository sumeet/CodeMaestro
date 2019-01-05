#![feature(trait_alias)]
#![feature(unboxed_closures)]
#![feature(specialization)]
#![feature(nll)]
#![feature(arbitrary_self_types)]
#![feature(slice_concat_ext)]
#![feature(refcell_replace_swap)]
#![feature(box_patterns)]
#![feature(await_macro, async_await, futures_api)]
#![recursion_limit="256"]
#![feature(fnbox)]

#[cfg(feature = "default")]
mod imgui_support;
#[cfg(feature = "default")]
mod imgui_toolkit;
#[cfg(feature = "javascript")]
mod yew_toolkit;

mod asynk;
pub mod builtin_funcs;
pub mod lang;
mod structs;
pub mod env;
mod code_loading;
mod editor;
mod edit_types;
mod undo;
mod code_generation;
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
fn load_externalfuncs(controller: &mut Controller, world: &code_loading::TheWorld) {
    for pyfunc in world.pyfuncs.iter() {
        controller.load_function(pyfunc.clone());
    }
}

#[cfg(feature = "javascript")]
fn load_externalfuncs(controller: &mut Controller, world: &code_loading::TheWorld) {
    for jsfunc in world.jsfuncs.iter() {
        controller.load_function(jsfunc.clone());
    }
}

fn load_structs(controller: &mut Controller, world: &code_loading::TheWorld) {
    for strukt in world.structs.iter() {
        controller.load_typespec(strukt.clone());
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
    controller.load_code(&the_world.main_code);
    // we could just load these into the env.... lol
    controller.borrow_env(&mut interpreter.env().borrow_mut(), |mut controller| {
        load_externalfuncs(&mut controller, &the_world);
        load_structs(&mut controller, &the_world);
        controller.load_function(builtin_funcs::Print{});
        controller.load_function(builtin_funcs::Capitalize{});
        controller.load_function(builtin_funcs::HTTPGet{});
    });
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
        self.controller.borrow_env(&mut self.interpreter.env().borrow_mut(), |controller| {
            let renderer = editor::Renderer::new(
                ui_toolkit,
                controller,
                Rc::clone(&command_buffer));
            renderer.render_app()
        })
    }

    pub fn flush_commands(&mut self) {
        let command_buffer = Rc::clone(&self.command_buffer);
        self.controller.borrow_env(&mut self.interpreter.env().borrow_mut(), |mut controller| {
            command_buffer.borrow_mut().flush_to_controller(&mut controller);
        });
        command_buffer.borrow_mut().flush_to_interpreter(&mut self.interpreter);
    }

    pub fn turn_event_loop(&mut self) {
        self.interpreter.turn()
    }
}
