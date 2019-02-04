use http;
use super::result::Result;

pub fn get(url: &str) -> Result<http::Request<String>> {
    Ok(http::Request::get(url).body("".to_owned())?)
}