extern crate cs;

use std::pin::Pin;

use cs::asynk::{backward, forward, OldFuture};
use cs::builtins::ChatReply;
use cs::code_loading::TheWorld;
use cs::env;
use diesel::prelude::*;
use diesel::query_dsl::QueryDsl;
use futures::future::join_all;
use futures_channel::mpsc;
use futures_util::compat::Stream01CompatExt;
use hyper::service::service_fn;
use hyper::{Body, Request, Response, Server};
use irc::client::prelude::*;
use irc::client::PackedIrcClient;
use irc_proto::Command;
use itertools::Itertools;
use noob;
use serde::Deserialize;
use serde_derive::{Deserialize as Deserializeable, Serialize as Serializeable};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::thread;
use tokio::prelude::*;
use tokio::runtime::current_thread::Runtime;

fn main() {
    use std::fs::File;
    use std::io::BufReader;

    let _guard =
        sentry::init("https://d8b23e4125944accaee73d0aa4663f0d@o407424.ingest.sentry.io/5276333");

    let mut runtime = Runtime::new().unwrap();

    // args for running administrative tasks
    let mut args = std::env::args();
    let main_arg = args.nth(1);
    if main_arg == Some("load_service_configs".to_string()) {
        let filename = args.next().expect("expected a filename");
        let file = File::open(filename).unwrap();
        let configs: Vec<NewServiceConfig> = serde_json::from_reader(BufReader::new(file)).unwrap();
        runtime.block_on(insert_new_service_configs(configs))
               .unwrap();
        std::process::exit(0);
    }

    let service_configs_by_instance_id = runtime.block_on(backward(load_instances())).unwrap();

    if main_arg == Some("list_instance_urls".to_string()) {
        for (instance_id, service_configs) in service_configs_by_instance_id.iter() {
            let url = GenerateProgramBotUrl::new(*instance_id).generate_url()
                                                              .unwrap();
            let nicknames = service_configs.iter().map(|sc| &sc.nickname).join(", ");
            println!("instance {}: {}", instance_id, nicknames);
            println!("{}", url);
            println!();
        }
        std::process::exit(0);
    }

    let mut new_code_sender_by_instance_id = HashMap::new();

    let mut threads = service_configs_by_instance_id.into_iter().map(|(instance_id, service_configs)| {
        // GHETTO: this is for sending worlds from the web interface into the interp
        let (tx, rx) = mpsc::unbounded::<TheWorld>();
        new_code_sender_by_instance_id.insert(instance_id, tx);
        thread::spawn(move || {
            start_new_interpreter_instance_with_services(instance_id, &service_configs, rx);
        })
    }).collect_vec();

    threads.push(thread::spawn(move || {
                     let mut runtime = Runtime::new().unwrap();
                     runtime.block_on(backward(http_server(new_code_sender_by_instance_id)))
                            .unwrap();
                 }));

    for thread in threads {
        thread.join().unwrap();
    }
}

fn start_new_interpreter_instance_with_services(instance_id: i32,
                                                service_configs: &[ServiceConfig],
                                                new_code_receiver: mpsc::UnboundedReceiver<TheWorld>)
{
    let mut runtime = Runtime::new().unwrap();
    let chat_thingy = Rc::new(RefCell::new(ChatThingy::new(instance_id)));

    runtime.block_on(backward(load_code_from_the_db_into(Rc::clone(&chat_thingy), instance_id)))
           .unwrap();
    let mut futures = service_configs.iter()
                                     .map(|service_config| {
                                         let b: Box<dyn OldFuture<Item = (), Error = ()>> =
                                             Box::new(backward(new_conn(service_config,
                                                                        Rc::clone(&chat_thingy))));
                                         b
                                     })
                                     .collect_vec();
    futures.push(Box::new(backward(receive_code(chat_thingy, new_code_receiver))));
    let joined = join_all(futures);
    runtime.block_on(joined).unwrap();
}

async fn receive_code(chat_thingy: Rc<RefCell<ChatThingy>>,
                      mut rx: mpsc::UnboundedReceiver<TheWorld>)
                      -> Result<(), ()> {
    while let Some(world) = rx.next().await {
        chat_thingy.borrow_mut().load_world(&world);
    }
    Ok::<(), ()>(())
}

async fn new_conn(service_config: &ServiceConfig,
                  chat_thingy: Rc<RefCell<ChatThingy>>)
                  -> Result<(), ()> {
    match service_config.service_type.as_str() {
        "irc" => {
            println!("new irc conn");
            new_irc_conn(service_config.irc_config().unwrap(), chat_thingy).await
        }
        "discord" => {
            println!("new discord conn");
            new_discord_conn(service_config.discord_token().unwrap(), chat_thingy).await
        }
        "slack" => {
            println!("new slack conn");
            new_slack_conn(service_config.slack_token().unwrap(), chat_thingy).await
        }
        _ => panic!("unknown service type: {}", service_config.service_type),
    }
}

#[derive(Serializeable, Deserializeable, Clone)]
struct GenerateProgramBotUrl {
    instance_id: i32,
}

impl GenerateProgramBotUrl {
    fn new(instance_id: i32) -> Self {
        Self { instance_id }
    }

    fn generate_url(&self) -> Result<url::Url, Box<dyn std::error::Error>> {
        let token = NewCodeIntent::token(self.instance_id)?;
        Ok(config::edit_code_url(&token)?)
    }
}

struct ChatThingy {
    interp: env::Interpreter,
    reply_buffer: Arc<Mutex<Vec<String>>>,
    instance_id: i32,
}

impl ChatThingy {
    pub fn new(instance_id: i32) -> Self {
        let reply_buffer = Arc::new(Mutex::new(vec![]));
        let interp = cs::init_interpreter();

        let reply_function = ChatReply::new(Arc::clone(&reply_buffer));
        interp.env.borrow_mut().add_function(reply_function);

        Self { interp,
               reply_buffer,
               instance_id }
    }

    // TODO: this is duped from lib.rs
    pub fn load_world(&self, world: &TheWorld) {
        let mut env = self.interp.env.borrow_mut();
        for function in &world.functions {
            env.add_function_box(function.clone());
        }
        for typespec in &world.typespecs {
            env.add_typespec_box(typespec.clone());
        }
    }

    pub fn message_received(&self,
                            sender: String,
                            text: String)
                            -> Pin<Box<dyn std::future::Future<Output = ()>>> {
        if text == ".letmeprogramyou" {
            let program_url = GenerateProgramBotUrl::new(self.instance_id).generate_url()
                                                                          .unwrap();
            self.reply_buffer
                .lock()
                .unwrap()
                .push(program_url.to_string());
            return Box::pin(async { () });
        }

        message_received(&self.interp, sender, text)
    }
}

async fn new_irc_conn(mut config: Config, chat_thingy: Rc<RefCell<ChatThingy>>) -> Result<(), ()> {
    config.version = Some("cs: program me!".to_string());
    config.alt_nicks =
        Some((1..6).map(|n| {
                       let underscores = std::iter::repeat("_").take(n).join("");
                       format!("{}{}", config.nickname.as_ref().unwrap(), underscores)
                   })
                   .collect());
    let irc_client_future = IrcClient::new_future(config).unwrap();
    let PackedIrcClient(client, irc_future) = forward(irc_client_future).await.unwrap();

    // must send cap_req before identify because identify sends cap_end
    client.send_cap_req(&[Capability::Custom("twitch.tv/membership")])
          .unwrap();
    client.identify().unwrap();
    let irc_future = backward(async move {
        forward(irc_future).await
                           .map_err(|e| println!("irc error: {:?}", e))
                           .ok();
        Ok::<(), ()>(())
    });

    forward(backward(irc_interaction_future(client, chat_thingy)).join(irc_future)).await
                                                                                   .ok();
    Ok::<(), ()>(())
}

async fn irc_interaction_future(client: IrcClient,
                                chat_thingy: Rc<RefCell<ChatThingy>>)
                                -> Result<(), ()> {
    let mut stream = client.stream().compat();
    while let Some(message) = stream.next().await {
        if message.is_err() {
            println!("there was an error: {:?}", message)
        } else {
            let message = message.unwrap();
            println!("{:?}", message);
            if let Command::PRIVMSG(sender, text) = &message.command {
                if let Some(response_target) = message.response_target() {
                    chat_thingy.borrow_mut()
                               .message_received(sender.clone(), text.clone())
                               .await;
                    for reply in chat_thingy.borrow_mut()
                                            .reply_buffer
                                            .lock()
                                            .unwrap()
                                            .drain(..)
                    {
                        client.send_privmsg(response_target, &reply)
                              .map_err(|err| println!("error sending msg: {:?}", err))
                              .ok();
                    }
                }
            }
        }
    }
    Ok::<(), ()>(())
}

async fn new_discord_conn(token: &str, chat_thingy: Rc<RefCell<ChatThingy>>) -> Result<(), ()> {
    'outer: loop {
        let (client, stream) =
            forward(noob::Client::connect(token)).await
                                                 .unwrap_or_else(|e| {
                                                     panic!("error connecting to discord: {:?}", e)
                                                 });
        let mut stream = stream.compat();
        while let Some(event) = stream.next().await {
            match event {
                Ok(noob::Event::MessageCreate(msg)) => {
                    chat_thingy.borrow_mut()
                               .message_received(msg.author.username, msg.content)
                               .await;

                    for reply in chat_thingy.borrow_mut()
                                            .reply_buffer
                                            .lock()
                                            .unwrap()
                                            .drain(..)
                    {
                        forward(client.send_message(&noob::MessageBuilder::new(&reply), &msg.channel_id)).await
                            .map_err(|e| println!("error sending discord message: {:?}", e)).ok();
                    }
                }
                Err(e) => {
                    println!("there was a discord error, and we must reconnect: {:?}", e);
                    forward(tokio_timer::sleep(Duration::from_secs(1))).await
                                                                       .unwrap();
                    continue 'outer;
                }
                _ => (),
            }
        }
    }
    //Ok::<(), ()>(())
}

async fn new_slack_conn(token: &str, chat_thingy: Rc<RefCell<ChatThingy>>) -> Result<(), ()> {
    use futures::future::{ok, FutureResult};
    use slack::api::MessageStandard;
    use slack::future::client::{Client, EventHandler};
    use slack::{Event, Message};

    struct MyHandler {
        chat_thingy: Rc<RefCell<ChatThingy>>,
    };

    impl EventHandler for MyHandler {
        type EventFut = Box<dyn OldFuture<Item = (), Error = ()>>;
        type OnCloseFut = FutureResult<(), ()>;
        type OnConnectFut = FutureResult<(), ()>;

        fn on_event(&mut self, cli: &Client, event: Event) -> Self::EventFut {
            // print out the event
            // do something if it's a `Message::Standard`
            if let Event::Message(ref message) = event {
                if let Message::Standard(MessageStandard { ref channel,
                                                           ref user,
                                                           ref text,
                                                           ts: _,
                                                           .. }) = **message
                {
                    if let (Some(channel), Some(text), Some(user)) = (channel, text, user) {
                        let msg_sender = cli.sender().clone();
                        let sender = user.clone();
                        let text = text.clone();
                        let channel = channel.clone();

                        let chat_thingy = Rc::clone(&self.chat_thingy);
                        return Box::new(backward(async move {
                                            chat_thingy.borrow_mut()
                                                       .message_received(sender, text)
                                                       .await;

                                            for reply in chat_thingy.borrow_mut()
                                                                    .reply_buffer
                                                                    .lock()
                                                                    .unwrap()
                                                                    .drain(..)
                                            {
                                                msg_sender.send_message(&channel, &reply)
                                          .map_err(|err| {
                                              println!("error sending slack message: {:?}", err)
                                          })
                                          .ok();
                                            }
                                            Ok(())
                                        }));
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

    forward(Client::login_and_run(token, MyHandler { chat_thingy })).await
        .map_err(|e| println!("slack error: {:?}", e)).ok();
    Ok::<(), ()>(())
}

// database shit
use diesel;
use futures_cpupool::CpuPool;
//use diesel::prelude::*;
use cs::config;
use cs::schema::{codes, service_configs};
use diesel::query_dsl::RunQueryDsl;
use diesel::r2d2;
use diesel::{Insertable, Queryable};
use lazy_static::lazy_static;

pub type Conn = diesel::pg::PgConnection;
pub type Pool = r2d2::Pool<r2d2::ConnectionManager<Conn>>;

lazy_static! {
    static ref DIESEL_CONN_POOL: Pool = connect();
}

fn connect() -> Pool {
    let db_url = config::get("DATABASE_URL").expect("couldn't find DATABASE_URL");
    let manager = r2d2::ConnectionManager::<Conn>::new(db_url);
    r2d2::Pool::builder().build(manager)
                         .expect("Failed to create pool")
}

pub fn exec_async<T, E, F, R>(f: F) -> impl Future<Item = T, Error = E>
    where T: Send + 'static,
          E: std::error::Error + Send + 'static,
          F: FnOnce(&Conn) -> R + Send + 'static,
          R: IntoFuture<Item = T, Error = E> + Send + 'static,
          <R as IntoFuture>::Future: Send
{
    lazy_static! {
        static ref THREAD_POOL: CpuPool = CpuPool::new_num_cpus();
    }

    let pool = DIESEL_CONN_POOL.clone();
    THREAD_POOL.spawn_fn(move || {
                   pool.get()
                       // TODO: this is still super fucked. we'll crash any time the insert fails lol
                       .map_err(|_err| panic!("ugh fuck this thing"))
                       .map(|conn| f(&conn))
                       .into_future()
                       .and_then(|f| f)
               })
}

use branca::Branca;
use cs::chat_program::message_received;
use futures_util::stream::StreamExt;
use http_fs::config::StaticFileConfig;
use http_fs::StaticFiles;
use hyper::service::Service;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
pub struct DirectoryConfig;
impl StaticFileConfig for DirectoryConfig {
    type FileService = http_fs::config::DefaultConfig;
    type DirService = http_fs::config::DefaultConfig;

    fn handle_directory(&self, _path: &Path) -> bool {
        false
    }

    fn index_file(&self, _path: &Path) -> Option<&Path> {
        Some(Path::new("index.html"))
    }

    fn serve_dir(&self) -> &Path {
        Path::new("static/")
    }
}

pub fn serve_static(
    req: Request<Body>)
    -> impl std::future::Future<Output = Result<Response<Body>, std::io::Error>> {
    let mut static_files = StaticFiles::new(DirectoryConfig);
    forward(static_files.call(req))
}

#[derive(Insertable)]
#[table_name = "codes"]
struct NewCode {
    added_by: String,
    code: serde_json::Value,
    instance_id: i32,
}

#[derive(Queryable)]
// for some reason, Queryable requires that we have all DB fields even if we don't use them
#[allow(dead_code)]
struct Code {
    id: i32,
    added_by: String,
    code: serde_json::Value,
    instance_id: i32,
    created_at: std::time::SystemTime,
    updated_at: std::time::SystemTime,
}

#[derive(Insertable, Serializeable, Deserializeable)]
#[table_name = "service_configs"]
struct NewServiceConfig {
    instance_id: i32,
    nickname: String,
    service_type: String,
    config: serde_json::Value,
}

#[derive(Queryable, Debug)]
// for some reason, Queryable requires that we have all DB fields even if we don't use them
#[allow(dead_code)]
struct ServiceConfig {
    id: i32,
    nickname: String,
    instance_id: i32,
    service_type: String,
    created_at: std::time::SystemTime,
    updated_at: std::time::SystemTime,
    config: serde_json::Value,
}

impl ServiceConfig {
    pub fn discord_token(&self) -> Result<&str, Box<dyn std::error::Error>> {
        Ok(self.config
               .get("token")
               .ok_or("discord token not found in config")?
               .as_str()
               .ok_or("discord token not a string")?)
    }

    pub fn slack_token(&self) -> Result<&str, Box<dyn std::error::Error>> {
        Ok(self.config
               .get("token")
               .ok_or("slack token not found in config")?
               .as_str()
               .ok_or("slack token not a string")?)
    }

    pub fn irc_config(&self) -> Result<Config, Box<dyn std::error::Error>> {
        Ok(serde_json::from_value(self.config.clone())?)
    }
}

fn insert_new_code(
    code: &TheWorld,
    instance_id: i32)
    -> impl OldFuture<Error = impl std::error::Error + std::fmt::Debug + 'static> {
    use cs::schema::codes::dsl::codes;
    let newcode = NewCode { added_by: "sumeet".to_string(),
                            code: serde_json::to_value(code).unwrap(),
                            instance_id };
    exec_async(|conn| diesel::insert_into(codes).values(newcode).execute(conn))
}

fn insert_new_service_configs(
    configs: Vec<NewServiceConfig>)
    -> impl OldFuture<Error = impl std::error::Error + std::fmt::Debug + 'static> {
    use cs::schema::service_configs::dsl::service_configs;
    exec_async(move |conn| {
        diesel::insert_into(service_configs).values(configs)
                                            .execute(conn)
    })
}

async fn load_instances() -> Result<HashMap<i32, Vec<ServiceConfig>>, Box<dyn std::error::Error>> {
    use crate::service_configs::dsl::*;

    let all_service_configs =
        forward(exec_async(|conn| service_configs.load::<ServiceConfig>(conn))).await?;

    Ok(all_service_configs.into_iter()
                          .map(|service_config| (service_config.instance_id, service_config))
                          .into_group_map())
}

async fn load_code_from_the_db_into(chat_thingy: Rc<RefCell<ChatThingy>>,
                                    for_instance_id: i32)
                                    -> Result<(), ()> {
    use crate::codes::dsl::*;

    let code_rows = forward(exec_async(move |conn| {
                                codes.filter(instance_id.eq(for_instance_id))
                                     .load::<Code>(conn)
                            })).await
                               .unwrap();
    for code_row in code_rows {
        let the_world = serde_json::from_value(code_row.code);
        match the_world {
            Ok(ref the_world) => {
                println!("loading smth from the world");
                chat_thingy.borrow().load_world(the_world);
            }
            Err(e) => println!("error deserializing world: {:?}", e),
        }
    }
    println!("done loading from db");
    Ok::<(), ()>(())
}

async fn http_server(new_code_sender_by_instance_id: HashMap<i32,
                             mpsc::UnboundedSender<TheWorld>>)
                     -> Result<(), ()> {
    let port = config::get("PORT").expect("PORT envvar not set")
                                  .parse()
                                  .expect("PORT must be an integer");
    forward(Server::bind(&([0, 0, 0, 0], port).into())
        .executor(tokio::runtime::current_thread::TaskExecutor::current())
        .serve(move || service_fn(http_handler(new_code_sender_by_instance_id.clone())))).await.unwrap();
    Ok::<(), ()>(())
}

async fn deserialize<T>(req: Request<Body>)
                        -> Result<Request<T>, Box<dyn std::error::Error + 'static>>
    where for<'de> T: Deserialize<'de>
{
    let (parts, body) = req.into_parts();
    let body = forward(body.concat2()).await?;
    println!("{}", std::str::from_utf8(&body).as_ref().unwrap());
    let body = serde_json::from_slice(&body)?;
    Ok(Request::from_parts(parts, body))
}

fn http_handler(
    new_code_sender_by_instance_id: HashMap<i32, mpsc::UnboundedSender<TheWorld>>)
    -> impl Fn(Request<Body>) -> Box<dyn OldFuture<Item = Response<Body>, Error = hyper::Error>> {
    move |request| {
        let uri = request.uri();
        let new_code_intent = extract_intent(uri);
        let new_code_sender =
            new_code_intent.as_ref().and_then(|intent| {
                                        new_code_sender_by_instance_id.get(&intent.instance_id)
                                    });
        if uri.path() == "/postthecode" && new_code_sender.is_some() {
            let mut new_code_sender = new_code_sender.unwrap().clone();
            let new_code_intent = new_code_intent.unwrap();
            Box::new(backward(async move {
                         let body = deserialize::<TheWorld>(request).await;
                         if let Err(e) = body {
                             println!("error: {:?}", e);
                             return Ok(validation_error("ur world sucked"));
                         }

                         let the_world = body.unwrap().into_body();
                         forward(insert_new_code(&the_world, new_code_intent.instance_id)).await
                                                                                          .unwrap();

                         use futures_util::sink::SinkExt;
                         new_code_sender.send(the_world).await.unwrap();

                         Ok(Response::new(Body::from("던지다")))
                     }))
        } else {
            Box::new(backward(async move {
                         // oh jesus christ, the unimplemented
                         serve_static(request).await.map_err(|e| {
                                                        println!("{:?}", e);
                                                        unimplemented!()
                                                    })
                     }))
        }
    }
}

fn extract_intent(uri: &http::Uri) -> Option<NewCodeIntent> {
    Some(NewCodeIntent::decode(uri.query()?).ok()?)
}

fn validation_error(str: &'static str) -> Response<Body> {
    Response::builder().status(400).body(str.into()).unwrap()
}

#[derive(Serializeable, Deserializeable)]
struct NewCodeIntent {
    instance_id: i32,
}

impl NewCodeIntent {
    fn token(instance_id: i32) -> Result<String, Box<dyn std::error::Error>> {
        Ok(Self { instance_id }.encode()?)
    }
}

lazy_static! {
    static ref SIGNING_TOKEN: Branca = {
        let signing_secret = config::get("SIGNING_SECRET").expect("SIGNING_SECRET");
        Branca::new(signing_secret.as_bytes()).unwrap()
    };
}

impl NewCodeIntent {
    fn encode(&self) -> Result<String, Box<dyn std::error::Error>> {
        Ok(SIGNING_TOKEN.encode(&serde_json::to_string(self)?)?)
    }

    fn decode(str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(&SIGNING_TOKEN.decode(str, 0)?)?)
    }
}
