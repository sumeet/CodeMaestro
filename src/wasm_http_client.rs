use super::asynk::forward;
use stdweb::PromiseFuture;
use http::{Request,Response};
use stdweb::{js,_js_impl, js_deserializable, __js_deserializable_serde_boilerplate};
//use stdweb::unstable::TryFrom;
use stdweb::unstable::TryInto;
use std::collections::HashMap;
use serde_derive::{Serialize,Deserialize};
use stdweb::{console,__internal_console_unsafe};

// this conflicts with the js_deserializable macro definition if it's called Result, hence the rename
// to EZResult
type EZResult<T> = std::result::Result<T, Box<std::error::Error>>;

pub async fn fetch<T>(request: Request<T>) -> EZResult<Response<String>> {
    let js_resp = await!(js_fetch(request))?;
    let mut resp_builder = Response::builder();
    resp_builder.status(js_resp.status);
    for (key, val) in js_resp.headers.iter() {
        resp_builder.header(key.as_str(), val.as_str());
    }
    Ok(resp_builder.body(js_resp.text)?)
}


fn js_fetch<T>(request: Request<T>) -> PromiseFuture<JSHTTPResponse> {
    let request_url: String = request.uri().to_string();
    js! (
        return CS_FETCH__(@{request_url});
    ).try_into().unwrap()
}


// this matches the JS object created in scope.js
#[derive(Debug, Serialize, Deserialize)]
struct JSHTTPResponse {
    text: String,
    status: u16,
    headers: HashMap<String,String>,
}

js_deserializable!(JSHTTPResponse);
