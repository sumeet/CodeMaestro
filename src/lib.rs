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


use std::cell::RefCell;
use std::rc::Rc;

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

pub struct CSApp {
    pub controller: Rc<RefCell<Controller>>,
}

impl CSApp {
    pub fn new() -> CSApp {
        let app = CSApp {
            controller: Rc::new(RefCell::new(Controller::new())),
        };
        app.controller.borrow_mut().load_function(builtin_funcs::Print{});
        app.controller.borrow_mut().load_function(builtin_funcs::Capitalize{});

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
        let renderer = Renderer::new(ui_toolkit, Rc::clone(&self.controller));
        renderer.render_app()
    }
}
