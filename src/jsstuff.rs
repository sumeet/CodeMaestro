use super::env;
use super::lang;
use super::external_func;
use super::stdweb;

use std::collections::HashMap;
use stdweb::unstable::TryInto;

#[derive(Clone)]
pub struct JSFunc {
    pub eval: String,
    pub return_type: lang::Type,
    pub name: String,
    pub id: lang::ID,
}

impl JSFunc {
    pub fn new() -> Self {
        Self {
            eval: "".to_string(),
            return_type: lang::Type::from_spec(&lang::NULL_TYPESPEC),
            name: "New JSFunc".to_string(),
            id: lang::new_id(),
        }
    }

    fn extract(&self, value: stdweb::Value) -> lang::Value {
        use self::lang::Function;
        self.ex(value, &self.returns())
    }

    fn ex(&self, value: stdweb::Value, into_type: &lang::Type) -> lang::Value {
        if into_type.matches_spec(&lang::STRING_TYPESPEC) {
            if let (Some(string)) = value.into_string() {
                return lang::Value::String(string)
            }
        } else if into_type.matches_spec(&lang::NUMBER_TYPESPEC) {
            if let(Ok(int)) = value.try_into() {
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

impl lang::Function for JSFunc {
    fn call(&self, _env: &mut env::ExecutionEnvironment, _args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        let value = js! { return eval(@{&self.eval}); };
        self.extract(value)
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn id(&self) -> lang::ID {
        self.id
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![]
    }

    fn returns(&self) -> lang::Type {
        self.return_type.clone()
    }
}

impl external_func::ModifyableFunc for JSFunc {
    fn set_return_type(&mut self, return_type: lang::Type) {
        self.return_type = return_type
    }

    fn clone(&self) -> Self {
        JSFunc {
            eval: self.eval.clone(),
            return_type: self.return_type.clone(),
            name: self.name.clone(),
            id: self.id.clone(),
        }
    }
}
