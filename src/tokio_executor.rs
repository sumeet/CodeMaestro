use std::future::Future as NewFuture;
//use tokio::runtime::Runtime;
//use tokio::runtime::current_thread::Runtime;
use tokio_current_thread::CurrentThread;
//use tokio
use futures::Future as OldFuture;
//use std::pin::Unpin;
use std::marker::Unpin;
use std::time;
use super::asynk::{backward,forward};

#[derive(Debug)]
pub struct AsyncExecutor {
    current_thread: CurrentThread,
}

impl AsyncExecutor {
    pub fn new() -> Self {
        Self { current_thread: CurrentThread::new() }
    }

    pub fn turn(&mut self) {
        let duration = time::Duration::from_millis(10);
        self.current_thread.turn(Some(duration)).unwrap();
    }

    pub fn exec<I, E: std::fmt::Debug, F: NewFuture<Output = Result<I, E>> + 'static>(&mut self, future: F) {
        self.current_thread.spawn(backward(async {
            await!(future).unwrap();
            Ok(())
        }));
    }
}
