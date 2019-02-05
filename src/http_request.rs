use http;
use url;
use super::result::Result;

pub fn get(url: &str) -> Result<http::Request<String>> {
    Ok(http::Request::get(url::Url::parse(url)?.as_str()).body("".to_owned())?)
}