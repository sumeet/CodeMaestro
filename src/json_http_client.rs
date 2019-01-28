use super::builtins;
use super::lang;
use super::env;
use super::function;
use super::builtins::new_result;

use http;
use std::collections::HashMap;

#[derive(Clone)]
pub struct JSONHTTPClient {
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
        "JSON HTTP Client"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("afa2b0b6-88fb-4003-97a1-a329279562b0").unwrap()
    }

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