use super::code_loading;
use super::env;
use super::lang;
use crate::await_eval_result;
use futures_util::future::join_all;
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

mod http_request;

use crate::env::ExecutionError;
pub use http_request::HTTPRequest;
pub use http_request::HTTP_RESPONSE_STRUCT_ID;

lazy_static! {
    pub static ref HTTP_REQUEST_FUNC_ID: uuid::Uuid =
        uuid::Uuid::parse_str("04ae1441-8499-4ea1-9ecb-8a547e941e8d").unwrap();
    pub static ref RESULT_ENUM_ID: uuid::Uuid =
        uuid::Uuid::parse_str("ffd15538-175e-4f60-8acd-c24222ddd664").unwrap();
    pub static ref RESULT_OK_VARIANT_ID: uuid::Uuid =
        uuid::Uuid::parse_str("f70c799a-1d63-4293-889d-55c07a7456a0").unwrap();
    pub static ref RESULT_ERROR_VARIANT_ID: uuid::Uuid =
        uuid::Uuid::parse_str("9f22e23e-d9b9-49c2-acf2-43a59598ea86").unwrap();
    pub static ref OPTION_ENUM_ID: uuid::Uuid =
        uuid::Uuid::parse_str("f580d95e-2b63-4790-a061-4ddc3d6d21b8").unwrap();
    pub static ref OPTION_SOME_VARIANT_ID: uuid::Uuid =
        uuid::Uuid::parse_str("8049bbb7-ab7e-4b5f-89f8-b248a1e68ca6").unwrap();
    pub static ref OPTION_NONE_VARIANT_ID: uuid::Uuid =
        uuid::Uuid::parse_str("373bd161-d7a0-40b5-9cbe-91bfa449d1e4").unwrap();
    pub static ref HTTP_FORM_PARAM_STRUCT_ID: uuid::Uuid =
        uuid::Uuid::parse_str("b6566a28-8257-46a9-aa29-39d9add25173").unwrap();
    pub static ref MESSAGE_STRUCT_ID: uuid::Uuid =
        uuid::Uuid::parse_str("cc430c68-1eba-4dd7-a3a8-0ee8e202ee83").unwrap();
    pub static ref CHAT_REPLY_FUNC_ID: uuid::Uuid =
        uuid::Uuid::parse_str("36052afd-cf12-4146-bbc7-f9df04148b73").unwrap();
    pub static ref CHAT_REPLY_MESSAGE_ARG_ID: uuid::Uuid =
        uuid::Uuid::parse_str("95bbed9a-6757-43c5-8e74-b15862e300c8").unwrap();
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Builtins {
    pub funcs: HashMap<lang::ID, Box<dyn lang::Function>>,
    pub typespecs: HashMap<lang::ID, Box<dyn lang::TypeSpec>>,
}

impl Builtins {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let str = include_str!("../builtins.json");
        let mut builtins = Self::deserialize(str)?;
        for ts in lang::BUILT_IN_TYPESPECS.iter().cloned().cloned() {
            builtins.typespecs.insert(ts.id, Box::new(ts));
        }
        Ok(builtins)
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

pub fn new_struct_value(struct_id: lang::ID, values: lang::StructValues) -> lang::Value {
    lang::Value::Struct { struct_id, values }
}

pub fn new_message(sender: String, argument_text: String, full_text: String) -> lang::Value {
    new_struct_value(*MESSAGE_STRUCT_ID,
                     hashmap! {
                         uuid::Uuid::parse_str("e01e6346-5c8f-4b1b-9723-cde0abf77ec0").unwrap() => lang::Value::String(sender),
                         uuid::Uuid::parse_str("d0d3b2b3-1d25-4d3d-bdca-fe34022eadf2").unwrap() => lang::Value::String(argument_text),
                         uuid::Uuid::parse_str("9a8d9059-a729-4660-b440-8ee7c411e70a").unwrap() => lang::Value::String(full_text),
                     })
}

pub fn some_option_value(value: lang::Value) -> lang::Value {
    lang::Value::Enum { variant_id: *OPTION_SOME_VARIANT_ID,
                        value: Box::new(value) }
}

pub fn none_option_value() -> lang::Value {
    lang::Value::Enum { variant_id: *OPTION_NONE_VARIANT_ID,
                        value: Box::new(lang::Value::Null) }
}

pub fn ok_result_value(value: lang::Value) -> lang::Value {
    lang::Value::Enum { variant_id: *RESULT_OK_VARIANT_ID,
                        value: Box::new(value) }
}

pub fn err_result_value(string: String) -> lang::Value {
    lang::Value::Enum { variant_id: *RESULT_ERROR_VARIANT_ID,
                        value: Box::new(lang::Value::String(string)) }
}

pub fn convert_lang_value_to_rust_result(value: &lang::Value)
                                         -> Result<&lang::Value, &lang::Value> {
    let (variant_id, inner_value) = value.as_enum().unwrap();
    if variant_id == *RESULT_OK_VARIANT_ID {
        Ok(inner_value)
    } else if variant_id == *RESULT_ERROR_VARIANT_ID {
        Err(inner_value)
    } else {
        panic!("that's an enum, but not a Result: {:?}", inner_value)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Print {}

lazy_static! {
    static ref PRINT_FUNC_ID: uuid::Uuid =
        uuid::Uuid::parse_str("b5c18d63-f9a0-4f08-8ee7-e35b3db9122d").unwrap();
    static ref PRINT_ARG_ID: uuid::Uuid =
        uuid::Uuid::parse_str("feff08f0-7319-4b47-964e-1f470eca81df").unwrap();
}

pub fn get_args<const N: usize>(mut arg_by_id: HashMap<lang::ID, lang::Value>,
                                arg_id: [lang::ID; N])
                                -> Result<[lang::Value; N], ExecutionError> {
    let mut args: [lang::Value; N] = array_init::array_init(|_| lang::Value::Null);
    for (i, arg_id) in arg_id.iter().enumerate() {
        match arg_by_id.remove(arg_id) {
            None => return Err(ExecutionError::ArgumentNotFound),
            Some(value) => args[i] = value,
        }
    }
    Ok(args)
}

fn get_string(v: lang::Value) -> Result<String, ExecutionError> {
    match v.into_string() {
        Ok(s) => Ok(s),
        Err(_) => Err(ExecutionError::ArgumentWrongType),
    }
}

#[typetag::serde]
impl lang::Function for Print {
    fn call(&self,
            interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        // TODO: not always gonna be an unwrap, we're going to change around the interpreter to
        // accept Result types
        let [arg] = get_args(args, [*PRINT_ARG_ID]).unwrap();
        let string = get_string(arg).unwrap();
        interpreter.env.borrow_mut().println(&string);
        lang::Value::Null
    }

    fn name(&self) -> &str {
        "Debug"
    }

    fn description(&self) -> &str {
        "Print output to the debug console for debugging only"
    }

    fn id(&self) -> lang::ID {
        *PRINT_FUNC_ID
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![lang::ArgumentDefinition::new_with_id(*PRINT_ARG_ID,
                                                   lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                                                   "Text".to_string())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NULL_TYPESPEC)
    }
}

lazy_static! {
    static ref CAPITALIZE_FUNC_ID: uuid::Uuid =
        uuid::Uuid::parse_str("86ae2a51-5538-436f-b48e-3aa6c873b189").unwrap();
    static ref CAPITALIZE_ARG_ID: uuid::Uuid =
        uuid::Uuid::parse_str("94e81ddc-843b-426d-847e-a215125c9593").unwrap();
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Capitalize {}

#[typetag::serde]
impl lang::Function for Capitalize {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let [arg] = get_args(args, [*CAPITALIZE_ARG_ID]).unwrap();
        let string = get_string(arg).unwrap();
        lang::Value::String(string.to_uppercase())
    }

    fn name(&self) -> &str {
        "Capitalize"
    }

    fn description(&self) -> &str {
        "Capitalize every character in text"
    }

    fn id(&self) -> lang::ID {
        *CAPITALIZE_FUNC_ID
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![lang::ArgumentDefinition::new_with_id(*CAPITALIZE_ARG_ID,
                                                   lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                                                   "String to capitalize".to_string())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::STRING_TYPESPEC)
    }
}

pub fn new_option(some_type: lang::Type) -> lang::Type {
    lang::Type { typespec_id: *OPTION_ENUM_ID,
                 params: vec![some_type] }
}

pub fn new_result_with_null_error(ok_type: lang::Type) -> lang::Type {
    let null_type = lang::Type::from_spec(&*lang::NULL_TYPESPEC);
    new_result(ok_type, null_type)
}

pub fn new_result(ok_type: lang::Type, error_type: lang::Type) -> lang::Type {
    lang::Type { typespec_id: *RESULT_ENUM_ID,
                 params: vec![ok_type, error_type] }
}

pub fn get_ok_type_from_result_type(result_type: &lang::Type) -> &lang::Type {
    if result_type.typespec_id != *RESULT_ENUM_ID {
        panic!("wrong typespec ID")
    }
    &result_type.params[0]
}

pub fn get_error_type_from_result_type(result_type: &lang::Type) -> &lang::Type {
    if result_type.typespec_id != *RESULT_ENUM_ID {
        panic!("wrong typespec ID")
    }
    &result_type.params[1]
}

#[derive(Clone)]
pub struct ChatReply {
    pub output_buffer: Arc<Mutex<Vec<String>>>,
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

    fn description(&self) -> &str {
        "Send a message back to where this program was initiated from. If initiated from a private message or a chat room, it'll go there."
    }

    fn id(&self) -> lang::ID {
        *CHAT_REPLY_FUNC_ID
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![lang::ArgumentDefinition::new_with_id(*CHAT_REPLY_MESSAGE_ARG_ID,
                                                   lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                                                   "Message".to_string()),]
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

    fn description(&self) -> &str {
        "Combine multiple texts into a single one"
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

#[derive(Clone, Serialize, Deserialize)]
pub struct SplitString {}

#[typetag::serde]
impl lang::Function for SplitString {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let str = args.get(&self.takes_args()[0].id)
                      .unwrap()
                      .as_str()
                      .unwrap();
        let delimiter = args.get(&self.takes_args()[1].id)
                            .unwrap()
                            .as_str()
                            .unwrap();
        let strings = str.split(delimiter)
                         .map(|str| lang::Value::String(str.to_string()))
                         .collect();
        lang::Value::List(lang::Type::from_spec(&*lang::STRING_TYPESPEC), strings)
    }

    fn name(&self) -> &str {
        "Split"
    }

    fn description(&self) -> &str {
        "Split a string into a list by a delimiter"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("2a6af5fe-8512-4d03-a018-a549c10cac8a").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("401e29a6-afd6-4868-913c-83bef61e9783").unwrap(),
                lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                "String to split".to_string()),
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("ad4e23c5-233b-466c-b0f5-7662e832adf1").unwrap(),
                lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                "Delimiter".to_string()),
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::with_params(&*lang::LIST_TYPESPEC,
                                vec![lang::Type::from_spec(&*lang::STRING_TYPESPEC)])
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Map {}

#[typetag::serde]
impl lang::Function for Map {
    fn call(&self,
            interpreter: env::Interpreter,
            mut args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let what_to_map_over = args.remove(&self.takes_args()[0].id)
                                   .unwrap()
                                   .into_vec()
                                   .unwrap();
        let map_fn = args.remove(&self.takes_args()[1].id)
                         .unwrap()
                         .into_anon_func()
                         .unwrap();
        lang::Value::new_future(async move {
            let mapped =
                what_to_map_over.into_iter()
                                .map(|value| {
                                    let mut interpreter = interpreter.shallow_copy();
                                    let map_fn = map_fn.clone();
                                    async move {
                                        interpreter.set_local_variable(map_fn.takes_arg.id,
                                                                       value);
                                        await_eval_result!(interpreter.evaluate(map_fn.block
                                                                                      .as_ref()))
                                    }
                                });
            let joined = join_all(mapped).await;
            lang::Value::List(map_fn.returns, joined)
        })
    }

    fn name(&self) -> &str {
        "Transform (Map)"
    }

    fn description(&self) -> &str {
        "Transforms a List of items into a new List, applying a function to each element. This is sometimes called \"Map\" in other programming languages."
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("7add6e8d-0f89-4958-a435-bad3c9066927").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("75ac0660-6814-4d17-8444-481237581f16").unwrap(),
                lang::Type::with_params(&*lang::LIST_TYPESPEC,
                                    vec![lang::Type::from_spec(&*lang::STRING_TYPESPEC)]),
                "Collection".to_string()),
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("1ae3bf22-a2ab-4bd0-af5f-193385284b7d").unwrap(),
                lang::Type::with_params(&*lang::ANON_FUNC_TYPESPEC,
                                    vec![lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                                        lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
                                    ]),
                "Transformation".to_string()),
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::with_params(&*lang::LIST_TYPESPEC,
                                vec![lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)])
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ParseNumber {}

#[typetag::serde]
impl lang::Function for ParseNumber {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let val = args.get(&self.takes_args()[0].id).unwrap();
        let num = val.as_str().unwrap().parse().unwrap_or(0);
        lang::Value::Number(num)
    }

    fn name(&self) -> &str {
        "ParseNumber"
    }

    fn description(&self) -> &str {
        "Turns a String into a Number, returns 0 if it can't parse (TODO FIXME)"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("fd49253c-3661-413f-b78c-25f20f8e3473").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![
            lang::ArgumentDefinition::new_with_id(
                uuid::Uuid::parse_str("f99f9d51-2ec7-4fce-9471-14b4c800110b").unwrap(),
                lang::Type::from_spec(&*lang::STRING_TYPESPEC), "String to convert".into())
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DivideTemp {}

#[typetag::serde]
impl lang::Function for DivideTemp {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let dividend = args.get(&self.takes_args()[0].id)
                           .unwrap()
                           .as_i128()
                           .unwrap();
        let divisor = args.get(&self.takes_args()[1].id)
                          .unwrap()
                          .as_i128()
                          .unwrap();
        lang::Value::Number(dividend / divisor)
    }

    fn name(&self) -> &str {
        "DivideTemp"
    }

    fn description(&self) -> &str {
        "Does Division"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("d1943888-27bc-40da-9756-e25da8584f96").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![lang::ArgumentDefinition::new_with_id(uuid::Uuid::parse_str("fea14b3b-71dd-4907-88b9-ee1a857937ef").unwrap(),
                                                   lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
                                                   "Dividend".into()),
        lang::ArgumentDefinition::new_with_id(uuid::Uuid::parse_str("88afb818-e741-48e4-8550-7a29aaf4b500").unwrap(),
                                                   lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
                                                   "Divisor".into())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Subtract {}

#[typetag::serde]
impl lang::Function for Subtract {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        // TODO: fix the names
        let dividend = args.get(&self.takes_args()[0].id)
                           .unwrap()
                           .as_i128()
                           .unwrap();
        let divisor = args.get(&self.takes_args()[1].id)
                          .unwrap()
                          .as_i128()
                          .unwrap();
        lang::Value::Number(dividend - divisor)
    }

    fn name(&self) -> &str {
        "Subtract"
    }

    fn description(&self) -> &str {
        "Does subtraction"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("a54e21c5-d20f-4a46-98ca-fede6474d9c7").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![lang::ArgumentDefinition::new_with_id(uuid::Uuid::parse_str("2563941c-b8aa-4e22-9081-d7507d01f575").unwrap(),
                                                   lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
                                                   "Minuend".into()),
             lang::ArgumentDefinition::new_with_id(uuid::Uuid::parse_str("1b5a2487-0cb8-4101-b184-a9b61d154e2a").unwrap(),
                                                   lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
                                                   "Subtrahen".into())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SumList {}

#[typetag::serde]
impl lang::Function for SumList {
    fn call(&self,
            _interpreter: env::Interpreter,
            mut args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let what_to_map_over = args.remove(&self.takes_args()[0].id)
                                   .unwrap()
                                   .into_vec()
                                   .unwrap()
                                   .into_iter()
                                   .map(|value| value.as_i128().unwrap())
                                   .sum();
        lang::Value::Number(what_to_map_over)
    }

    fn name(&self) -> &str {
        "SumList"
    }

    fn description(&self) -> &str {
        "Add all the numbers together"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("a35bb47b-2660-4c90-a7c5-d015ea6954cb").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![lang::ArgumentDefinition::new_with_id(uuid::Uuid::parse_str("c68fa262-5ea7-4f2f-8c2c-4838cf0959b1").unwrap(),
                                                   lang::Type::with_params(&*lang::LIST_TYPESPEC,
                                                                           vec![lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)]),
                                                   "Numbers".into())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)
    }
}
