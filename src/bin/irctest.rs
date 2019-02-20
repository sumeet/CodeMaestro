#![feature(await_macro, async_await, futures_api)]

extern crate cs;

use cs::asynk::{backward, forward, OldFuture};
use itertools::Itertools;
use maplit::hashmap;
use std::cell::RefCell;
use std::rc::Rc;
use cs::builtins::ChatReply;
use cs::builtins::new_message;
use cs::resolve_all_futures;
use cs::EnvGenie;
use cs::env;
use cs::lang;
use cs::lang::Function;
use std::marker::Unpin;
use tokio::prelude::*;
use irc::client::prelude::*;
use irc::client::PackedIrcClient;
use irc_proto::{Command};
use tokio::runtime::current_thread::Runtime;

struct ChatThingy {
    interp: env::Interpreter,
    reply_buffer: Rc<RefCell<Vec<String>>>,
}

impl ChatThingy {
    pub fn new() -> Self {
        let reply_buffer = Rc::new(RefCell::new(vec![]));
        let interp = cs::App::new().interpreter;
        let reply_function = ChatReply::new(Rc::clone(&reply_buffer));
        let reply_function_id = reply_function.id();
        interp.env.borrow_mut().add_function(reply_function);
        Self { interp, reply_buffer }
    }

    pub fn message_received(&self, sender: String, text: String) -> impl std::future::Future {
        let triggers = {
            let env = self.interp.env.borrow();
            let env_genie = EnvGenie::new(&env);
            env_genie.list_chat_triggers().cloned().collect_vec()
        };

        let mut triggered_values = vec![];
        let message = new_message(sender, text.clone());

        for chat_trigger in triggers {
            // TODO: this starts_with stuff is hax but i just wanna test it!!!
            if text.starts_with(&chat_trigger.prefix) {
                let message_arg_id = chat_trigger.takes_args()[0].id;
                triggered_values.push(chat_trigger.call(self.interp.dup(), hashmap! {
                    message_arg_id => message.clone(),
                }));
            }
        }
        async move {
            for value in triggered_values {
                println!("there's a triggered value d00d");
                await!(resolve_all_futures(value));
            }
            ()
        }
    }
}

fn main() {
    // i think this doesn't need to be rc refcell, i think we can just move it into the EventHandler
    // for slack... BUT whatever for now i'm gonna share it with irc
    let chat_thingy = Rc::new(RefCell::new(ChatThingy::new()));

    let config = Config {
        nickname: Some("cs".to_owned()),
        server: Some("irc.darwin.network".to_owned()),
        channels: Some(vec!["#darwin".to_owned()]),
        use_ssl: Some(true),
        password: Some("smellyoulater".to_string()),
        port: Some(6697),
        ..Config::default()
    };

    let mut runtime = Runtime::new().unwrap();
    let irc_client_future = IrcClient::new_future(config).unwrap();
    let PackedIrcClient(client, irc_future) = runtime.block_on(irc_client_future).unwrap();

    client.identify().unwrap();
    let slaq = slack(Rc::clone(&chat_thingy));

    let slack_future = backward(async move {
        await!(forward(slaq)).unwrap();
        Ok::<(), ()>(())
    });

    let thingy2 = Rc::clone(&chat_thingy);
    let irc_interaction_future = backward(async move {
        let mut stream = client.stream();
        while let Some(message) = await!(stream.next()) {
            if message.is_err() {
                println!("there was an error: {:?}", message)
            } else {
                let message = message.unwrap();
                println!("{:?}", message);
                if let Command::PRIVMSG(sender, text) = &message.command {
                    if let Some(response_target) = message.response_target() {
                        await!(thingy2.borrow_mut().message_received(sender.clone(), text.clone()));
                        for reply in chat_thingy.borrow_mut().reply_buffer.borrow_mut().drain(..) {
                            client.send_privmsg(response_target, &reply).map_err(|_err| {
                                ()
                            })?;
                        }
                    }
                }
            }
        }
        Ok::<(), ()>(())
    });

    let irc_future = backward(async move {
        await!(forward(irc_future)).unwrap();
        Ok::<(), ()>(())
    });

    runtime.block_on(slack_future.join(irc_future).join(irc_interaction_future)).unwrap();

}

fn slack(chat_thingy: Rc<RefCell<ChatThingy>>) -> impl OldFuture<Error = impl std::fmt::Debug> {
    use slack::{Event, Message};
    use slack::api::MessageStandard;
    use slack::future::client::{Client, EventHandler};
    use futures::future::{ok, FutureResult};

    struct MyHandler {
        chat_thingy: Rc<RefCell<ChatThingy>>,
    };

    impl EventHandler for MyHandler {
        type EventFut = Box<OldFuture<Item = (), Error = ()>>;
        type OnCloseFut = FutureResult<(), ()>;
        type OnConnectFut = FutureResult<(), ()>;

        fn on_event(&mut self, cli: &Client, event: Event) -> Self::EventFut {
            // print out the event
            // do something if it's a `Message::Standard`
            if let Event::Message(ref message) = event {
                if let Message::Standard(MessageStandard {
                                             ref channel,
                                             ref user,
                                             ref text,
                                             ts: _,
                                             ..
                                         }) = **message {
                    if let (Some(channel), Some(text), Some(user)) = (channel, text, user) {
                        let msg_sender = cli.sender().clone();
                        let sender = user.clone();
                        let text = text.clone();
                        let channel = channel.clone();

                        let chat_thingy = Rc::clone(&self.chat_thingy);
                        return Box::new(backward(async move {

                            await!(chat_thingy.borrow_mut().message_received(sender, text));

                            for reply in chat_thingy.borrow_mut().reply_buffer.borrow_mut().drain(..) {
                                msg_sender.send_message(&channel, &reply).map_err(|_err| {
                                    ()
                                })?;
                            }
                            Ok(())
                        }))
                    }
                }
            }
            Box::new(ok(()))
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
    Client::login_and_run(token, MyHandler { chat_thingy })
}

