use stdweb::{spawn_local,unwrap_future};

use std::future::Future as NewFuture;

#[derive(Debug)]
pub struct AsyncExecutor {}

impl AsyncExecutor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn exec<I, E: std::fmt::Debug, F: NewFuture<Output = Result<I, E>> + 'static>(&mut self, future: F) {
        spawn_local(async {
            await!(future).unwrap();
        });
    }

    // no need to turn here. since we're running inside the browser, the event loop is always
    // running
    pub fn turn(&mut self) { }
}
