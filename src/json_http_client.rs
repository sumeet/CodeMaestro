use super::builtins;
use super::lang;
#[macro_use] use super::env;
use super::function;
use super::builtins::new_result;
use super::external_func;
use super::http_request;
use super::result::Result;
use super::http_client;
use super::structs;

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
    pub args: Vec<lang::ArgumentDefinition>,
    pub return_type: lang::Type,
}

impl lang::Function for JSONHTTPClient {
    fn call(&self, interpreter: env::Interpreter, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        let request = self.http_request(interpreter.dup(), args);
        let returns = self.return_type.clone();
        lang::Value::new_future(async move {
            let request = await!(request);
            match await!(get_json(request)) {
                Ok(json_value) => {
                    match ex(json_value, &returns, &interpreter.env.borrow()) {
                        Ok(value) => builtins::ok_result(value),
                        Err(e) => builtins::err_result(e)
                    }

                },
                Err(err_string) => {
                    builtins::err_result(err_string.to_string())
                }
            }
        })
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
        new_result(self.return_type.clone())
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
            args: vec![],
            return_type: lang::Type::from_spec(&*lang::NULL_TYPESPEC),
        }
    }

    pub fn http_request(&self, mut interpreter: env::Interpreter, args: HashMap<lang::ID, lang::Value>) -> impl Future<Output=http::Request<String>> {
        for (id, value) in args {
            interpreter.set_local_variable(id, value)
        }
        let gen_url_params = self.gen_url_params.clone();
        let base_url = self.url.clone();
        async move {
            let url_params_value = await_eval_result!(interpreter.evaluate(&lang::CodeNode::Block(gen_url_params)));
            let form_params = extract_form_params(&url_params_value);
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

fn ex(value: serde_json::Value, into_type: &lang::Type, env: &env::ExecutionEnvironment) -> std::result::Result<lang::Value, String> {
    if into_type.matches_spec(&lang::STRING_TYPESPEC) {
        if let Some(string) = value.as_str() {
            return Ok(lang::Value::String(string.to_owned()))
        } else if let Some(float) = value.as_f64() {
            return Ok(lang::Value::String(float.to_string()))
        }
    } else if into_type.matches_spec(&lang::NUMBER_TYPESPEC) {
        if let Some(int) = value.as_i64() {
            return Ok(lang::Value::Number(int as i128))
        }
    } else if into_type.matches_spec(&lang::NULL_TYPESPEC) {
        if value.is_null() {
            return Ok(lang::Value::Null)
        }
    } else if into_type.matches_spec(&lang::LIST_TYPESPEC) {
        if value.is_array() {
            // why do we need to clone here??? should our conversion methods take
            // references?
            let vec = value.as_array().unwrap().clone();
            let collection_type = into_type.params.first().unwrap();
            let collected: std::result::Result<Vec<lang::Value>, String> = vec.into_iter()
                .map(|value| {
                    ex(value, collection_type, env)
                })
                .collect();
            return Ok(lang::Value::List(collected?))
        }
    } else if let Some(strukt) = env.find_struct(into_type.typespec_id) {
        if let Some(value) = serde_value_into_struct(value.clone(), strukt, env) {
            return Ok(value)
        }
    }
    Err(format!("couldn't decode {:?}", value))
}

fn serde_value_into_struct(mut value: serde_json::Value, strukt: &structs::Struct,
                            env: &env::ExecutionEnvironment) -> Option<lang::Value> {
    if let Some(map) = value.as_object_mut() {
        let values : Option<_> = strukt.fields.iter()
            .map(|strukt_field| {
                let js_obj = map.remove(&strukt_field.name)?;
                Some((strukt_field.id, ex(js_obj, &strukt_field.field_type, env).ok()?))
            }).collect();
        return Some(lang::Value::Struct { struct_id: strukt.id, values: values? })
    }
    None
}

pub async fn get_json(request: http::Request<String>) -> Result<serde_json::Value> {
    let resp = await!(http_client::fetch(request))?;
    Ok(serde_json::from_str(resp.body())?)
}

fn extract_form_params(http_form_params: &lang::Value) -> Vec<(&str, &str)> {
    http_form_params.as_vec().unwrap()
        .iter()
        .map(|val| val.as_struct().unwrap())
        .map(|(_id, struct_values)| {
            (
                struct_values.get(&uuid::Uuid::parse_str("886a86df-1211-47c5-83c0-f9a410a6fdc8").unwrap()).unwrap().as_str().unwrap(),
                struct_values.get(&uuid::Uuid::parse_str("57607724-a63a-458e-9253-1e3efeb4de63").unwrap()).unwrap().as_str().unwrap(),
            )
        })
        .collect_vec()
}
