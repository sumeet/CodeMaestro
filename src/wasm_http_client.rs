use http::{Request, Response};
use stdweb::PromiseFuture;
use stdweb::{js, js_deserializable};
//use stdweb::unstable::TryFrom;
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use stdweb::unstable::TryInto;

pub async fn fetch(request: Request<String>)
                   -> Result<Response<String>, Box<dyn std::error::Error>> {
    let js_resp = js_fetch(request).await?;
    let mut resp_builder = Response::builder();
    resp_builder.status(js_resp.status);
    for (key, val) in js_resp.headers.iter() {
        resp_builder.header(key.as_str(), val.as_str());
    }
    Ok(resp_builder.body(js_resp.text)?)
}

fn js_fetch(request: Request<String>) -> PromiseFuture<JSHTTPResponse> {
    let request_url: String = request.uri().to_string();
    let request_method = request.method().to_string();
    let request_headers = serializable_headers(&request);
    let request_body = request.body();
    js! (
        return CS_FETCH__(@{request_url}, @{request_method}, @{request_headers}, @{request_body});
    ).try_into()
     .unwrap()
}

fn serializable_headers(request: &Request<String>) -> HashMap<String, &[u8]> {
    request.headers()
           .iter()
           .map(|(key, val)| (key.to_string(), val.as_ref()))
           .collect()
}

// this matches the JS object created in scope.js
#[derive(Debug, Serialize, Deserialize)]
struct JSHTTPResponse {
    text: String,
    status: u16,
    headers: HashMap<String, String>,
}

js_deserializable!(JSHTTPResponse);
