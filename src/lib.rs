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

pub struct CSApp {
    pub controller: Rc<RefCell<Controller>>,
}

impl CSApp {
    pub fn new() -> CSApp {
        let app = CSApp {
            controller: Rc::new(RefCell::new(Controller::new(create_executor()))),
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

#[cfg(feature = "default")]
use tokio::runtime::Runtime;

#[cfg(feature = "default")]
pub struct TokioExecutor {
    runtime: Runtime,
}

#[cfg(feature = "default")]
impl TokioExecutor {
    pub fn new() -> Self {
        Self { runtime: Runtime::new().unwrap() }
    }
}

use std::future::Future as NewFuture;
use futures::Future as OldFuture;

// converts from a new style Future to an old style one:
fn backward<I,E>(f: impl NewFuture<Output=Result<I,E>>) -> impl OldFuture<Item=I, Error=E> {
    use tokio_async_await::compat::backward;
    backward::Compat::new(f)
}

#[cfg(feature = "default")]
impl env::AsyncExecutor for TokioExecutor {
    fn exec<I, E, F: Future<Output = Result<I, E>> + Send + 'static>(&mut self, future: F) where Self: Sized {
        self.runtime.spawn(backward(async {
            await!(future);
            Ok(())
        }));
    }
}

#[cfg(feature = "default")]
pub fn create_executor() -> TokioExecutor {
    TokioExecutor::new()
}



#[cfg(feature = "javascript")]
pub struct StdwebExecutor {}

#[cfg(feature = "javascript")]
impl StdwebExecutor {
    pub fn new() -> Self {
        Self {}
    }
}

#[cfg(feature = "javascript")]
impl env::AsyncExecutor for StdwebExecutor {
    fn exec<F: Future>(&self, future: F) where Self: Sized {
        unimplemented!()
    }
}

#[cfg(feature = "javascript")]
pub fn create_executor() -> StdwebExecutor {
}
