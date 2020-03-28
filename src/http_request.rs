use super::result::Result;
use http;
use serde_json;
use url;

pub fn new_req(url: &str,
               method: impl Into<http::Method>,
               data: Option<&impl serde::Serialize>)
               -> Result<http::Request<String>> {
    let mut builder = http::Request::builder();
    let builder = builder.method(method.into());
    let builder = builder.uri(url);
    let body = match data {
        None => "".to_string(),
        // TODO: gonna have to not hardcode JSON in here
        Some(data) => serde_json::to_string(data.into())?,
    };
    Ok(builder.body(body)?)
}

pub fn get(url: &str) -> Result<http::Request<String>> {
    Ok(http::Request::get(url::Url::parse(url)?.as_str()).body("".to_owned())?)
}

pub fn post_json(url: &str, data: &impl serde::Serialize) -> Result<http::Request<String>> {
    Ok(http::Request::post(url::Url::parse(url)?.as_str()).body(serde_json::to_string(data)?)?)
}
