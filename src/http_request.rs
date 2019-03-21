use super::result::Result;
use http;
use serde_json;
use url;

pub fn get(url: &str) -> Result<http::Request<String>> {
    Ok(http::Request::get(url::Url::parse(url)?.as_str()).body("".to_owned())?)
}

pub fn post_json(url: &str, data: &impl serde::Serialize) -> Result<http::Request<String>> {
    Ok(http::Request::post(url::Url::parse(url)?.as_str()).body(serde_json::to_string(data)?)?)
}
