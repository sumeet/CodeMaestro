#[cfg(target_arch = "wasm32")]
pub use super::wasm_http_client::*;

#[cfg(not(target_arch = "wasm32"))]
pub use super::native_http_client::*;

use super::http_request;
use super::result::Result;

pub async fn post_json<'a>(url: &'a str,
                           data: &'a impl serde::Serialize)
                           -> Result<http::Response<String>> {
    // TODO: why not just combine http_request.rs with http_client.rs?
    let req = http_request::post_json(url, data)?;
    Ok(await!(fetch(req))?)
}
