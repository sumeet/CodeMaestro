use std::future::Future as NewFuture;
use tokio_current_thread::CurrentThread;
//use tokio_current_thread::TaskExecutor;
use std::time;
use super::asynk::{backward};
use take_mut::take;
//
//#[derive(Debug)]
//pub struct AsyncExecutor {
//    current_thread: CurrentThread,
//}
//
//impl AsyncExecutor {
//    pub fn new() -> Self {
//        Self { current_thread: CurrentThread::new() }
//    }
//
//    pub fn turn(&mut self) {
//        let duration = time::Duration::from_millis(30);
//        self.current_thread.turn(Some(duration)).unwrap();
//    }
//
//    pub fn exec<I, E: std::fmt::Debug, F: NewFuture<Output = Result<I, E>> + 'static>(&mut self, future: F) {
//        self.current_thread.spawn(backward(async {
//            await!(future).unwrap();
//            Ok(())
//        }));
//    }
//}

use std::io::Error as IoError;
use std::time::{Duration, Instant};

use futures::{future, Future};
//use tokio_current_thread::CurrentThread;
use tokio_reactor::Reactor;
use tokio_timer::timer::{self, Timer};
//use std::time;
//use super::asynk::{backward};
//use std::future::Future as NewFuture;
use std::cell::RefCell;


pub fn with_executor_context<F: FnOnce(tokio_current_thread::Entered<tokio_timer::timer::Timer<tokio_reactor::Reactor>>)>
    (run: F) {
    // We need a reactor to receive events about IO objects from kernel
    let reactor = Reactor::new().unwrap();
    let reactor_handle = reactor.handle();
    // Place a timer wheel on top of the reactor. If there are no timeouts to fire, it'll let the
    // reactor pick up some new external events.
    let timer = Timer::new(reactor);
    let timer_handle = timer.handle();
    // And now put a single-threaded executor on top of the timer. When there are no futures ready
    // to do something, it'll let the timer or the reactor generate some new stimuli for the
    // futures to continue in their life.
    let mut executor = CurrentThread::new_with_park(timer);
    // Binds an executor to this thread
    let mut enter = tokio_executor::enter().expect("Multiple executors at once");
    // This will set the default handle and timer to use inside the closure and run the future.
    tokio_reactor::with_default(&reactor_handle, &mut enter, |enter| {
        timer::with_default(&timer_handle, enter, |enter| {
            // The TaskExecutor is a fake executor that looks into the current single-threaded
            // executor when used. This is a trick, because we need two mutable references to the
            // executor (one to run the provided future, another to install as the default one). We
            // use the fake one here as the default one.
            let mut default_executor = tokio_current_thread::TaskExecutor::current();
            tokio_executor::with_default(&mut default_executor, enter, |enter| {
                let mut executor = executor.enter(enter);
                run(executor);
            });
        });
    });
}

#[derive(Debug)]
pub struct AsyncExecutor<'a> {
    executor: tokio_current_thread::Entered<'a, tokio_timer::timer::Timer<tokio_reactor::Reactor>>
}

impl<'a> AsyncExecutor<'a> {
    pub fn new(executor: tokio_current_thread::Entered<'a, tokio_timer::timer::Timer<tokio_reactor::Reactor>>) -> Self {
        Self { executor }
    }

    pub fn turn(&mut self) {
        let duration = time::Duration::from_millis(30);
        self.executor.turn(Some(duration)).unwrap();
    }

    pub fn exec<I, E: std::fmt::Debug, F: NewFuture<Output = Result<I, E>> + 'static>(&mut self, future: F) {
        self.executor.spawn(backward(async {
            await!(future).unwrap();
            Ok(())
        }));
    }
}
