use std::future::Future as NewFuture;

pub struct AsyncExecutor {}

impl AsyncExecutor {
    pub fn new() -> Self {
        Self {}
    }

    pub fn exec<I, E: std::fmt::Debug, F: NewFuture<Output = Result<I, E>> + 'static>(&mut self, future: F) {
        unimplemented!()
    }
}
