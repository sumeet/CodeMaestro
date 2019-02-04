use super::builtins;
use super::lang;
use super::env;
use super::function;
use super::builtins::new_result;
use super::external_func;
use super::http_request;
use super::result::Result;
use super::http_client;

use itertools::Itertools;
use http;
use std::future::Future;
use std::collections::HashMap;
use serde_derive::{Serialize,Deserialize};
use serde_json;

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
            url: "https://httpbin.org/get".to_string(),
            name: "JSON HTTP Get Client".to_string(),
            gen_url_params: lang::Block::new(),
            args: vec![]
        }
    }

    pub fn http_request(&self, mut interpreter: env::Interpreter, args: HashMap<lang::ID, lang::Value>) -> impl Future<Output = http::Request<String>> {
        for (id, value) in args {
            interpreter.set_local_variable(id, value)
        }
        let gen_url_params = self.gen_url_params.clone();
        let base_url = self.url.clone();
        async move {
            let url_params_value = await!(interpreter.evaluate(&lang::CodeNode::Block(gen_url_params)));
            let form_params = url_params_value.as_vec().unwrap()
                .iter()
                .map(|val| val.as_struct().unwrap())
                .map(|(_id, struct_values)| {
                    (
                        struct_values.get(&uuid::Uuid::parse_str("886a86df-1211-47c5-83c0-f9a410a6fdc8").unwrap()).unwrap().as_str().unwrap(),
                        struct_values.get(&uuid::Uuid::parse_str("57607724-a63a-458e-9253-1e3efeb4de63").unwrap()).unwrap().as_str().unwrap(),
                    )
                })
                .collect_vec();
            let mut url = url::Url::parse(&base_url).unwrap();
            {
                let mut pairs = url.query_pairs_mut();
                for (key, value) in form_params {
                    pairs.append_pair(key, value);
                }
            }
            http_request::get(&url.to_string()).unwrap()
        }
    }
}

pub async fn get_json(request: http::Request<String>) -> Result<serde_json::Value> {
    let resp = await!(http_client::fetch(request))?;
    Ok(serde_json::from_str(resp.body())?)
}