// generates a JS file with all environment variables
//
// used because the wasm app reads "environment variables" just like the main app does. see
// src/config.rs
use std::collections::HashMap;
use std::env;
use std::fs;

use serde_json;

fn main() {
    let filename = env::args().nth(1).expect("filename must be the first arg");
    let env_vars = env::vars().collect::<HashMap<_, _>>();
    let json_dump_of_env_vars = serde_json::to_string_pretty(&env_vars).unwrap();
    let executable_js_code = format!("ENV = {};", json_dump_of_env_vars);
    fs::write(filename, executable_js_code).unwrap();
}
