use stdweb::{spawn_local};

use std::future::Future as NewFuture;
use std::rc::Rc;

pub fn with_executor_context<F: FnOnce(AsyncExecutor)>(run: F) {
    run(AsyncExecutor::new())
}

#[derive(Clone)]
pub struct AsyncExecutor {
    pub onupdate: Option<Rc<Fn()>>,
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
        let onupdate = self.onupdate.clone();
        spawn_local(async move {
            await!(future).unwrap();
            onupdate.map(|f| f());
        });
    }

    // no need to turn here. since we're running inside the browser, the event loop is always
    // running
    //pub fn turn(&mut self) { }
}
