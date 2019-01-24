use std::future::Future as NewFuture;
use std::marker::Unpin;
use futures::Future as OldFuture;


// converts from a new style Future to an old style one:
// javascript needs 0.1 futures while tokio needs 0.3 futures, idk why:
// but see https://github.com/tokio-rs/tokio/pull/819 for how i figured this out
// for some reason you've gotta do this :/
#[cfg(feature = "default")]
pub fn backward<I,E>(f: impl NewFuture<Output=Result<I,E>>) -> impl OldFuture<Item=I, Error=E> {
    use futures_util::compat::Compat;
    Compat::new(Box::pin(f))
}

// or else the futures don't run in JSland... h000what the fuck
//#[cfg(feature = "javascript")]
//pub fn backward<I,E>(f: impl NewFuture<Output=Result<I,E>>) -> impl OldFuture<Item=I, Error=E> {
//    use tokio_async_await::compat::backward;
//    backward::Compat::new(f)
//}

#[allow(dead_code)]
pub fn forward<I,E>(f: impl OldFuture<Item=I, Error=E> + Unpin) -> impl NewFuture<Output=Result<I,E>> {
    use tokio_async_await::compat::forward::IntoAwaitable;
    f.into_awaitable()
}
