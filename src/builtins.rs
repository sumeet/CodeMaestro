use std::collections::HashMap;
use super::lang;
use super::env;
use lazy_static::lazy_static;
use itertools::Itertools;
use url;
use http::Request;
use super::http_client;

// this gets loaded through codesample.json... TODO: make a builtins.json file
lazy_static! {
    pub static ref HTTP_RESPONSE_STRUCT_ID : uuid::Uuid = uuid::Uuid::parse_str("31d96c85-5966-4866-a90a-e6db3707b140").unwrap();
    pub static ref RESULT_ENUM_ID : uuid::Uuid = uuid::Uuid::parse_str("ffd15538-175e-4f60-8acd-c24222ddd664").unwrap();
    pub static ref HTTP_FORM_PARAM_ID : uuid::Uuid = uuid::Uuid::parse_str("b6566a28-8257-46a9-aa29-39d9add25173").unwrap();
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



#[derive(Clone)]
pub struct HTTPGet {}

impl lang::Function for HTTPGet {
    fn call(&self, _interpreter: env::Interpreter, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        let url = self.get_url(args);
        let request = Request::get(url.to_string()).body("".to_owned()).unwrap();
        lang::Value::new_future(async move {
            lang::Value::String(await!(http_client::fetch(request)).unwrap().body().to_owned())
        })

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
                "URL".to_string()),
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("291b9156-db02-4d81-a965-4b5a95bb51a5").unwrap(),
                lang::Type::with_params(
                    &*lang::LIST_TYPESPEC,
                    vec![lang::Type::from_spec_id(*HTTP_FORM_PARAM_ID, vec![])]),
                "URL params".to_string()),
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::STRING_TYPESPEC)
    }
}

impl HTTPGet {
    fn get_url(&self, args: HashMap<lang::ID, lang::Value>) -> String {
        // jesus christ
        let url = args.get(&uuid::Uuid::parse_str("7a5952b5-f814-40a7-b555-e01ac6eb2d69").unwrap()).unwrap().as_str().unwrap();
        // these SHOULD be key, value pairs for URL params
        let form_params = args.get(&uuid::Uuid::parse_str("291b9156-db02-4d81-a965-4b5a95bb51a5").unwrap()).unwrap().as_vec().unwrap()
            .iter()
            .map(|val| val.as_struct().unwrap())
            .map(|(_id, struct_values)| {
                (
                    struct_values.get(&uuid::Uuid::parse_str("886a86df-1211-47c5-83c0-f9a410a6fdc8").unwrap()).unwrap().as_str().unwrap(),
                    struct_values.get(&uuid::Uuid::parse_str("57607724-a63a-458e-9253-1e3efeb4de63").unwrap()).unwrap().as_str().unwrap(),
                )
            })
            .collect_vec();

        let mut url = url::Url::parse(url).unwrap();
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in form_params {
                pairs.append_pair(key, value);
            }
        }
        url.to_string()
    }
}


//fn new_result(ok_type: lang::Type) -> lang::Type {
//    lang::Type { typespec_id: *RESULT_ENUM_ID, params: vec![ok_type] }
//}
