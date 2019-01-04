use super::env;
use super::lang;
use super::external_func;
use super::external_func::ValueWithEnv;

use itertools::Itertools;
use serde;
use serde::ser::{SerializeSeq,SerializeMap};
use serde_derive::{Serialize,Deserialize};
use stdweb::{js,_js_impl, js_serializable, __js_serializable_serde_boilerplate,
             __js_serializable_boilerplate};
use stdweb::private::SerializedValue;
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


impl<'a> serde::Serialize for ValueWithEnv<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: serde::Serializer {
        use super::lang::Value::*;
        match (&self.env, &self.value) {
            (_, Null) => serializer.serialize_none(),
            (_, String(s)) => serializer.serialize_str(&s),
            // not quite sure what to do with these...
            (_, Error(e)) => serializer.serialize_str(&format!("{:?}", e)),
            // TODO: fix this i128 to i64 cast...
            (_, Number(i)) => serializer.serialize_i64(*i as i64),
            (env, List(v)) => {
                let mut seq = serializer.serialize_seq(Some(v.len()))?;
                for item in v {
                    // TODO: ugh this clone...
                    seq.serialize_element(&Self { value: item.clone(), env })?;
                }
                seq.end()
            },
            (env, Struct { struct_id, values }) => {
                let strukt = env.find_struct(*struct_id).unwrap();
                let field_by_id = strukt.field_by_id();
                let mut map = serializer.serialize_map(Some(values.len()))?;
                for (id, value) in values {
                    // TODO: ugh this clone
                    let val_with_env = Self { value: value.clone(), env };
                    let name = &field_by_id.get(&id).unwrap().name;
                    map.serialize_entry(name, &val_with_env)?;
                }
                map.end()
            }
        }
    }
}

//struct Wrapper<'a, T: serde::Serialize + 'a>(&'a T);

// WTF?
js_serializable!(impl <'a> for ValueWithEnv<'a>);

// caveats regarding this eval:
// 1) we don't support asynchronous code at all ATM
// 2) down the line, any JavaScript Error thrown will get converted into a
//    lang::Error::JavascriptError with a tuple containing (JS exception name, JS exception message)
// 3) any instance of Error returned (not thrown) will also be treated as an error
// 4) anything thrown that's not an Error, will result in a lang::JavascriptDeserializationError
fn eval(js_code: &str, locals: HashMap<String, ValueWithEnv>) -> Result<stdweb::Value, (String, String)> {
    let value = js! {
        try {
            return  CS_EVAL__(@{js_code}, @{locals});
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
    fn call(&self, env: &mut env::ExecutionEnvironment, args: HashMap<lang::ID, lang::Value>) -> lang::Value {
        let named_args : HashMap<String, ValueWithEnv> = external_func::to_named_args(self, args)
            .map(|(name, value)| (name, ValueWithEnv { env, value })).collect();

        match eval(&self.eval, named_args) {
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
