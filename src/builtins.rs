use cfg_if::{cfg_if};

use std::collections::HashMap;
use super::lang;
use super::env;
use super::structs;
use serde_json::json;
use lazy_static::lazy_static;


// this gets loaded through codesample.json... TODO: make a builtins.json file
lazy_static! {
    pub static ref HTTP_RESPONSE_STRUCT_ID : uuid::Uuid = uuid::Uuid::parse_str("31d96c85-5966-4866-a90a-e6db3707b140").unwrap();
    pub static ref RESULT_ENUM_ID : uuid::Uuid = uuid::Uuid::parse_str("ffd15538-175e-4f60-8acd-c24222ddd664").unwrap();
}

#[derive(Clone)]
pub struct Print {}

impl lang::Function for Print {
    fn call(&self, interpreter: env::Interpreter, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        match args.get(&self.takes_args()[0].id) {
            Some(lang::Value::String(ref string)) =>  {
                interpreter.env.borrow_mut().println(string);
                lang::Value::Null
            }
            _ => lang::Value::Error(lang::Error::ArgumentError)
        }
    }

    fn name(&self) -> &str {
        "Print"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("b5c18d63-f9a0-4f08-8ee7-e35b3db9122d").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("feff08f0-7319-4b47-964e-1f470eca81df").unwrap(),
                lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                "String to print".to_string()
            )
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NULL_TYPESPEC)
    }
}

#[derive(Clone)]
pub struct Capitalize {}

impl lang::Function for Capitalize {
    fn call(&self, _interpreter: env::Interpreter, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        match args.get(&self.takes_args()[0].id) {
            Some(lang::Value::String(ref string)) =>  {
                lang::Value::String(string.to_uppercase())
            }
            _ => lang::Value::Error(lang::Error::ArgumentError)
        }
    }

    fn name(&self) -> &str {
        "Capitalize"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("86ae2a51-5538-436f-b48e-3aa6c873b189").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("94e81ddc-843b-426d-847e-a215125c9593").unwrap(),
                lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                "String to capitalize".to_string(),
            )
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::STRING_TYPESPEC)
    }
}


cfg_if! {
    if #[cfg(feature = "javascript")] {
        #[derive(Clone)]
        pub struct HTTPGet {}

        use http::Request;
        use super::http_client;
        use futures_util::future::FutureExt;

        impl lang::Function for HTTPGet {
            fn call(&self, _env: &mut env::ExecutionEnvironment, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
                match args.get(&self.takes_args()[0].id) {
                    Some(lang::Value::String(ref url)) =>  {
                        let request = Request::get(url).body(()).unwrap();
                        lang::Value::Future(
                            FutureExt::shared(Box::pin(async move {
                                let str = await!(http_client::fetch(request)).unwrap();
                                lang::Value::String(str)
                            }))
                        )
                    }
                    _ => lang::Value::Error(lang::Error::ArgumentError)
                }
            }

            fn name(&self) -> &str {
                "HTTP Get"
            }

            fn id(&self) -> lang::ID {
                uuid::Uuid::parse_str("7a5952b5-f814-40a7-b555-e01ac6eb2d69").unwrap()
            }

            fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
                vec![
                    lang::ArgumentDefinition::new_with_id(
                        uuid::Uuid::parse_str("7a5952b5-f814-40a7-b555-e01ac6eb2d69").unwrap(),
                        lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                        "URL".to_string(),
                    )
                ]
            }

            fn returns(&self) -> lang::Type {
                lang::Type::from_spec(&*lang::STRING_TYPESPEC)
            }
        }
    } else if #[cfg(feature = "default")] {
        #[derive(Clone)]
        pub struct HTTPGet {}

        impl lang::Function for HTTPGet {
            fn call(&self, _interpreter: env::Interpreter, _args: HashMap<lang::ID, lang::Value>) -> lang::Value {
                unimplemented ! ();
            }

            fn name(&self) -> &str {
                "HTTP Get"
            }

            fn id(&self) -> lang::ID {
                uuid::Uuid::parse_str("7a5952b5-f814-40a7-b555-e01ac6eb2d69").unwrap()
            }

            fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
                vec![
                    lang::ArgumentDefinition::new_with_id(
                        uuid::Uuid::parse_str("7a5952b5-f814-40a7-b555-e01ac6eb2d69").unwrap(),
                        lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                        "URL".to_string())
                ]
            }

            fn returns(&self) -> lang::Type {
                lang::Type::from_spec(&*lang::STRING_TYPESPEC)
            }
        }
    }
}


fn new_result(ok_type: lang::Type) -> lang::Type {
    lang::Type { typespec_id: *RESULT_ENUM_ID, params: vec![ok_type] }
}
