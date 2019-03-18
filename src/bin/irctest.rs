#![feature(await_macro, async_await, futures_api)]
#![feature(custom_attribute)]

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
use noob;
use gotham;

fn say_hello(state: gotham::state::State) -> (gotham::state::State, &'static str) {
    (state, "Hello world!")
}

async fn http_server() -> Result<(), ()> {
    let val : serde_json::Value = serde_json::from_str(r#"{"hoohaw": 123}"#).unwrap();
    await!(forward(insert_new_code(val)));
    await!(forward(gotham::init_server("0.0.0.0:9000", || Ok(say_hello)))).unwrap();
    Ok::<(), ()>(())
}

//pub type

fn main() {
    let getrekt_slack_token = "xoxb-492475447088-515728907968-8tDDF4YTSMwRHRQQa8gIw43p";
    let sandh_slack_token = "xoxb-562464349142-560290195488-MfjUZW4VTBYrDTO5wBzltnC6";
    let discord_bot_token = "NTQ5OTAyOTcwMzg5NzkwNzIx.D1auqw.QN0-mQBA4KmLZImlaRVwJHRsImQ";

    let chat_thingy = Rc::new(RefCell::new(ChatThingy::new()));

    let futures : Vec<Box<dyn OldFuture<Item = (), Error = ()>>> = vec![
        Box::new(backward(new_irc_conn(darwin_config(), Rc::clone(&chat_thingy)))),
        Box::new(backward(new_irc_conn(esper_config(), Rc::clone(&chat_thingy)))),
        Box::new(backward(slack(getrekt_slack_token, Rc::clone(&chat_thingy)))),
        Box::new(backward(slack(sandh_slack_token, Rc::clone(&chat_thingy)))),
        Box::new(backward(discord(discord_bot_token, Rc::clone(&chat_thingy)))),
        Box::new(backward(http_server())),
    ];

    let joined = join_all(futures);
    Runtime::new().unwrap().block_on(joined).unwrap();
}

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
        nickname: Some("ceeess".to_owned()),
        server: Some("irc.esper.net".to_owned()),
        channels: Some(vec!["#devnullzone".to_owned()]),
        port: Some(6667),
        ..Config::default()
    }
}

async fn discord(token: &'static str, chat_thingy: Rc<RefCell<ChatThingy>>) -> Result<(), ()> {
    let (client, mut stream) = await!(forward(noob::Client::connect(token))).unwrap_or_else(|e| {
        panic!("error connecting to discord: {:?}", e)
    });
    while let Some(event) = await!(stream.next()) {
        match event {
            Ok(noob::Event::MessageCreate(msg)) => {
                await!(chat_thingy.borrow_mut().message_received(msg.author.username, msg.content));

                for reply in chat_thingy.borrow_mut().reply_buffer.borrow_mut().drain(..) {
                    await!(forward(client.send_message(&noob::MessageBuilder::new(&reply), &msg.channel_id)))
                        .map_err(|e| println!("error sending discord message: {:?}", e)).ok();
                }
            }
            Err(e) => println!("there was a discord error: {:?}", e),
            _ => (),
        }
    }
    Ok::<(), ()>(())
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
                                msg_sender.send_message(&channel, &reply).map_err(|err| {
                                    println!("error sending slack message: {:?}", err)
                                }).ok();
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


// database shit
use futures_cpupool::CpuPool;
use diesel;
//use diesel::prelude::*;
use diesel::Insertable;
use diesel::r2d2;
use lazy_static::lazy_static;
use cs::schema::codes;
use diesel::query_dsl::RunQueryDsl;

pub type Conn = diesel::pg::PgConnection;
pub type Pool = r2d2::Pool<r2d2::ConnectionManager<Conn>>;

lazy_static! {
    static ref DIESEL_CONN_POOL : Pool = connect();
}

fn connect() -> Pool {
    let db_url = std::env::var("DATABASE_URL").expect("couldn't find DATABASE_URL");
    let manager = r2d2::ConnectionManager::<Conn>::new(db_url);
    r2d2::Pool::builder().build(manager).expect("Failed to create pool")
}

pub fn exec_async<T, E, F, R>(f: F) -> impl Future<Item = T, Error = E>
    where
        T: Send + 'static,
        E: From<r2d2::PoolError> + Send + 'static,
        F: FnOnce(&Conn) -> R + Send + 'static,
        R: IntoFuture<Item = T, Error = E> + Send + 'static,
        <R as IntoFuture>::Future: Send,
{
    lazy_static! {
      static ref THREAD_POOL: CpuPool = {
        CpuPool::new_num_cpus()
      };
    }

    let pool = DIESEL_CONN_POOL.clone();
    THREAD_POOL.spawn_fn(move || {
        pool
            .get()
            .map_err(|err| E::from(err))
            .map(|conn| f(&conn))
            .into_future()
            .and_then(|f| f)
    })
}

#[derive(Insertable)]
#[table_name="codes"]
struct NewCode {
    added_by: String,
    code: serde_json::Value,
    instance_id: i32,
}

fn insert_new_code(code: serde_json::Value) -> impl OldFuture {
    use cs::schema::codes::dsl::codes;
    let newcode = NewCode {
        added_by: "sumeet".to_string(),
        code,
        instance_id: 123,
    };
    exec_async(|conn| {
        diesel::insert_into(codes).values(newcode).get_result(conn)
    })
}
