//use reqwest::{r#async::Client};
use yukikaze::client::{Client, Request as YRequest, HttpClient};
use http::{Request,Response};
use std::future::Future;
use futures::stream::Stream;
use futures::future::{Future as _};

use super::asynk::forward;

type Result<T> = std::result::Result<T, Box<std::error::Error>>;

//pub async fn fetch(request: Request<String>) -> Result<Response<String>> {
//    let resp = await!(forward(Client::new()
//        .request(request.method().clone(), &request.uri().to_string())
//        .headers(request.headers().clone())
//        .body(request.body().clone())
//        .send()))?;
//
//    let request_url: String = request.uri().to_string();
//    let status = resp.status();
//    let body = await!(forward(resp.into_body().concat2()))?;
//    let body = String::from_utf8_lossy(&body);
//    Ok(Response::builder()
//        .status(status)
//        .body(body.into())
//        .unwrap())
//}


pub async fn fetch(request: Request<String>) -> Result<Response<String>> {
    // keep the client in the environment maybe???? to take advantage of connection pooling!
    // TODO: get rid of the unwraps
    let client = Client::default();
    println!("B");

    let request = YRequest::new(
        request.method().into(),
        request.uri().to_string()
    ).unwrap().body(Some(request.body().as_bytes()));

    println!("C");

    let resp = await!(forward(client.execute(request))).unwrap();

    println!("D");

    let status = resp.status();
    let body = await!(forward(resp.text())).unwrap();

    Ok(Response::builder()
        .status(status)
        .body(body)?
    )
}
