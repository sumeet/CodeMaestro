use cs::json_http_client::JSONHTTPClient;
use cs::{env, lang};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct FakeHTTPClient {
    wrapped_http_client: JSONHTTPClient,
    // HACK: the Arc is just so i can clone this
    made_request: Arc<Mutex<Option<http::Request<String>>>>,
}

impl FakeHTTPClient {
    pub fn new(wrapped_http_client: JSONHTTPClient) -> Self {
        Self { wrapped_http_client,
               made_request: Arc::new(Mutex::new(None)) }
    }

    pub fn test_code(&self) -> lang::CodeNode {
        lang::CodeNode::Block(self.wrapped_http_client.test_code.clone())
    }

    pub fn take_made_request(&mut self) -> Option<http::Request<String>> {
        self.made_request.lock().unwrap().take()
    }
}

impl lang::Function for FakeHTTPClient {
    fn call(&self,
            interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let request_future = self.wrapped_http_client.http_request(interpreter, args);
        let our_test_request = Arc::clone(&self.made_request);
        lang::Value::new_future(async move {
            let request = request_future.await;
            our_test_request.lock().unwrap().replace(request);
            lang::Value::Null
        })
    }

    fn name(&self) -> &str {
        self.wrapped_http_client.name()
    }

    fn description(&self) -> &str {
        self.wrapped_http_client.description()
    }

    fn id(&self) -> lang::ID {
        self.wrapped_http_client.id()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        self.wrapped_http_client.takes_args()
    }

    fn returns(&self) -> lang::Type {
        self.wrapped_http_client.returns()
    }

    fn typetag_name(&self) -> &'static str {
        panic!("this isn't actually needed")
    }

    fn typetag_deserialize(&self) {
        panic!("this isn't actually needed")
    }
}

impl<'de> Deserialize<'de> for FakeHTTPClient {
    fn deserialize<D>(_deserializer: D) -> Result<FakeHTTPClient, D::Error>
        where D: Deserializer<'de>
    {
        panic!("we shouldn't ever need this")
    }
}

impl Serialize for FakeHTTPClient {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        panic!("we should never need this")
    }
}
