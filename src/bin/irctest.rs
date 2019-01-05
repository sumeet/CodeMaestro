#![feature(await_macro, async_await, futures_api)]

extern crate cs;

use cs::lang::Value;
use std::marker::Unpin;
use tokio::prelude::*;
use irc::client::prelude::*;
use irc::client::PackedIrcClient;
use irc_proto::{Command};
use tokio::runtime::current_thread::Runtime;
//use irc::error;
use std::future::Future as NewFuture;
use futures::Future as OldFuture;

// converts from a new style Future to an old style one:
fn backward<I,E>(f: impl NewFuture<Output=Result<I,E>>) -> impl OldFuture<Item=I, Error=E> {
    use tokio_async_await::compat::backward;
    backward::Compat::new(f)
}

// converts from an old style Future to a new style one:
fn forward<I,E>(f: impl OldFuture<Item=I, Error=E> + Unpin) -> impl NewFuture<Output=Result<I,E>> {
    use tokio_async_await::compat::forward::IntoAwaitable;
    f.into_awaitable()
}

fn main() {
    let config = Config {
        nickname: Some("cs".to_owned()),
        server: Some("irc.darwin.network".to_owned()),
        channels: Some(vec!["#darwin".to_owned()]),
        use_ssl: Some(true),
        password: Some("smellyoulater".to_string()),
        port: Some(6697),
        ..Config::default()
    };

//    let config = Config {
//        nickname: Some("cs".to_owned()),
//        server: Some("127.0.0.1".to_owned()),
//        channels: Some(vec!["#darwin".to_owned()]),
//        use_ssl: Some(false),
//        port: Some(6667),
//        ..Config::default()
//    };

    let mut runtime = Runtime::new().unwrap();
    let irc_client_future = IrcClient::new_future(config).unwrap();
    let PackedIrcClient(client, future) = runtime.block_on(irc_client_future).unwrap();

    client.identify().unwrap();
    let slaq = slack();

    let slack_future = backward(async move {
        await!(forward(slaq)).unwrap();
        Ok::<(), ()>(())
    });

    let irc_future = backward(async move {
        await!(forward(future)).unwrap();
        Ok::<(), ()>(())
    });

    runtime.block_on(slack_future.join(irc_future)).unwrap();

//    runtime.block_on(backward(async move {
////        let app = cs::App::new();
////        let loaded_code = app.controller.borrow().loaded_code.clone().unwrap();
////        let mut controller = app.controller.borrow_mut();
////        let env = &mut controller.execution_environment;
////
////        let mut stream = client.stream();
////        while let Some(message) = await!(stream.next()) {
////            if message.is_err() {
////                println!("there was an error: {:?}", message)
////            } else {
////                let message = message.unwrap();
////                println!("{:?}", message);
////                if let Command::PRIVMSG(_, _) = message.command {
////                    if let Some(response_target) = message.response_target() {
////                        let output_from_lang = env.evaluate(&loaded_code);
////                        if let Value::String(output_string) = output_from_lang {
////                            client.send_privmsg(response_target, output_string).unwrap();
////                        }
////                    }
////                }
////            }
////        }
////        Ok(())
//        Ok(())
//    }).join(future).join(slack(&runtime))).unwrap();
}

fn slack() -> impl OldFuture<Error = impl std::fmt::Debug> {
    use slack::{Event, Message};
    use slack::api::MessageStandard;
    use slack::future::client::{Client, EventHandler};
    use futures::future::{ok, FutureResult};

    struct MyHandler;

    impl EventHandler for MyHandler {
        type EventFut = FutureResult<(), ()>;
        type OnCloseFut = FutureResult<(), ()>;
        type OnConnectFut = FutureResult<(), ()>;

        fn on_event(&mut self, _cli: &Client, event: Event) -> Self::EventFut {
            // print out the event
            println!("event = {:#?}", event);
            // do something if it's a `Message::Standard`
            if let Event::Message(ref message) = event {
                if let Message::Standard(MessageStandard {
                                             ref channel,
                                             ref user,
                                             ref text,
                                             ref ts,
                                             ..
                                         }) = **message {
                    println!("{:?}, {:?}, {:?}, {:?}", channel, user, text, ts);
                }
            }
            ok(())
        }

        fn on_close(&mut self, _cli: &Client) -> Self::OnCloseFut {
            println!("on_close");
            ok(())
        }

        fn on_connect(&mut self, _cli: &Client) -> Self::OnConnectFut {
            println!("on_connect");
            ok(())
        }
    }

    let token = "xoxb-492475447088-515728907968-8tDDF4YTSMwRHRQQa8gIw43p";
    Client::login_and_run(token, MyHandler)
}