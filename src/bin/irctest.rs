#![feature(await_macro, async_await, futures_api)]

extern crate cs;

use cs::asynk::{backward, forward, OldFuture};
use itertools::Itertools;
use std::cell::RefCell;
use std::rc::Rc;
use cs::builtins::ChatReply;
use cs::resolve_all_futures;
use cs::EnvGenie;
use cs::env;
use tokio::prelude::*;
use irc::client::prelude::*;
use irc::client::PackedIrcClient;
use irc_proto::{Command};
use tokio::runtime::current_thread::Runtime;
use futures::future::join_all;

struct ChatThingy {
    interp: env::Interpreter,
    reply_buffer: Rc<RefCell<Vec<String>>>,
}

impl ChatThingy {
    pub fn new() -> Self {
        let reply_buffer = Rc::new(RefCell::new(vec![]));
        let interp = cs::App::new().interpreter;
        let reply_function = ChatReply::new(Rc::clone(&reply_buffer));
        interp.env.borrow_mut().add_function(reply_function);
        Self { interp, reply_buffer }
    }

    pub fn message_received(&self, sender: String, text: String) -> impl std::future::Future {
        let triggers = {
            let env = self.interp.env.borrow();
            let env_genie = EnvGenie::new(&env);
            env_genie.list_chat_triggers().cloned().collect_vec()
        };

        let triggered_values = triggers.iter()
            .filter_map(|ct| {
                ct.try_to_trigger(self.interp.dup(), sender.clone(),
                                  text.clone())
            })
            .collect_vec();

        async move {
            for value in triggered_values {
                println!("there's a triggered value d00d");
                await!(resolve_all_futures(value));
            }
            ()
        }
    }
}


async fn new_irc_conn(mut config: Config, chat_thingy: Rc<RefCell<ChatThingy>>) -> Result<(), ()> {
    config.version = Some("cs: program me!".to_string());
    let irc_client_future = IrcClient::new_future(config).unwrap();
    let PackedIrcClient(client,
                        irc_future) = await!(forward(irc_client_future)).unwrap();
    client.identify().unwrap();
    let irc_future = backward(async move {
        await!(forward(irc_future)).map_err(|e| println!("irc error: {:?}", e)).ok();
        Ok::<(), ()>(())
    });
    await!(forward(backward(irc_interaction_future(client, chat_thingy)).join(irc_future))).ok();
    Ok::<(), ()>(())
}

async fn irc_interaction_future(client: IrcClient, chat_thingy: Rc<RefCell<ChatThingy>>) -> Result<(), ()> {
    let mut stream = client.stream();
    while let Some(message) = await!(stream.next()) {
        if message.is_err() {
            println!("there was an error: {:?}", message)
        } else {
            let message = message.unwrap();
            println!("{:?}", message);
            if let Command::PRIVMSG(sender, text) = &message.command {
                if let Some(response_target) = message.response_target() {
                    await!(chat_thingy.borrow_mut().message_received(sender.clone(), text.clone()));
                    for reply in chat_thingy.borrow_mut().reply_buffer.borrow_mut().drain(..) {
                        client.send_privmsg(response_target, &reply).map_err(|err| {
                            println!("error sending msg: {:?}", err)
                        }).ok();
                    }
                }
            }
        }
    }
    Ok::<(), ()>(())
}

fn darwin_config() -> Config {
   Config {
        nickname: Some("cs".to_owned()),
        server: Some("irc.darwin.network".to_owned()),
        channels: Some(vec!["#darwin".to_owned()]),
        use_ssl: Some(true),
        password: Some("smellyoulater".to_string()),
        port: Some(6697),
        ..Config::default()
    }
}

fn esper_config() -> Config {
    Config {
        nickname: Some("cs".to_owned()),
        server: Some("irc.esper.net".to_owned()),
        channels: Some(vec!["#devnullzone".to_owned()]),
        port: Some(6667),
        ..Config::default()
    }
}

fn main() {
    let getrekt_slack_token = "xoxb-492475447088-515728907968-8tDDF4YTSMwRHRQQa8gIw43p";

    let chat_thingy = Rc::new(RefCell::new(ChatThingy::new()));

    let futures : Vec<Box<dyn OldFuture<Item = (), Error = ()>>> = vec![
        Box::new(backward(new_irc_conn(darwin_config(), Rc::clone(&chat_thingy)))),
        Box::new(backward(new_irc_conn(esper_config(), Rc::clone(&chat_thingy)))),
        Box::new(backward(slack(getrekt_slack_token, Rc::clone(&chat_thingy)))),
    ];

    let joined = join_all(futures);
    Runtime::new().unwrap().block_on(joined).unwrap();
}


async fn slack(token: &'static str, chat_thingy: Rc<RefCell<ChatThingy>>) -> Result<(), ()> {
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

    await!(forward(Client::login_and_run(token, MyHandler { chat_thingy })))
        .map_err(|e| println!("slack error: {:?}", e)).ok();
    Ok::<(), ()>(())
}

