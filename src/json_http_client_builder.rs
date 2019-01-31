use serde_json;

use super::lang;

pub struct JSONHTTPClientBuilder {
    test_run_result: Option<serde_json::Value>,
    pub json_http_client_id: lang::ID,
}

impl JSONHTTPClientBuilder {
    pub fn new(json_http_client_id: lang::ID) -> Self {
        Self {
            test_run_result: None,
            json_http_client_id,
        }
    }
}