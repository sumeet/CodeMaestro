use super::builtins;
use super::env;
use super::function;
use super::http_client;
use super::http_request;
use super::lang;
use super::result::Result;
use super::structs;

use crate::builtins::{
    get_ok_type_from_result_type, none_option_value, ok_result_value, some_option_value,
};
use crate::code_generation;
use http;
use itertools::Itertools;
use lazy_static::lazy_static;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::future::Future;

lazy_static! {
    static ref HTTP_FORM_PARAM_KEY_FIELD_ID: uuid::Uuid =
        uuid::Uuid::parse_str("886a86df-1211-47c5-83c0-f9a410a6fdc8").unwrap();
    static ref HTTP_FORM_PARAM_VALUE_FIELD_ID: uuid::Uuid =
        uuid::Uuid::parse_str("57607724-a63a-458e-9253-1e3efeb4de63").unwrap();
}

// list from http::Method
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
pub enum HTTPMethod {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Trace,
    Connect,
    Patch,
    Options,
}

// ranked in order of common-ness
pub const HTTP_METHOD_LIST: [&HTTPMethod; 9] = [&HTTPMethod::Get,
                                                &HTTPMethod::Post,
                                                &HTTPMethod::Put,
                                                &HTTPMethod::Delete,
                                                &HTTPMethod::Patch,
                                                &HTTPMethod::Head,
                                                &HTTPMethod::Trace,
                                                &HTTPMethod::Connect,
                                                &HTTPMethod::Options];

impl HTTPMethod {
    pub fn to_display(&self) -> &str {
        match self {
            HTTPMethod::Get => "GET",
            HTTPMethod::Post => "POST",
            HTTPMethod::Put => "PUT",
            HTTPMethod::Delete => "DELETE",
            HTTPMethod::Head => "HEAD",
            HTTPMethod::Trace => "TRACE",
            HTTPMethod::Connect => "CONNECT",
            HTTPMethod::Patch => "PATCH",
            HTTPMethod::Options => "OPTIONS",
        }
    }
}

impl From<HTTPMethod> for http::Method {
    fn from(method: HTTPMethod) -> Self {
        match method {
            HTTPMethod::Get => http::Method::GET,
            HTTPMethod::Post => http::Method::POST,
            HTTPMethod::Put => http::Method::PUT,
            HTTPMethod::Delete => http::Method::DELETE,
            HTTPMethod::Head => http::Method::HEAD,
            HTTPMethod::Trace => http::Method::TRACE,
            HTTPMethod::Connect => http::Method::CONNECT,
            HTTPMethod::Patch => http::Method::PATCH,
            HTTPMethod::Options => http::Method::OPTIONS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JSONHTTPClient {
    id: lang::ID,
    // TODO: get rid of URL
    pub url: String,
    pub gen_url_code: lang::Block,
    pub gen_url_params_code: lang::Block,
    pub test_code: lang::Block,
    pub transform_code: lang::Block,
    // TODO: for body params, we can use a JSON enum strings, ints, bools, etc.
    pub name: String,
    // hardcoded to GET for now
    pub method: HTTPMethod,
    pub description: String,
    pub args: Vec<lang::ArgumentDefinition>,
    pub intermediate_parse_schema: lang::Type,
    pub intermediate_parse_structs: Vec<structs::Struct>,
    pub intermediate_parse_argument: lang::ArgumentDefinition,
    pub return_type_after_transform: lang::Type,
}

#[typetag::serde]
impl lang::Function for JSONHTTPClient {
    fn call(&self,
            interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let request = self.http_request(interpreter.new_stack_frame(), args);
        let returns = self.intermediate_parse_schema.clone();

        // some issues here:
        // FIRST this returns a result, but that's not reflected in the type
        // signatures anywhere.
        // SECONDLY, this needs to go through the transformation processing
        // let's start with SECONDLY just to wire the badboy up end to end
        let intermediate_parse_argument_id = self.intermediate_parse_argument.id;
        let transform_code = self.transform_code.clone();
        lang::Value::new_future(async move {
            let request = request.await;
            match fetch_json(request).await {
                Ok(json_value) => {
                    let converted_lang_value =
                        serde_value_to_lang_value(&json_value,
                                                  get_ok_type_from_result_type(&returns),
                                                  &interpreter.env.borrow());
                    match converted_lang_value {
                        Ok(inner_ok_value) => {
                            let value = ok_result_value(inner_ok_value);

                            // HAPPY CASE: let's do out work side of here
                            // builtins::ok_result(value) <= TODO: use the result later, but for now, we
                            // TODO: get error handling working
                            let mut interpreter = interpreter.new_stack_frame();
                            interpreter.set_local_variable(intermediate_parse_argument_id, value);
                            interpreter.evaluate(&lang::CodeNode::Block(transform_code))
                                       .await
                        }
                        Err(e) => builtins::err_result_string(e),
                    }
                }
                Err(err_string) => builtins::err_result_string(err_string.to_string()),
            }
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    // TODO: this should really return a reference
    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        self.args.clone()
    }

    fn returns(&self) -> lang::Type {
        self.return_type_after_transform.clone()
    }

    fn cs_code(&self) -> Box<dyn Iterator<Item = &lang::Block> + '_> {
        Box::new(
            std::iter::once(&self.gen_url_params_code).
                chain(std::iter::once(&self.gen_url_code)
                    .chain(std::iter::once(&self.test_code))
                    .chain(std::iter::once(&self.transform_code))))
    }
}

impl function::SettableArgs for JSONHTTPClient {
    fn set_args(&mut self, args: Vec<lang::ArgumentDefinition>) {
        self.args = args
    }
}

const DEFAULT_JSON_HTTP_CLIENT_BASE_URL: &'static str = "https://lichess.org/api/user/smt2";

impl JSONHTTPClient {
    fn default_url() -> lang::Block {
        code_generation::new_block(vec![code_generation::new_string_literal(
            DEFAULT_JSON_HTTP_CLIENT_BASE_URL.to_owned(),
        )])
    }

    fn initial_test_code(&self) -> lang::Block {
        code_generation::new_block(
            vec![code_generation::new_function_call_with_placeholder_args(self)]
        )
    }

    pub fn build_intermediate_parse_argument(intermediate_parse_schema: lang::Type)
                                             -> lang::ArgumentDefinition {
        lang::ArgumentDefinition::new(intermediate_parse_schema, "Response".to_string())
    }

    pub fn new() -> Self {
        let id = lang::new_id();
        // TODO: should this be an Option and set to None instead? probably
        let intermediate_parse_schema = lang::Type::from_spec(&*lang::NULL_TYPESPEC);
        let intermediate_parse_argument =
            Self::build_intermediate_parse_argument(intermediate_parse_schema.clone());
        let mut client = Self { id,
                                method: HTTPMethod::Get,
                                url: "https://httpbin.org/get".to_string(),
                                name: "JSON HTTP Get Client".to_string(),
                                description: "".to_string(),
                                gen_url_code: Self::default_url(),
                                gen_url_params_code: lang::Block::new(),
                                test_code: lang::Block::new(),
                                args: vec![],
                                intermediate_parse_schema,
                                intermediate_parse_argument,
                                transform_code: lang::Block::new(),
                                return_type_after_transform:
                                    lang::Type::from_spec(&*lang::NULL_TYPESPEC),
                                intermediate_parse_structs: vec![] };
        client.test_code = client.initial_test_code();
        client
    }

    pub fn http_request(&self,
                        mut interpreter: env::Interpreter,
                        args: HashMap<lang::ID, lang::Value>)
                        -> impl Future<Output = http::Request<String>> {
        for (id, value) in args {
            interpreter.set_local_variable(id, value)
        }
        let gen_url_params = self.gen_url_params_code.clone();
        let gen_url = self.gen_url_code.clone();
        let method = self.method;
        async move {
            let base_url_value =
                await_eval_result!(interpreter.evaluate(&lang::CodeNode::Block(gen_url)));
            let base_url = base_url_value.as_str().unwrap();

            let url_params_value =
                await_eval_result!(interpreter.evaluate(&lang::CodeNode::Block(gen_url_params)));
            let form_params = extract_form_params(&url_params_value);
            let mut url = url::Url::parse(&base_url).unwrap();
            {
                let mut pairs = url.query_pairs_mut();
                for (key, value) in form_params {
                    pairs.append_pair(key, value);
                }
            }
            let none: Option<&String> = None;
            http_request::new_req(&url.as_str(), method, none).unwrap()
        }
    }
}

// TODO: probably need to do something at the response level
pub fn serde_value_to_lang_value_wrapped_in_enum(value: &serde_json::Value,
                                                 into_type: &lang::Type,
                                                 env: &env::ExecutionEnvironment)
                                                 -> std::result::Result<lang::Value, String> {
    Ok(ok_result_value(serde_value_to_lang_value(value, into_type, env)?))
}

fn serde_value_to_lang_value(value: &serde_json::Value,
                             into_type: &lang::Type,
                             env: &env::ExecutionEnvironment)
                             -> std::result::Result<lang::Value, String> {
    if into_type.matches_spec(&lang::STRING_TYPESPEC) {
        if let Some(string) = value.as_str() {
            return Ok(lang::Value::String(string.to_owned()));
        } else if let Some(float) = value.as_f64() {
            return Ok(lang::Value::String(float.to_string()));
        }
    } else if into_type.matches_spec(&lang::NUMBER_TYPESPEC) {
        if let Some(int) = value.as_i64() {
            return Ok(lang::Value::Number(int as i128));
        }
    } else if into_type.matches_spec(&lang::NULL_TYPESPEC) {
        if value.is_null() {
            return Ok(lang::Value::Null);
        }
    } else if into_type.matches_spec(&lang::BOOLEAN_TYPESPEC) {
        if let Some(b) = value.as_bool() {
            return Ok(lang::Value::Boolean(b));
        }
    } else if into_type.matches_spec(&lang::LIST_TYPESPEC) {
        if value.is_array() {
            // TODO: why do we need to clone here??? should our conversion methods take
            // references?
            let vec = value.as_array().unwrap().clone();
            let collection_type = into_type.params.first().unwrap();
            let collected: std::result::Result<Vec<lang::Value>, String> =
                vec.into_iter()
                   .map(|value| serde_value_to_lang_value(&value, collection_type, env))
                   .collect();
            return Ok(lang::Value::List(collection_type.clone(), collected?));
        }
    } else if let Some(strukt) = env.find_struct(into_type.typespec_id) {
        if let Some(value) = serde_value_into_struct(value.clone(), strukt, env) {
            return Ok(value);
        }
    } else if into_type.matches_spec_id(*builtins::OPTION_ENUM_ID) {
        return serde_value_into_option(value.clone(), &into_type.params[0], env).map_err(|e| {
                                                                                    e.to_string()
                                                                                });
    }
    Err(format!("couldn't decode {:?} into {:?}", value, into_type))
}

// helper function to `serde_value_to_lang_value`
fn serde_value_into_option(value: serde_json::Value,
                           some_type: &lang::Type,
                           env: &env::ExecutionEnvironment)
                           -> std::result::Result<lang::Value, Box<dyn std::error::Error>> {
    if value.is_null() {
        Ok(none_option_value())
    } else {
        Ok(some_option_value(serde_value_to_lang_value(&value,
                                                       some_type,
                                                       env)?))
    }
}

fn serde_value_into_struct(mut value: serde_json::Value,
                           strukt: &structs::Struct,
                           env: &env::ExecutionEnvironment)
                           -> Option<lang::Value> {
    let value = value.as_object_mut();
    if value.is_none() {
        return None;
    }
    let map = value.unwrap();
    let values: Option<_> =
        strukt.fields
              .iter()
              .map(|strukt_field| {
                  if strukt_field.field_type
                                 .matches_spec_id(*builtins::OPTION_ENUM_ID)
                     && !map.contains_key(&strukt_field.name)
                  {
                      return Some((strukt_field.id, none_option_value()));
                  }

                  let js_obj = map.remove(&strukt_field.name)?;
                  Some((strukt_field.id,
                        serde_value_to_lang_value(&js_obj, &strukt_field.field_type, env).ok()?))
              })
              .collect();
    Some(lang::Value::Struct { struct_id: strukt.id,
                               values: values? })
}

pub async fn fetch_json(request: http::Request<String>) -> Result<serde_json::Value> {
    let resp = http_client::fetch(request).await?;
    Ok(serde_json::from_str(resp.body())?)
}

fn extract_form_params(http_form_params: &lang::Value) -> Vec<(&str, &str)> {
    http_form_params.as_vec()
                    .unwrap()
                    .iter()
                    .map(|val| val.as_struct().unwrap())
                    .map(|(_id, struct_values)| {
                        (struct_values.get(&HTTP_FORM_PARAM_KEY_FIELD_ID)
                                      .unwrap()
                                      .as_str()
                                      .unwrap(),
                         struct_values.get(&HTTP_FORM_PARAM_VALUE_FIELD_ID)
                                      .unwrap()
                                      .as_str()
                                      .unwrap())
                    })
                    .collect_vec()
}
