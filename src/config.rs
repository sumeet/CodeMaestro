use lazy_static::lazy_static;
use std::collections::HashMap;

#[cfg(feature = "javascript")]
lazy_static! {
    static ref ENV: HashMap<String, String> = {
        use stdweb::{_js_impl, js};
        let env = js! { return ENV; };
        let env: HashMap<String, _> = env.into_object().expect("ENV isn't a JS object").into();
        env.into_iter()
           .map(|(k, v)| (k, v.into_string().expect("ENV contains non string values")))
           .collect()
    };
}

#[cfg(feature = "default")]
lazy_static! {
    static ref ENV: HashMap<String, String> = {
        use dotenv::dotenv;

        dotenv().ok();
        std::env::vars().collect()
    };
}

pub fn get(key: &str) -> Option<&str> {
    ENV.get(key).map(|v| v.as_str())
}
