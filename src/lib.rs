#![feature(unboxed_closures)]
#![feature(specialization)]
#![feature(nll)]
#![feature(arbitrary_self_types)]
#![feature(slice_concat_ext)]
#![feature(refcell_replace_swap)]
#![feature(box_patterns)]
#![feature(await_macro, async_await, futures_api)]
#![recursion_limit="256"]

#[cfg(feature = "default")]
mod imgui_support;
#[cfg(feature = "default")]
mod imgui_toolkit;
#[cfg(feature = "javascript")]
mod yew_toolkit;

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


use std::cell::RefCell;
use std::rc::Rc;
use std::future::Future;

use self::editor::{Controller,Renderer,UiToolkit};
use self::env::{ExecutionEnvironment};
//use debug_cell::RefCell;

#[cfg(feature = "default")]
pub fn draw_app(app: Rc<CSApp>) {
    imgui_toolkit::draw_app(Rc::clone(&app));
}

#[cfg(feature = "javascript")]
pub fn draw_app(app: Rc<CSApp>) {
    yew_toolkit::draw_app(Rc::clone(&app));
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

pub fn newmain() {
    let interpreter = env::Interpreter::new();
    // TODO: pass this into the renderer, but for now we'll just not use it
    let _command_buffer = Rc::new(RefCell::new(editor::CommandBuffer::new()));

    let mut controller = init_controller(&interpreter);
    // we're just passing this into the renderer so it has something. we're gonna replace this with the command buffer :>
    let deadcontroller = Rc::new(RefCell::new(init_controller(&interpreter)));

    let mut command_buffer = Rc::new(RefCell::new(editor::CommandBuffer::new()));

    imgui_support::run(
        "cs".to_string(),
        imgui_toolkit::CLEAR_COLOR,
        move |ui, keypress| {
            let mut toolkit = imgui_toolkit::ImguiToolkit::new(ui, keypress);
            controller.borrow_env(&mut interpreter.env().borrow_mut(), |mut controller| {
                let renderer = editor::Renderer::new(
                    &mut toolkit,
                    controller,
                    Rc::clone(&deadcontroller),
                    Rc::clone(&command_buffer));
                renderer.render_app();
            });
            true
       },
    );
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
    });
    controller
}

pub struct CSApp {
    pub controller: Rc<RefCell<Controller>>,
    pub async_executor: Rc<RefCell<async_executor::AsyncExecutor>>,
}

impl CSApp {
    pub fn new() -> CSApp {
        let interpreter = env::Interpreter::new();
        let async_executor2 =
            Rc::clone(&interpreter.async_executor);

        let app = CSApp {
            async_executor: async_executor2,
            controller: Rc::new(RefCell::new(Controller::new())),
        };

        // TODO: controller can load the world as well as saving it, i don't think the code should
        // be in here
        let codestring = include_str!("../codesample.json");
        let the_world : code_loading::TheWorld = code_loading::deserialize(codestring).unwrap();
        app.controller.borrow_mut().load_code(&the_world.main_code);
        load_externalfuncs(&mut app.controller.borrow_mut(), &the_world);
        load_structs(&mut app.controller.borrow_mut(), &the_world);

        app
    }

    fn draw<T: UiToolkit>(self: &Rc<CSApp>, ui_toolkit: &mut T) -> T::DrawResult {
        let renderer = Renderer::new(
            ui_toolkit,
            unimplemented!(),
            Rc::clone(&self.controller),
            unimplemented!());
        renderer.render_app()
    }
}

