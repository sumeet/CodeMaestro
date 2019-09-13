use futures::stream::Stream;
use http::{Request, Response};
use reqwest::r#async::Client;

use super::asynk::forward;

#[allow(dead_code)] // compiler bug warns for this type alias not being used :/
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub async fn fetch(request: Request<String>) -> Result<Response<String>> {
    let resp = forward(Client::new().request(request.method().clone(),
                                                    &request.uri().to_string())
                                           .headers(request.headers().clone())
                                           .body(request.body().clone())
                                           .send()).await?;

    let mut resp_builder = Response::builder();
    resp_builder.status(resp.status());
    for (key, val) in resp.headers().iter() {
        resp_builder.header(key, val);
    }

    let body = forward(resp.into_body().concat2()).await?;
    let body = String::from_utf8_lossy(&body);
    Ok(resp_builder.body(body.into())?)
}
