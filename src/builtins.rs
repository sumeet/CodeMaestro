use super::code_loading;
use super::env;
use super::lang;
use itertools::Itertools;
use lazy_static::lazy_static;
use maplit::hashmap;
use serde::ser::SerializeStruct;
use serde::{
    Deserialize as DeserializeTrait, Deserializer, Serialize as SerializeTrait, Serializer,
};
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::collections::HashMap;
use std::fs::File;
use std::sync::{Arc, Mutex};

// this gets loaded through codesample.json... TODO: make a builtins.json file
lazy_static! {
    pub static ref HTTP_RESPONSE_STRUCT_ID: uuid::Uuid =
        uuid::Uuid::parse_str("31d96c85-5966-4866-a90a-e6db3707b140").unwrap();
    pub static ref RESULT_ENUM_ID: uuid::Uuid =
        uuid::Uuid::parse_str("ffd15538-175e-4f60-8acd-c24222ddd664").unwrap();
    pub static ref HTTP_FORM_PARAM_STRUCT_ID: uuid::Uuid =
        uuid::Uuid::parse_str("b6566a28-8257-46a9-aa29-39d9add25173").unwrap();
    pub static ref MESSAGE_STRUCT_ID: uuid::Uuid =
        uuid::Uuid::parse_str("cc430c68-1eba-4dd7-a3a8-0ee8e202ee83").unwrap();
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Builtins {
    pub funcs: HashMap<lang::ID, Box<dyn lang::Function>>,
    pub typespecs: HashMap<lang::ID, Box<dyn lang::TypeSpec>>,
}

impl Builtins {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let str = include_str!("../builtins.json");
        Ok(Self::deserialize(str)?)
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let f = File::create("builtins.json")?;
        Ok(serde_json::to_writer_pretty(f, self)?)
    }

    pub fn is_builtin(&self, id: lang::ID) -> bool {
        self.funcs.contains_key(&id) || self.typespecs.contains_key(&id)
    }

    fn deserialize(str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let deserialize: BuiltinsDeserialize = serde_json::from_str(str)?;
        let funcs = deserialize.funcs
                               .into_iter()
                               .map(|(_, func_json)| {
                                   let func = code_loading::deserialize_fn(func_json)?;
                                   Ok((func.id(), func))
                               })
                               .collect::<Result<HashMap<_, _>, Box<dyn std::error::Error>>>();
        let typespecs = deserialize.typespecs
                                   .into_iter()
                                   .map(|(_, typespec_json)| {
                                       let typespec =
                                           code_loading::deserialize_typespec(typespec_json)?;
                                       Ok((typespec.id(), typespec))
                                   })
                                   .collect::<Result<HashMap<_, _>, Box<dyn std::error::Error>>>();
        Ok(Self { funcs: funcs?,
                  typespecs: typespecs? })
    }
}

// XXX: ugh we need this to deserialize builtins because typetag doesn't work in wasm :(((
#[derive(Deserialize)]
struct BuiltinsDeserialize {
    pub funcs: HashMap<lang::ID, serde_json::Value>,
    pub typespecs: HashMap<lang::ID, serde_json::Value>,
}

pub fn new_message(sender: String, argument_text: String, full_text: String) -> lang::Value {
    lang::Value::Struct { struct_id: *MESSAGE_STRUCT_ID,
                          values: hashmap! {
                              uuid::Uuid::parse_str("e01e6346-5c8f-4b1b-9723-cde0abf77ec0").unwrap() => lang::Value::String(sender),
                              uuid::Uuid::parse_str("d0d3b2b3-1d25-4d3d-bdca-fe34022eadf2").unwrap() => lang::Value::String(argument_text),
                              uuid::Uuid::parse_str("9a8d9059-a729-4660-b440-8ee7c411e70a").unwrap() => lang::Value::String(full_text),
                          } }
}

pub fn ok_result(value: lang::Value) -> lang::Value {
    lang::Value::Enum { variant_id:
                            uuid::Uuid::parse_str("f70c799a-1d63-4293-889d-55c07a7456a0").unwrap(),
                        value: Box::new(value) }
}

pub fn err_result(string: String) -> lang::Value {
    lang::Value::Enum { variant_id:
                            uuid::Uuid::parse_str("9f22e23e-d9b9-49c2-acf2-43a59598ea86").unwrap(),
                        value: Box::new(lang::Value::String(string)) }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Print {}

#[typetag::serde]
impl lang::Function for Print {
    fn call(&self,
            interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        match args.get(&self.takes_args()[0].id) {
            Some(lang::Value::String(ref string)) => {
                interpreter.env.borrow_mut().println(string);
                lang::Value::Null
            }
            _ => lang::Value::Error(lang::Error::ArgumentError),
        }
    }

    fn name(&self) -> &str {
        "Debug"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("b5c18d63-f9a0-4f08-8ee7-e35b3db9122d").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("feff08f0-7319-4b47-964e-1f470eca81df").unwrap(),
                lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                "Text".to_string()
            )
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NULL_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Capitalize {}

#[typetag::serde]
impl lang::Function for Capitalize {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        match args.get(&self.takes_args()[0].id) {
            Some(lang::Value::String(ref string)) => lang::Value::String(string.to_uppercase()),
            _ => lang::Value::Error(lang::Error::ArgumentError),
        }
    }

    fn name(&self) -> &str {
        "Capitalize"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("86ae2a51-5538-436f-b48e-3aa6c873b189").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("94e81ddc-843b-426d-847e-a215125c9593").unwrap(),
                lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                "String to capitalize".to_string(),
            )
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::STRING_TYPESPEC)
    }
}

pub fn new_result(ok_type: lang::Type) -> lang::Type {
    lang::Type { typespec_id: *RESULT_ENUM_ID,
                 params: vec![ok_type] }
}

#[derive(Clone)]
pub struct ChatReply {
    //    output_buffer: Rc<RefCell<Vec<String>>>,
    output_buffer: Arc<Mutex<Vec<String>>>,
}

impl<'de> DeserializeTrait<'de> for ChatReply {
    fn deserialize<D>(_deserializer: D) -> Result<ChatReply, D::Error>
        where D: Deserializer<'de>
    {
        Ok(ChatReply::new(Arc::new(Mutex::new(vec![]))))
    }
}

impl SerializeTrait for ChatReply {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let state = serializer.serialize_struct("ChatReply", 0)?;
        state.end()
    }
}

impl ChatReply {
    pub fn new(output_buffer: Arc<Mutex<Vec<String>>>) -> Self {
        Self { output_buffer }
    }
}

#[typetag::serde]
impl lang::Function for ChatReply {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let text_to_send = args.get(&self.takes_args()[0].id)
                               .unwrap()
                               .as_str()
                               .unwrap();
        self.output_buffer
            .lock()
            .unwrap()
            .push(text_to_send.to_string());
        lang::Value::Null
    }

    fn name(&self) -> &str {
        "Reply"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("36052afd-cf12-4146-bbc7-f9df04148b73").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("95bbed9a-6757-43c5-8e74-b15862e300c8").unwrap(),
                lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                "Message".to_string()),
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NULL_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct JoinString {}

#[typetag::serde]
impl lang::Function for JoinString {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let joined = args.get(&self.takes_args()[0].id)
                         .unwrap()
                         .as_vec()
                         .unwrap()
                         .iter()
                         .map(|val| val.as_str().unwrap())
                         .join("");
        lang::Value::String(joined)
    }

    fn name(&self) -> &str {
        "Join"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("024247f6-3202-4acc-8d9a-b80a427cda3c").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("78cf269a-2a29-4325-9a18-8d84132485ed").unwrap(),
                lang::Type::with_params(&*lang::LIST_TYPESPEC, vec![lang::Type::from_spec(&*lang::STRING_TYPESPEC)]),
                "Strings".to_string()),
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::STRING_TYPESPEC)
    }
}
