#![feature(trait_alias)]
#![feature(unboxed_closures)]
#![feature(specialization)]
#![feature(nll)]
#![feature(arbitrary_self_types)]
#![feature(slice_concat_ext)]
#![feature(box_patterns)]
#![feature(drain_filter)]
#![feature(generators)]
#![recursion_limit = "256"]

pub mod asynk;
pub mod builtins;
pub mod enums;
pub mod lang;
pub mod structs;
#[macro_use]
pub mod env;
mod click_handling;
pub mod code_loading;
pub mod config;
pub mod env_genie;
pub mod http_request;
pub mod json_http_client;
mod result;
pub mod validation;
#[cfg(not(target_arch = "wasm32"))]
#[macro_use]
extern crate diesel;
pub mod chat_program;
pub mod code_function;
pub mod external_func;
pub mod function;
#[cfg(feature = "python")]
pub mod pystuff;
#[cfg(not(target_arch = "wasm32"))]
pub mod schema;
pub mod scripts;
pub mod tests;

#[cfg(not(feature = "python"))]
mod fakepystuff;

#[cfg(not(feature = "python"))]
pub mod pystuff {
    pub use super::fakepystuff::*;
}

#[cfg(target_arch = "wasm32")]
pub mod jsstuff;

#[cfg(not(target_arch = "wasm32"))]
mod fakejsstuff;

#[cfg(not(target_arch = "wasm32"))]
pub mod jsstuff {
    pub use super::fakejsstuff::*;
}

pub mod http_client;
#[cfg(not(target_arch = "wasm32"))]
mod native_http_client;
#[cfg(target_arch = "wasm32")]
mod wasm_http_client;
pub use env_genie::EnvGenie;
pub use external_func::resolve_all_futures;

use self::env::ExecutionEnvironment;

use std::sync::{Arc, Mutex};

// TODO: builtins loaded twice from disk, once here, once in init_controller.
pub fn init_interpreter() -> env::Interpreter {
    let interpreter = env::Interpreter::new();
    let builtins = builtins::Builtins::load().unwrap();
    load_builtins(builtins, &mut interpreter.env.borrow_mut());
    interpreter
}

fn load_builtins(builtins: builtins::Builtins, env: &mut ExecutionEnvironment) {
    for func in builtins.funcs.values() {
        env.add_function_box(func.clone());
    }
    for ts in builtins.typespecs.values() {
        env.add_typespec_box(ts.clone())
    }
}

// this is only ever used when changing builtins
fn _save_builtins(env: &ExecutionEnvironment) -> Result<(), Box<dyn std::error::Error>> {
    #[allow(unused_imports)]
    use lang::Function;
    #[allow(unused_imports)]
    use std::collections::HashMap;

    let mut functions: Vec<Box<dyn lang::Function>> = vec![];
    functions.push(Box::new(builtins::Print {}));
    functions.push(Box::new(builtins::Capitalize {}));
    functions.push(Box::new(builtins::JoinString {}));
    functions.push(Box::new(builtins::ChatReply::new(Arc::new(Mutex::new(vec![])))));

    let struct_ids = &[// HTTP Form param
                       uuid::Uuid::parse_str("b6566a28-8257-46a9-aa29-39d9add25173").unwrap(),
                       // Chat Message
                       uuid::Uuid::parse_str("cc430c68-1eba-4dd7-a3a8-0ee8e202ee83").unwrap(),
                       // HTTP Response
                       uuid::Uuid::parse_str("31d96c85-5966-4866-a90a-e6db3707b140").unwrap()];
    let enum_ids = &[// Result
                     uuid::Uuid::parse_str("ffd15538-175e-4f60-8acd-c24222ddd664").unwrap()];

    builtins::Builtins { funcs: functions.into_iter().map(|f| (f.id(), f)).collect(),
                         typespecs: struct_ids.iter()
                                              .chain(enum_ids.iter())
                                              .map(|ts_id| {
                                                  (*ts_id,
                                                   env.find_typespec(*ts_id).unwrap().clone())
                                              })
                                              .collect() }.save()
}
