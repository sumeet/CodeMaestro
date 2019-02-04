use super::builtins;
use super::lang;
use super::env;
use super::function;
use super::builtins::new_result;
use super::external_func;

use http;
use std::collections::HashMap;
use serde_derive::{Serialize,Deserialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct JSONHTTPClient {
    id: lang::ID,
    pub url: String,
    // for body params, we can use a JSON enum strings, ints, bools, etc.
    pub name: String,
    // hardcoded to GET for now
//    pub method: http::Method,
    pub gen_url_params: lang::Block,
    pub args: Vec<lang::ArgumentDefinition>
}

impl lang::Function for JSONHTTPClient {
    fn call(&self, _interpreter: env::Interpreter, _args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        unimplemented!()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    // TODO: this should really return a reference
    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        self.args.clone()
    }

    fn returns(&self) -> lang::Type {
        new_result(lang::Type {
            typespec_id: *builtins::HTTP_RESPONSE_STRUCT_ID,
            params: vec![],
        })
    }
}

impl function::SettableArgs for JSONHTTPClient {
    fn set_args(&mut self, args: Vec<lang::ArgumentDefinition>) {
        self.args = args
    }
}

impl JSONHTTPClient {
    pub fn new() -> Self {
        Self {
            id: lang::new_id(),
            url: "".to_string(),
            name: "".to_string(),
            gen_url_params: lang::Block::new(),
            args: vec![]
        }
    }
}