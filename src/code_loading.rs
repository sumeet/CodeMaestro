use std::cell::RefCell;
use std::fs::File;
use std::rc::Rc;

use super::builtins;
use super::lang;
use super::scripts;
use super::tests;

use crate::chat_trigger::ChatTrigger;
use crate::code_function::CodeFunction;
use crate::enums::Enum;
use crate::json_http_client::JSONHTTPClient;
use crate::lang::BuiltInTypeSpec;
use crate::structs::Struct;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::sync::{Arc, Mutex};

type Error = Box<std::error::Error>;

// TODO: find a better name. til then, we're gonna save the world
#[derive(Serialize, Deserialize, Debug)]
pub struct TheWorld {
    pub scripts: Vec<scripts::Script>,
    pub tests: Vec<tests::Test>,
    pub functions: Vec<Box<lang::Function>>,
    pub typespecs: Vec<Box<lang::TypeSpec>>,
}

// pub fn load(filename: &str) -> Result<CodeNode,Error> {
//     let f = File::open(filename)?;
//     Ok(serde_json::from_reader(f)?)
// }

pub fn save(filename: &str, world: &TheWorld) -> Result<(), Error> {
    let f = File::create(filename)?;
    Ok(serde_json::to_writer_pretty(f, &world)?)
}

// pub fn serialize(world: &TheWorld) -> Result<String,Error> {
//    Ok(serde_json::to_string_pretty(&world)?)
// }

pub fn deserialize(str: &str) -> Result<TheWorld, Error> {
    let deserialize_the_world = serde_json::from_str::<DeserializeTheWorld>(str)?;
    let functions = deserialize_the_world.functions
                                         .into_iter()
                                         .map(deserialize_fn)
                                         .collect::<Result<Vec<_>, Error>>()?;
    let typespecs = deserialize_the_world.typespecs
                                         .into_iter()
                                         .map(deserialize_typespec)
                                         .collect::<Result<Vec<_>, Error>>()?;
    Ok(TheWorld { scripts: deserialize_the_world.scripts,
                  tests: deserialize_the_world.tests,
                  functions,
                  typespecs })
}

pub fn deserialize_fn(value: serde_json::Value) -> Result<Box<lang::Function>, Error> {
    let typ = value.as_object()
                   .and_then(|obj| obj.get("type"))
                   .and_then(|typ| typ.as_str())
                   .ok_or_else(|| format!("couldn't decode funcs from {:?}", value))?;
    Ok(match typ {
        "ChatReply" => Box::new(builtins::ChatReply::new(Arc::new(Mutex::new(vec![])))),
        "Capitalize" => Box::new(builtins::Capitalize {}),
        "HTTPGet" => Box::new(builtins::HTTPGet {}),
        "JoinString" => Box::new(builtins::JoinString {}),
        "Print" => Box::new(builtins::Print {}),
        "JSONHTTPClient" => Box::new(serde_json::from_value::<JSONHTTPClient>(value)?),
        "ChatTrigger" => Box::new(serde_json::from_value::<ChatTrigger>(value)?),
        "CodeFunction" => Box::new(serde_json::from_value::<CodeFunction>(value)?),
        _ => panic!(format!("don't know how to load builtin func type {}", typ)),
    })
}

pub fn deserialize_typespec(value: serde_json::Value) -> Result<Box<lang::TypeSpec>, Error> {
    let typ = value.as_object()
                   .and_then(|obj| obj.get("type"))
                   .and_then(|typ| typ.as_str())
                   .ok_or_else(|| format!("couldn't decode typespecs from {:?}", value))?;
    Ok(match typ {
        "Struct" => Box::new(serde_json::from_value::<Struct>(value)?),
        "Enum" => Box::new(serde_json::from_value::<Enum>(value)?),
        "BuiltInTypeSpec" => Box::new(serde_json::from_value::<BuiltInTypeSpec>(value)?),
        _ => panic!(format!("don't know how to load typespec type {}", typ)),
    })
}

#[derive(Deserialize)]
struct DeserializeTheWorld {
    scripts: Vec<scripts::Script>,
    tests: Vec<tests::Test>,
    functions: Vec<serde_json::Value>,
    typespecs: Vec<serde_json::Value>,
}
