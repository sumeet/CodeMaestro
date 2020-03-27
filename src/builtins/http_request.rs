use crate::builtins::{new_result, new_struct_value};
use crate::env::Interpreter;
use crate::http_client;
use crate::lang;
use crate::lang::{ArgumentDefinition, Type, Value, ID};
use lazy_static::lazy_static;
use maplit::hashmap;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Serialize, Deserialize)]
pub struct HTTPRequest {}

lazy_static! {
    // the ID of the struct (typespec)
    pub static ref HTTP_RESPONSE_STRUCT_ID: uuid::Uuid =
        uuid::Uuid::parse_str("31d96c85-5966-4866-a90a-e6db3707b140").unwrap();

    // struct field IDs
    // body (string)
    static ref HTTP_RESPONSE_BODY_STRING_FIELD_ID: uuid::Uuid =
        uuid::Uuid::parse_str("34268b4f-e617-4e94-adbe-f5f0c9357865").unwrap();
    // status code (int)
    static ref HTTP_RESPONSE_STATUS_CODE_INT_FIELD_ID: uuid::Uuid =
        uuid::Uuid::parse_str("5e6cd734-fe98-47d2-9182-601a5a62e4d2").unwrap();

    // Make HTTP Request argument IDs
    static ref HTTP_METHOD_ARG_ID: uuid::Uuid =
        uuid::Uuid::parse_str("6934f70d-d007-46e4-8c9e-a1a97ab3be30").unwrap();
    static ref URL_ARG_ID: uuid::Uuid =
        uuid::Uuid::parse_str("a8907c89-cf6a-4e0a-938f-f08446d6d09e").unwrap();
}

#[typetag::serde]
impl lang::Function for HTTPRequest {
    fn call(&self, _interpreter: Interpreter, args: HashMap<ID, Value>) -> Value {
        let http_method = match args.get(&HTTP_METHOD_ARG_ID) {
            Some(lang::Value::String(ref string)) => string,
            _ => return lang::Value::Error(lang::Error::ArgumentError),
        };
        let url = match args.get(&URL_ARG_ID) {
            Some(lang::Value::String(ref string)) => string,
            _ => return lang::Value::Error(lang::Error::ArgumentError),
        };

        // build HTTP request
        let mut request_builder = http::Request::builder();
        request_builder.uri(url);
        request_builder.method(http_method.as_str());
        // TODO: make body a parameter
        let request = request_builder.body("".to_string()).unwrap();

        lang::Value::new_future(async move {
            let response = http_client::fetch(request).await;
            match response {
                Ok(resp) => {
                    let status_code = resp.status().as_u16();
                    let body = resp.body();
                    // build up the response struct here
                    new_struct_value(*super::HTTP_RESPONSE_STRUCT_ID,
                                     hashmap! {
                                         *HTTP_RESPONSE_BODY_STRING_FIELD_ID => Value::String(body.clone()),
                                         *HTTP_RESPONSE_STATUS_CODE_INT_FIELD_ID => Value::Number(status_code as _),
                                     })
                }
                Err(err) => super::err_result(err.to_string()),
            }
        })
    }

    fn name(&self) -> &str {
        "Make HTTP Request"
    }

    fn description(&self) -> &str {
        "Makes an HTTP request to a remote server"
    }

    fn id(&self) -> ID {
        *super::HTTP_REQUEST_FUNC_ID
    }

    fn takes_args(&self) -> Vec<ArgumentDefinition> {
        vec![// TODO: this should be an enum, not a string
             lang::ArgumentDefinition::new_with_id(*URL_ARG_ID,
                                                   lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                                                   "URL".to_string()),
             lang::ArgumentDefinition::new_with_id(*HTTP_METHOD_ARG_ID,
                                                   lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                                                   "HTTP Method".to_string())]
    }

    fn returns(&self) -> Type {
        new_result(lang::Type::from_spec_id(*super::HTTP_RESPONSE_STRUCT_ID, Vec::new()))
    }
}
