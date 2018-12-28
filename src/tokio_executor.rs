use std::future::Future as NewFuture;
//use tokio::runtime::Runtime;
use tokio::runtime::current_thread::Runtime;
//use tokio
use futures::Future as OldFuture;
//use std::pin::Unpin;
use std::marker::Unpin;

// converts from a new style Future to an old style one:
pub fn backward<I,E>(f: impl NewFuture<Output=Result<I,E>>) -> impl OldFuture<Item=I, Error=E> {
    use tokio_async_await::compat::backward;
    backward::Compat::new(f)
}

pub fn forward<I,E>(f: impl OldFuture<Item=I, Error=E> + Unpin) -> impl NewFuture<Output=Result<I,E>> {
    use tokio_async_await::compat::forward::IntoAwaitable;
    f.into_awaitable()
}

pub struct AsyncExecutor {
    runtime: Runtime,
}

impl AsyncExecutor {
    pub fn new() -> Self {
        Self { runtime: Runtime::new().unwrap() }
    }

    pub fn exec<I, E: std::fmt::Debug, F: NewFuture<Output = Result<I, E>> + 'static>(&mut self, future: F) {
        self.runtime.spawn(backward(async {
            await!(future).unwrap();
            Ok(())
        }));
    }
}

