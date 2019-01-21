use super::builtins;
use super::lang;
use super::env;
use super::function;

use http;
use std::collections::HashMap;

#[derive(Clone)]
pub struct JSONHTTPClient {
    pub url: String,
    pub name: String,
    pub method: http::Method,
    pub url_params: Vec<(String, String)>,
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
        vec![lang::ArgumentDefinition {
            id: uuid::Uuid::parse_str("6c132cea-b6df-46e0-91e3-2a16616a9ac4").unwrap(),
            arg_type: lang::Type {
                typespec_id: lang::STRING_TYPESPEC.id,
                params: vec![]
            },
            short_name: "URL".to_string(),
        }]
    }

    fn returns(&self) -> lang::Type {
        lang::Type {
            typespec_id: *builtins::RESULT_ENUM_ID,
            params: vec![
                lang::Type {
                    typespec_id: *builtins::HTTP_RESPONSE_STRUCT_ID,
                    params: vec![],
                }
            ]
        }
    }
}

impl function::SettableArgs for JSONHTTPClient {
    fn set_args(&mut self, args: Vec<lang::ArgumentDefinition>) {
        self.args = args
    }
}