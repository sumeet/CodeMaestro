use super::env;
use super::lang;
use super::external_func;

use serde_derive::{Serialize,Deserialize};
use stdweb::{js,_js_impl};
use stdweb;
use std::collections::HashMap;
use stdweb::unstable::TryInto;
use stdweb::web::error;
use stdweb::traits::IError;
use stdweb::private::ConversionError;

#[derive(Clone, Serialize, Deserialize)]
pub struct JSFunc {
    pub eval: String,
    pub return_type: lang::Type,
    pub name: String,
    pub id: lang::ID,
    pub args: Vec<lang::ArgumentDefinition>
}

impl JSFunc {
    pub fn new() -> Self {
        Self {
            eval: "".to_string(),
            return_type: lang::Type::from_spec(&*lang::NULL_TYPESPEC),
            name: "New JSFunc".to_string(),
            id: lang::new_id(),
            args: vec![],
        }
    }

    fn extract(&self, value: stdweb::Value) -> lang::Value {
        use self::lang::Function;
        self.ex(value, &self.returns())
    }

    fn ex(&self, value: stdweb::Value, into_type: &lang::Type) -> lang::Value {
        if into_type.matches_spec(&lang::STRING_TYPESPEC) {
            if let Some(string) = value.into_string() {
                return lang::Value::String(string)
            }
        } else if into_type.matches_spec(&lang::NUMBER_TYPESPEC) {
            if let Ok(int) = value.try_into() {
                let val : i64 = int;
                return lang::Value::Number(val as i128)
            }
        } else if into_type.matches_spec(&lang::NULL_TYPESPEC) {
            if value.is_null() {
                return lang::Value::Null
            }
        } else if into_type.matches_spec(&lang::LIST_TYPESPEC) {
            if value.is_array() {
                let vec : Vec<stdweb::Value> = value.try_into().unwrap();
                let collection_type = into_type.params.first().unwrap();
                let collected: Vec<lang::Value> = vec.into_iter()
                    .map(|value| {
                        self.ex(value, collection_type)
                    })
                    .collect();
                return lang::Value::List(collected)
            }
        }
        lang::Value::Error(lang::Error::JavaScriptDeserializationError)
    }
}

// caveats regarding this eval:
// 1) we don't support asynchronous code at all ATM
// 2) down the line, any JavaScript Error thrown will get converted into a
//    lang::Error::JavascriptError with a tuple containing (JS exception name, JS exception message)
// 3) any instance of Error returned (not thrown) will also be treated as an error
// 4) anything thrown that's not an Error, will result in a lang::JavascriptDeserializationError
fn eval(js_code: &str) -> Result<stdweb::Value, (String, String)> {
    let value = js! {
        try {
            return eval(@{js_code});
        } catch(err) {
            return err;
        }
    };
    if let Some(value) = value.as_reference() {
        let error : Result<error::Error, ConversionError> = value.try_into();
        if let Ok(error) = error {
            return Err((error.name(), error.message()));
        }
    }
    Ok(value)
}

impl lang::Function for JSFunc {
    fn call(&self, _env: &mut env::ExecutionEnvironment, _args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        match eval(&self.eval) {
            Err((err_name, err_string)) => lang::Value::Error(lang::Error::JavaScriptError(err_name, err_string)),
            Ok(value) => self.extract(value)
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        self.args.clone()
    }

    fn returns(&self) -> lang::Type {
        self.return_type.clone()
    }
}

impl external_func::ModifyableFunc for JSFunc {
    fn set_return_type(&mut self, return_type: lang::Type) {
        self.return_type = return_type
    }

    fn set_args(&mut self, args: Vec<lang::ArgumentDefinition>) {
        self.args = args
    }

    fn clone(&self) -> Self {
        JSFunc {
            eval: self.eval.clone(),
            return_type: self.return_type.clone(),
            name: self.name.clone(),
            id: self.id.clone(),
            args: self.args.clone(),
        }
    }
}
