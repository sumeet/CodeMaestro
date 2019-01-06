use stdweb::PromiseFuture;
use http::{Request,Response};
use stdweb::{js,_js_impl};
//use stdweb::unstable::TryFrom;
use stdweb::unstable::TryInto;


pub fn fetch<T>(request: Request<T>) -> PromiseFuture<String> {
    let request_url: String = request.uri().to_string();
    js! (
        return CS_FETCH__(@{request_url});
    ).try_into().unwrap()
}