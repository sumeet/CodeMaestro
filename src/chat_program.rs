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
use crate::lang::Function;
use std::pin::Pin;
use crate::resolve_all_futures;
use itertools::Itertools;
use crate::env::Interpreter;

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
        for (id, value) in args.iter() {
            interpreter.env
                       .borrow_mut()
                       .set_local_variable(*id, value.clone());
        }

        lang::Value::new_future(interpreter.evaluate(&lang::CodeNode::Block(self.code.clone())))
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
}

pub fn message_received(chat_programs: &[ChatProgram], interp: &Interpreter, sender: String, text: String) -> Pin<Box<dyn std::future::Future<Output = ()>>> {
    let triggered_values = chat_programs.iter()
        .filter_map(|cp| {
            println!("{:?}", cp);
            cp.try_to_trigger(interp.dup(), sender.clone(), text.clone())
        }).collect_vec();

    Box::pin(async move {
        for value in triggered_values {
            println!("there's a triggered value d00d");
            resolve_all_futures(value).await;
        }
    })
}
