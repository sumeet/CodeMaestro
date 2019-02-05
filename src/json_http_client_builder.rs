use itertools::Itertools;
use matches::matches;
use serde_json;

use super::json2;
use super::lang;
use super::http_request;
use super::json_http_client;
use super::async_executor::AsyncExecutor;
use super::result::{Result as EZResult};

#[derive(Clone)]
pub struct JSONHTTPClientBuilder {
    pub test_url: String,
    pub test_run_result: Option<Result<serde_json::Value,String>>,
    pub json_http_client_id: lang::ID,
    pub selected_fields: Vec<SelectedField>,
}

#[derive(Clone)]
pub struct SelectedField {
    pub name: String,
    pub nesting: json2::Nesting,
    pub typ: lang::Type,
}

impl JSONHTTPClientBuilder {
    pub fn new(json_http_client_id: lang::ID) -> Self {
        Self {
            test_url: "https://httpbin.org/get".to_string(),
            test_run_result: None,
            json_http_client_id,
            selected_fields: vec![]
        }
    }

    pub fn get_selected_field(&self, nesting: &json2::Nesting) -> Option<&SelectedField> {
        self.selected_fields.iter()
            .find(|field| &field.nesting == nesting)
    }

    pub fn add_selected_field(&mut self, nesting: json2::Nesting) {
        self.selected_fields.push(SelectedField {
            name: gen_field_name(&nesting),
            nesting: nesting,
            typ: lang::Type::from_spec(&*lang::NULL_TYPESPEC),
        })
    }

    pub fn run_test<F: FnOnce(JSONHTTPClientBuilder) + 'static>(&self, async_executor: &mut AsyncExecutor,
                                                                callback: F) {
        let url = self.test_url.clone();
        let mut new_builder = self.clone();
        async_executor.exec(async move {
            let val = await!(do_get_request(url));
            let result = val.map_err(|e| e.to_string());
            new_builder.test_run_result = Some(result);
            callback(new_builder);
            let ok : Result<(), ()> = Ok(());
            ok
        });
    }
}

fn gen_field_name(nesting: &json2::Nesting) -> String {
    nesting.iter()
        .filter_map(|n| {
            match n {
                json2::Nest::MapKey(key) => Some(key),
                _ => None,
            }
        }).join("-")
}

async fn do_get_request(url: String) -> EZResult<serde_json::Value> {
    await!(json_http_client::get_json(http_request::get(&url)?))
}
