use reqwest::{r#async::Client};
use http::{Request,Response};
use futures::stream::Stream;

use super::asynk::forward;

#[allow(dead_code)] // compiler bug warns for this type alias not being used :/
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub async fn fetch(request: Request<String>) -> Result<Response<String>> {
    let resp = await!(forward(Client::new()
        .request(request.method().clone(), &request.uri().to_string())
        .headers(request.headers().clone())
        .body(request.body().clone())
        .send()))?;

    let mut resp_builder = Response::builder();
    resp_builder.status(resp.status());
    for (key, val) in resp.headers().iter() {
        resp_builder.header(key, val);
    }

    let body = await!(forward(resp.into_body().concat2()))?;
    let body = String::from_utf8_lossy(&body);
    Ok(resp_builder.body(body.into())?)
}


