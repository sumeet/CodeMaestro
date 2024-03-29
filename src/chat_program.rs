use lazy_static::lazy_static;
use maplit::hashmap;
use regex;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid;

use super::builtins::MESSAGE_STRUCT_ID;
use super::env;
use super::lang;
use crate::builtins::new_message;
use crate::env::Interpreter;
use crate::lang::Function;
use crate::{builtins, resolve_all_futures, validation, EnvGenie};
use itertools::Itertools;
use std::pin::Pin;

lazy_static! {
    static ref MESSAGE_ARG_ID: lang::ID =
        uuid::Uuid::parse_str("159dc4f3-3f37-44da-b979-d4a41a9273cf").unwrap();
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ChatProgram {
    pub id: lang::ID,
    pub prefix: String,
    pub code: lang::Block,
}

impl ChatProgram {
    pub fn new() -> Self {
        Self { id: lang::new_id(),
               prefix: "".to_string(),
               code: lang::Block::new() }
    }

    // case insensitive matches the prefix and cuts it off the beginning.
    //
    // for example:
    // ".wz" => ""
    // ".wz sf" => "sf"
    fn strip_prefix<'a>(&'a self, text: &'a str) -> String {
        self.prefix_re().replace_all(text, "").trim().into()
    }

    fn prefix_re(&self) -> regex::Regex {
        let regex_str = format!(r"^(?i){}(?:\b+|$)", regex::escape(&self.prefix));
        regex::Regex::new(&regex_str).unwrap()
    }

    pub fn try_to_trigger(&self,
                          interpreter: env::Interpreter,
                          sender: String,
                          message_text: String)
                          -> Option<lang::Value> {
        if !self.prefix_re().is_match(&message_text) {
            return None;
        }

        let can_be_run = {
            let env = interpreter.env.borrow();
            let env_genie = EnvGenie::new(&env);
            validation::can_be_run(self, &env_genie)
        };
        if !can_be_run {
            // TODO: send a message here saying that the code had an issue with it, and would've
            // otherwise run
            println!("would run this chat program, but deciding not to because it would crash");
            let env = interpreter.env.borrow();
            let env_genie = EnvGenie::new(&env);
            append_to_chat_buffer(&env_genie, "The code you're trying to run has some issues and cannot be run. Please get in touch with the author or the administrator.".to_owned());
            return None;
        }

        let argument_text = self.strip_prefix(&message_text);
        let message_struct = new_message(sender, argument_text, message_text);
        Some(self.call(interpreter, hashmap! {*MESSAGE_ARG_ID => message_struct}))
    }
}

// the only reason this is a function is so other functions can call chat triggers.... idk if we really
// need that functionality but it's how things ended up and i'm too lazy to change it rn
#[typetag::serde]
impl lang::Function for ChatProgram {
    fn call(&self,
            mut interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        // XXX: shouldn't the caller do this???? duped with CodeFunction
        for (id, value) in args {
            interpreter.set_local_variable(id, value);
        }

        let code = self.code.clone();
        lang::Value::new_future(async move { interpreter.evaluate(&lang::CodeNode::Block(code)).await })
    }

    fn name(&self) -> &str {
        &self.prefix
    }

    fn description(&self) -> &str {
        "Programatically triger this"
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![lang::ArgumentDefinition { id: *MESSAGE_ARG_ID,
                                        arg_type: lang::Type::from_spec_id(*MESSAGE_STRUCT_ID,
                                                                           vec![]),
                                        short_name: "Message".to_string() }]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NULL_TYPESPEC)
    }

    fn cs_code(&self) -> Box<dyn Iterator<Item = &lang::Block> + '_> {
        let x: Box<dyn Iterator<Item = &lang::Block>> = Box::new(std::iter::once(&self.code));
        x
    }
}

pub fn message_received(interp: &Interpreter,
                        sender: String,
                        text: String)
                        -> Pin<Box<dyn std::future::Future<Output = ()>>> {
    // this is mandatory or else we'll borrow the Env for too long
    // TODO: make this a better comment
    // BorrowMutError
    let chat_programs = {
        let env = interp.env.borrow();
        let env_genie = EnvGenie::new(&env);
        env_genie.list_chat_programs().cloned().collect::<Vec<_>>()
    };
    let triggered_values =
        chat_programs.iter()
                     .filter_map(|cp| {
                         cp.try_to_trigger(interp.new_stack_frame(), sender.clone(), text.clone())
                     })
                     .collect_vec();

    Box::pin(async move {
        for value in triggered_values {
            resolve_all_futures(value).await;
        }
    })
}

fn append_to_chat_buffer(env_genie: &EnvGenie, reply: String) {
    let chat_reply = env_genie.find_function(*builtins::CHAT_REPLY_FUNC_ID)
                              .unwrap()
                              .downcast_ref::<builtins::ChatReply>()
                              .unwrap();
    chat_reply.output_buffer.lock().unwrap().push(reply);
}

pub fn flush_reply_buffer(env_genie: &EnvGenie) -> Vec<String> {
    let chat_reply = env_genie.find_function(*builtins::CHAT_REPLY_FUNC_ID)
                              .unwrap()
                              .downcast_ref::<builtins::ChatReply>()
                              .unwrap();

    let mut output_buffer = vec![];
    std::mem::swap(&mut output_buffer,
                   &mut chat_reply.output_buffer.lock().unwrap());
    output_buffer
}
