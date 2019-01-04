use stdweb::{spawn_local,unwrap_future};

use std::cell::RefCell;
use std::future::Future as NewFuture;
use std::rc::Rc;

pub struct AsyncExecutor {
    onupdate: Option<Rc<Fn()>>,
}

impl AsyncExecutor {
    pub fn new() -> Self {
        Self {
            onupdate: None,
        }
    }

    pub fn setonupdate(&mut self, onupdate: Rc<Fn()>) {
        self.onupdate = Some(onupdate)
    }

    pub fn exec<I, E: std::fmt::Debug, F: NewFuture<Output = Result<I, E>> + 'static>(&mut self, future: F) {
        let mut onupdate = self.onupdate.clone();
        spawn_local(async move {
            await!(future).unwrap();
            onupdate.map(|f| f());
        });
    }

    // no need to turn here. since we're running inside the browser, the event loop is always
    // running
    pub fn turn(&mut self) { }
}
