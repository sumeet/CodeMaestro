use super::env;
use super::external_func;
use super::external_func::ValueWithEnv;
use super::function;
use super::lang;
use super::structs;

use crate::env::ExecutionError;
use serde;
use serde::ser::{SerializeMap, SerializeSeq};
use serde_derive::{Deserialize, Serialize};
use std::collections::HashMap;
use stdweb;
use stdweb::private::ConversionError;
use stdweb::traits::IError;
use stdweb::unstable::TryInto;
use stdweb::web::error;
use stdweb::{__js_serializable_boilerplate, js, js_serializable};

#[derive(Clone, Serialize, Deserialize)]
pub struct JSFunc {
    pub eval: String,
    pub return_type: lang::Type,
    pub name: String,
    pub id: lang::ID,
    pub args: Vec<lang::ArgumentDefinition>,
}

impl JSFunc {
    pub fn new() -> Self {
        Self { eval: "".to_string(),
               return_type: lang::Type::from_spec(&*lang::NULL_TYPESPEC),
               name: "New JSFunc".to_string(),
               id: lang::new_id(),
               args: vec![] }
    }

    fn extract(&self, value: stdweb::Value, env: &env::ExecutionEnvironment) -> lang::Value {
        use self::lang::Function;
        self.ex(value, &self.returns(), env)
    }

    fn ex(&self,
          value: stdweb::Value,
          into_type: &lang::Type,
          env: &env::ExecutionEnvironment)
          -> Result<lang::Value, ExecutionError> {
        if into_type.matches_spec(&lang::STRING_TYPESPEC) {
            if let Some(string) = value.into_string() {
                return Ok(lang::Value::String(string));
            }
        } else if into_type.matches_spec(&lang::NUMBER_TYPESPEC) {
            if let Ok(int) = value.try_into() {
                let val: i64 = int;
                return Ok(lang::Value::Number(val as i128));
            }
        } else if into_type.matches_spec(&lang::NULL_TYPESPEC) {
            if value.is_null() {
                return Ok(lang::Value::Null);
            }
        } else if into_type.matches_spec(&lang::LIST_TYPESPEC) {
            if value.is_array() {
                let vec: Vec<stdweb::Value> = value.try_into().unwrap();
                let collection_type = into_type.params.first().unwrap();
                let collected: Vec<lang::Value> =
                    vec.into_iter()
                       .map(|value| self.ex(value, collection_type, env))
                       .collect();
                return Ok(lang::Value::List(collected));
            }
        } else if let Some(strukt) = env.find_struct(into_type.typespec_id) {
            if let Some(value) = self.stdweb_value_into_struct(value, strukt, env) {
                return value;
            }
        }
        panic!(ExecutionError::JavaScriptDeserializationError)
    }

    fn stdweb_value_into_struct(&self,
                                value: stdweb::Value,
                                strukt: &structs::Struct,
                                env: &env::ExecutionEnvironment)
                                -> Option<lang::Value> {
        if let Some(obj) = value.into_object() {
            let mut map: HashMap<String, stdweb::Value> = obj.into();
            let values: Option<_> = strukt.fields
                                          .iter()
                                          .map(|strukt_field| {
                                              let js_obj = map.remove(&strukt_field.name)?;
                                              Some((strukt_field.id,
                                                    self.ex(js_obj, &strukt_field.field_type, env)))
                                          })
                                          .collect();
            return Some(lang::Value::Struct { struct_id: strukt.id,
                                              values: values? });
        }
        None
    }
}

impl<'a> serde::Serialize for ValueWithEnv<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: serde::Serializer
    {
        use super::lang::Value::*;
        match (&self.env, &self.value) {
            (_, Null) => serializer.serialize_none(),
            (_, Boolean(b)) => serializer.serialize_bool(*b),
            (_, String(s)) => serializer.serialize_str(&s),
            // not quite sure what to do with these...
            // TODO: fix this i128 to i64 cast...
            (_, Number(i)) => serializer.serialize_i64(*i as i64),
            (env, List(v)) => {
                let mut seq = serializer.serialize_seq(Some(v.len()))?;
                for item in v {
                    // TODO: ugh this clone...
                    seq.serialize_element(&Self { value: item.clone(),
                                                  env })?;
                }
                seq.end()
            }
            (env, Struct { struct_id, values }) => {
                let strukt = env.find_struct(*struct_id).unwrap();
                let field_by_id = strukt.field_by_id();
                let mut map = serializer.serialize_map(Some(values.len()))?;
                for (id, value) in values {
                    // TODO: ugh this clone
                    let val_with_env = Self { value: value.clone(),
                                              env };
                    let name = &field_by_id.get(&id).unwrap().name;
                    map.serialize_entry(name, &val_with_env)?;
                }
                map.end()
            }
            (env, Enum { box value, .. }) => Self { value: value.clone(),
                                                    env }.serialize(serializer),
            // TODO: map it into a JS future
            (_env, Future(_old_fut)) => serializer.serialize_none(),
        }
    }
}

// WTF?
js_serializable!(impl <'a> for ValueWithEnv<'a>);

// caveats regarding this eval:
// 1) we don't support asynchronous code at all ATM
// 2) down the line, any JavaScript Error thrown will get converted into a
//    lang::Error::JavascriptError with a tuple containing (JS exception name, JS exception message)
// 3) any instance of Error returned (not thrown) will also be treated as an error
// 4) anything thrown that's not an Error, will result in a lang::JavascriptDeserializationError
fn eval(js_code: &str,
        locals: HashMap<String, ValueWithEnv>)
        -> Result<stdweb::Value, (String, String)> {
    let value = js! {
        try {
            return  CS_EVAL__(@{js_code}, @{locals});
        } catch(err) {
            return err;
        }
    };
    if let Some(value) = value.as_reference() {
        let error: Result<error::Error, ConversionError> = value.try_into();
        if let Ok(error) = error {
            return Err((error.name(), error.message()));
        }
    }
    Ok(value)
}

#[typetag::serde]
impl lang::Function for JSFunc {
    fn call(&self,
            interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let env = interpreter.env.borrow();
        let named_args: HashMap<String, ValueWithEnv> =
            external_func::to_named_args(self, args).map(|(name, value)| {
                                                        (name, ValueWithEnv { env: &env, value })
                                                    })
                                                    .collect();

        match eval(&self.eval, named_args) {
            Err((err_name, err_string)) => panic!(ExecutionError::JavascriptError),
            Ok(value) => self.extract(value, &env),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    // TODO: i'm implementing code descriptions, but i currently don't give a shit about JSFuncs,
    // so...
    fn description(&self) -> &str {
        "I don't care about this"
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
}

impl function::SettableArgs for JSFunc {
    fn set_args(&mut self, args: Vec<lang::ArgumentDefinition>) {
        self.args = args
    }
}
