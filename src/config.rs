use lazy_static::lazy_static;
use std::collections::HashMap;
use url;

#[cfg(target_arch = "wasm32")]
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

#[cfg(not(target_arch = "wasm32"))]
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

pub fn get_or_err(key: &str) -> Result<&str, Box<dyn std::error::Error>> {
    Ok(get(key).ok_or(format!("{} not set", key))?)
}

pub fn server_listen_url() -> Result<url::Url, Box<dyn std::error::Error>> {
    Ok(url::Url::parse(get_or_err("SERVER_LISTEN_URL")?)?)
}

pub fn edit_code_url(querystring: &str) -> Result<url::Url, Box<dyn std::error::Error>> {
    // XXX this /postthecode is duped in irctest.rs
    let mut url = server_listen_url()?;
    url.set_query(Some(querystring));
    Ok(url)
}

pub fn post_code_url(querystring: &str) -> Result<url::Url, Box<dyn std::error::Error>> {
    // XXX this /postthecode is duped in irctest.rs
    let mut url = server_listen_url()?.join("/postthecode")?;
    url.set_query(Some(querystring));
    Ok(url)
}
