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
use crate::lang::FunctionRenderingStyle;
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
        for ts in lang::BUILT_IN_TYPESPECS.iter().cloned() {
            builtins.typespecs.insert(ts.id(), ts);
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
    lang::Value::EnumVariant { variant_id: *OPTION_SOME_VARIANT_ID,
                               value: Box::new(value) }
}

pub fn none_option_value() -> lang::Value {
    lang::Value::EnumVariant { variant_id: *OPTION_NONE_VARIANT_ID,
                               value: Box::new(lang::Value::Null) }
}

pub fn ok_result_value(value: lang::Value) -> lang::Value {
    lang::Value::EnumVariant { variant_id: *RESULT_OK_VARIANT_ID,
                               value: Box::new(value) }
}

pub fn err_result_value(value: lang::Value) -> lang::Value {
    lang::Value::EnumVariant { variant_id: *RESULT_ERROR_VARIANT_ID,
                               value: Box::new(value) }
}

pub fn err_result_string(string: String) -> lang::Value {
    lang::Value::EnumVariant { variant_id: *RESULT_ERROR_VARIANT_ID,
                               value: Box::new(lang::Value::String(string)) }
}

pub fn convert_lang_value_to_rust_result(value: lang::Value) -> Result<lang::Value, lang::Value> {
    let (variant_id, inner_value) = value.into_enum().unwrap();
    if variant_id == *RESULT_OK_VARIANT_ID {
        Ok(inner_value)
    } else if variant_id == *RESULT_ERROR_VARIANT_ID {
        Err(inner_value)
    } else {
        panic!("that's an enum, but not a Result: {:?}", inner_value)
    }
}

pub fn convert_lang_value_as_rust_result(value: &lang::Value)
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

pub fn convert_lang_option_to_rust_option(value: lang::Value) -> Option<lang::Value> {
    let (variant_id, inner_value) = value.into_enum().unwrap();
    if variant_id == *OPTION_SOME_VARIANT_ID {
        Some(inner_value)
    } else if variant_id == *OPTION_NONE_VARIANT_ID {
        None
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

pub fn get_some_type_from_option_type(option_type: &lang::Type)
                                      -> Result<&lang::Type, &'static str> {
    if option_type.typespec_id != *OPTION_ENUM_ID {
        Err("wrong typespec ID")
    } else {
        Ok(&option_type.params[0])
    }
}

pub fn get_ok_type_from_result_type(result_type: &lang::Type) -> Result<&lang::Type, &'static str> {
    if result_type.typespec_id != *RESULT_ENUM_ID {
        Err("wrong typespec ID")
    } else {
        Ok(&result_type.params[0])
    }
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
            mut args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let text_to_send = args.remove(&CHAT_REPLY_MESSAGE_ARG_ID)
                               .unwrap()
                               .into_string()
                               .unwrap();
        self.output_buffer.lock().unwrap().push(text_to_send);
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

lazy_static! {
    static ref JOIN_STRING_ARG_IDS: [lang::ID; 1] =
        [uuid::Uuid::parse_str("78cf269a-2a29-4325-9a18-8d84132485ed").unwrap()];
}

#[typetag::serde]
impl lang::Function for JoinString {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let joined = args.get(&JOIN_STRING_ARG_IDS[0])
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
                JOIN_STRING_ARG_IDS[0],
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

lazy_static! {
    static ref SPLIT_STRING_ARG_IDS: [lang::ID; 2] =
        [uuid::Uuid::parse_str("401e29a6-afd6-4868-913c-83bef61e9783").unwrap(),
         uuid::Uuid::parse_str("ad4e23c5-233b-466c-b0f5-7662e832adf1").unwrap(),];
}

#[typetag::serde]
impl lang::Function for SplitString {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let str = args.get(&SPLIT_STRING_ARG_IDS[0])
                      .unwrap()
                      .as_str()
                      .unwrap();
        let delimiter = args.get(&SPLIT_STRING_ARG_IDS[1])
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
        vec![lang::ArgumentDefinition::new_with_id(SPLIT_STRING_ARG_IDS[0],
                                                   lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                                                   "String to split".to_string()),
             lang::ArgumentDefinition::new_with_id(SPLIT_STRING_ARG_IDS[1],
                                                   lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                                                   "Delimiter".to_string()),]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::with_params(&*lang::LIST_TYPESPEC,
                                vec![lang::Type::from_spec(&*lang::STRING_TYPESPEC)])
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Map {}

lazy_static! {
    static ref MAP_ARG_IDS: [lang::ID; 2] =
        [uuid::Uuid::parse_str("75ac0660-6814-4d17-8444-481237581f16").unwrap(),
         uuid::Uuid::parse_str("1ae3bf22-a2ab-4bd0-af5f-193385284b7d").unwrap(),];
}

#[typetag::serde]
impl lang::Function for Map {
    fn call(&self,
            interpreter: env::Interpreter,
            mut args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let what_to_map_over = args.remove(&MAP_ARG_IDS[0]).unwrap().into_vec().unwrap();
        let map_fn = args.remove(&MAP_ARG_IDS[1])
                         .unwrap()
                         .into_anon_func()
                         .unwrap();
        lang::Value::new_future(async move {
            let mapped =
                what_to_map_over.into_iter()
                                .map(|value| {
                                    let mut interpreter = interpreter.new_stack_frame();
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
        "Transform"
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
                MAP_ARG_IDS[0],
                lang::Type::with_params(&*lang::LIST_TYPESPEC,
                                    vec![lang::Type::from_spec(&*lang::STRING_TYPESPEC)]),
                "Collection".to_string()),
            lang::ArgumentDefinition::new_with_id(
                MAP_ARG_IDS[1],
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

lazy_static! {
    static ref PARSE_NUMBER_ARGS: [lang::ID; 1] =
        [uuid::Uuid::parse_str("f99f9d51-2ec7-4fce-9471-14b4c800110b").unwrap(),];
}

#[typetag::serde]
impl lang::Function for ParseNumber {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let val = args.get(&PARSE_NUMBER_ARGS[0]).unwrap();
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
        vec![lang::ArgumentDefinition::new_with_id(PARSE_NUMBER_ARGS[0],
                                                   lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                                                   "String to convert".into())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)
    }
}

lazy_static! {
    static ref DIVIDE_RENDERING_STYLE: lang::FunctionRenderingStyle =
        lang::FunctionRenderingStyle::Infix(vec![], "÷".to_string());
    static ref SUBTRACT_RENDERING_STYLE: lang::FunctionRenderingStyle =
        lang::FunctionRenderingStyle::Infix(vec![], "-".to_string());
    static ref MULTIPLY_RENDERING_STYLE: lang::FunctionRenderingStyle =
        lang::FunctionRenderingStyle::Infix(vec![], "×".to_string());
    static ref EQUALS_RENDERING_STYLE: lang::FunctionRenderingStyle =
        // this is a special unicode symbol
        lang::FunctionRenderingStyle::Infix(vec![], "⩵".to_string());
    static ref NOT_EQUALS_RENDERING_STYLE: lang::FunctionRenderingStyle =
        // this is a special unicode symbol
        lang::FunctionRenderingStyle::Infix(vec![], "\u{f53e}".to_string());
    static ref LESS_THAN_RENDERING_STYLE: lang::FunctionRenderingStyle =
        FunctionRenderingStyle::Infix(vec![], "\u{f536}".to_string());
}

#[derive(Clone, Serialize, Deserialize)]
pub struct DivideTemp {}

lazy_static! {
    static ref DIVIDE_ARGS: [lang::ID; 2] =
        [uuid::Uuid::parse_str("fea14b3b-71dd-4907-88b9-ee1a857937ef").unwrap(),
         uuid::Uuid::parse_str("88afb818-e741-48e4-8550-7a29aaf4b500").unwrap(),];
}

#[typetag::serde]
impl lang::Function for DivideTemp {
    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let dividend = args.get(&DIVIDE_ARGS[0]).unwrap().as_i128().unwrap();
        let divisor = args.get(&DIVIDE_ARGS[1]).unwrap().as_i128().unwrap();
        lang::Value::Number(dividend / divisor)
    }

    fn style(&self) -> &lang::FunctionRenderingStyle {
        &DIVIDE_RENDERING_STYLE
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
        vec![lang::ArgumentDefinition::new_with_id(DIVIDE_ARGS[0],
                                                   lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
                                                   "Dividend".into()),
             lang::ArgumentDefinition::new_with_id(DIVIDE_ARGS[1],
                                                   lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
                                                   "Divisor".into())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Subtract {}

lazy_static! {
    static ref SUBTRACT_ARGS: [lang::ID; 2] =
        [uuid::Uuid::parse_str("2563941c-b8aa-4e22-9081-d7507d01f575").unwrap(),
         uuid::Uuid::parse_str("1b5a2487-0cb8-4101-b184-a9b61d154e2a").unwrap(),];
}

#[typetag::serde]
impl lang::Function for Subtract {
    fn style(&self) -> &lang::FunctionRenderingStyle {
        &SUBTRACT_RENDERING_STYLE
    }

    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        // TODO: fix the names
        let dividend = args.get(&SUBTRACT_ARGS[0]).unwrap().as_i128().unwrap();
        let divisor = args.get(&SUBTRACT_ARGS[1]).unwrap().as_i128().unwrap();
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
        vec![lang::ArgumentDefinition::new_with_id(SUBTRACT_ARGS[0],
                                                   lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
                                                   "Minuend".into()),
             lang::ArgumentDefinition::new_with_id(SUBTRACT_ARGS[1],
                                                   lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
                                                   "Subtrahen".into())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Multiply {}

// TODO: see if this actually improvse the execution time after applying it all over
lazy_static! {
    static ref MULTIPLY_ARG_IDS: [uuid::Uuid; 2] =
        [uuid::Uuid::parse_str("83aa68ce-910a-40f5-87ee-73689b0f3287").unwrap(),
         uuid::Uuid::parse_str("7d3918e6-f3f2-4b80-b449-7b6f35e067fc").unwrap()];
}

#[typetag::serde]
impl lang::Function for Multiply {
    fn style(&self) -> &lang::FunctionRenderingStyle {
        &MULTIPLY_RENDERING_STYLE
    }

    fn call(&self,
            _interpreter: env::Interpreter,
            args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        // TODO: fix the names
        let dividend = args.get(&MULTIPLY_ARG_IDS[0]).unwrap().as_i128().unwrap();
        let divisor = args.get(&MULTIPLY_ARG_IDS[1]).unwrap().as_i128().unwrap();
        lang::Value::Number(dividend * divisor)
    }

    fn name(&self) -> &str {
        "Multiply"
    }

    fn description(&self) -> &str {
        "Multiply two numbers"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("092e3cec-954d-47f2-9574-075624311297").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![lang::ArgumentDefinition::new_with_id(MULTIPLY_ARG_IDS[0],
                                                   lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
                                                   "Multiplier".into()),
             lang::ArgumentDefinition::new_with_id(MULTIPLY_ARG_IDS[1],
                                                   lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
                                                   "Multiplicand".into())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Sum {}

lazy_static! {
    static ref SUM_ARGS: [lang::ID; 1] =
        [uuid::Uuid::parse_str("c68fa262-5ea7-4f2f-8c2c-4838cf0959b1").unwrap(),];
}

#[typetag::serde]
impl lang::Function for Sum {
    fn call(&self,
            _interpreter: env::Interpreter,
            mut args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let what_to_map_over = args.remove(&SUM_ARGS[0])
                                   .unwrap()
                                   .into_vec()
                                   .unwrap()
                                   .into_iter()
                                   .map(|value| value.as_i128().unwrap())
                                   .sum();
        lang::Value::Number(what_to_map_over)
    }

    fn name(&self) -> &str {
        "Sum"
    }

    fn description(&self) -> &str {
        "Add all the numbers together"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("a35bb47b-2660-4c90-a7c5-d015ea6954cb").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![lang::ArgumentDefinition::new_with_id(SUM_ARGS[0],
                                                   lang::Type::with_params(&*lang::LIST_TYPESPEC,
                                                                           vec![lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)]),
                                                   "Numbers".into())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::NUMBER_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Equals {}

lazy_static! {
    static ref EQUALS_ARGS: [lang::ID; 2] =
        [uuid::Uuid::parse_str("c5effe9a-e4e9-4c58-ba35-73b59e8b3368").unwrap(),
         uuid::Uuid::parse_str("fe065a78-e84f-4365-8e3f-06331f8f2241").unwrap(),];
}

#[typetag::serde]
impl lang::Function for Equals {
    fn call(&self,
            _interpreter: env::Interpreter,
            mut args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let lhs = args.remove(&EQUALS_ARGS[0]);
        let rhs = args.remove(&EQUALS_ARGS[1]);
        lang::Value::Boolean(lhs == rhs)
    }

    fn style(&self) -> &lang::FunctionRenderingStyle {
        &EQUALS_RENDERING_STYLE
    }

    fn name(&self) -> &str {
        "Equals"
    }

    fn description(&self) -> &str {
        "Test if both sides have the same value"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("7809f2da-0ae1-4181-8fd9-b72b27fe7aa4").unwrap()
    }

    fn defines_generics(&self) -> Vec<lang::GenericParamTypeSpec> {
        vec![lang::GenericParamTypeSpec::new(uuid::Uuid::parse_str("98928b40-5aff-48df-ba9e-6871fd8c81a0").unwrap())]
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        let generic = self.defines_generics().pop().unwrap();

        vec![lang::ArgumentDefinition::new_with_id(EQUALS_ARGS[0],
                                                   lang::Type::with_params(&generic, vec![]),
                                                   "LHS".into()),
             lang::ArgumentDefinition::new_with_id(EQUALS_ARGS[1],
                                                   lang::Type::with_params(&generic, vec![]),
                                                   "RHS".into())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::BOOLEAN_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct NotEquals {}

lazy_static! {
    static ref NOT_EQUALS_ARGS: [lang::ID; 2] =
        [uuid::Uuid::parse_str("bfbdc6ce-8391-4cc5-a54b-39e80c19daf0").unwrap(),
         uuid::Uuid::parse_str("bab97245-01e9-42fd-a2a4-19e29df865c8").unwrap(),];
}

#[typetag::serde]
impl lang::Function for NotEquals {
    fn call(&self,
            _interpreter: env::Interpreter,
            mut args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let lhs = args.remove(&EQUALS_ARGS[0]);
        let rhs = args.remove(&EQUALS_ARGS[1]);
        lang::Value::Boolean(lhs != rhs)
    }

    fn style(&self) -> &lang::FunctionRenderingStyle {
        &NOT_EQUALS_RENDERING_STYLE
    }

    fn name(&self) -> &str {
        "NotEquals"
    }

    fn description(&self) -> &str {
        "Test if both sides have different values"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("6ef7f8e9-a4c3-49d0-be0c-eefe0ff852c7").unwrap()
    }

    fn defines_generics(&self) -> Vec<lang::GenericParamTypeSpec> {
        vec![lang::GenericParamTypeSpec::new(uuid::Uuid::parse_str("35f8392d-10f0-4ddf-ab6d-d5c144283e79").unwrap())]
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        let generic = self.defines_generics().pop().unwrap();

        vec![lang::ArgumentDefinition::new_with_id(NOT_EQUALS_ARGS[0],
                                                   lang::Type::with_params(&generic, vec![]),
                                                   "LHS".into()),
             lang::ArgumentDefinition::new_with_id(NOT_EQUALS_ARGS[1],
                                                   lang::Type::with_params(&generic, vec![]),
                                                   "RHS".into())]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::BOOLEAN_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LessThan {}

lazy_static! {
    static ref LESS_THAN_ARGS: [lang::ID; 2] =
        [uuid::Uuid::parse_str("47f4813c-9549-4e7e-98d5-a2eeeca5bfa3").unwrap(),
         uuid::Uuid::parse_str("e4a7afad-2f81-4d21-a17c-0e0cd38d1c19").unwrap(),];
}

#[typetag::serde]
impl lang::Function for LessThan {
    fn call(&self,
            _interpreter: env::Interpreter,
            mut args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let lhs = args.remove(&LESS_THAN_ARGS[0]).unwrap().as_i128().unwrap();
        let rhs = args.remove(&LESS_THAN_ARGS[1]).unwrap().as_i128().unwrap();
        lang::Value::Boolean(lhs < rhs)
    }

    fn style(&self) -> &lang::FunctionRenderingStyle {
        &LESS_THAN_RENDERING_STYLE
    }

    fn name(&self) -> &str {
        "Less than"
    }

    fn description(&self) -> &str {
        "Test if Left is less than Right"
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("9072ddc8-3e47-4874-adfd-3d564b4c4430").unwrap()
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        vec![lang::ArgumentDefinition::new_with_id(LESS_THAN_ARGS[0],
                                                   lang::Type::with_params(&*lang::NUMBER_TYPESPEC,
                                                                           vec![]),
                                                   "Left".into()),
             lang::ArgumentDefinition::new_with_id(LESS_THAN_ARGS[1],
                                                   lang::Type::with_params(&*lang::NUMBER_TYPESPEC,
                                                                           vec![]),
                                                   "Right".into())
        ]
    }

    fn returns(&self) -> lang::Type {
        lang::Type::from_spec(&*lang::BOOLEAN_TYPESPEC)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Range {}

lazy_static! {
    static ref RANGE_ARGS: [lang::ID; 3] =
        [uuid::Uuid::parse_str("55e37864-34d7-4792-97d9-09ec14340528").unwrap(),
         uuid::Uuid::parse_str("8760d197-4570-439b-bfad-622913fb7d59").unwrap(),
         uuid::Uuid::parse_str("e41d4ebd-f3c3-4924-81fc-f83c4fd06229").unwrap(),];
}

#[typetag::serde]
impl lang::Function for Range {
    fn call(&self,
            _interpreter: env::Interpreter,
            mut args: HashMap<lang::ID, lang::Value>)
            -> lang::Value {
        let (typ, vec) = args.remove(&RANGE_ARGS[0])
                             .unwrap()
                             .into_vec_with_type()
                             .unwrap();
        let mut range_lo = args.remove(&RANGE_ARGS[1]).unwrap().as_i128().unwrap();
        let mut range_hi = args.remove(&RANGE_ARGS[2]).unwrap().as_i128().unwrap();

        let len = vec.len();
        if range_lo < 0 {
            range_lo = len as i128 - range_lo
        }
        if range_hi < 0 {
            range_hi = len as i128 - range_hi
        }

        lang::Value::List(typ,
                          vec.get(range_lo as usize..=range_hi as usize)
                             .unwrap_or(&[])
                             .to_vec())
    }

    fn name(&self) -> &str {
        "Range"
    }

    fn description(&self) -> &str {
        "Select an inclusive range from a List. 0 is the first element, 1 is the second, -1 is the last, -2 is the second last, etc."
    }

    fn id(&self) -> lang::ID {
        uuid::Uuid::parse_str("6233cd92-b16c-488c-8a6b-4679a2d38633").unwrap()
    }

    fn defines_generics(&self) -> Vec<lang::GenericParamTypeSpec> {
        vec![lang::GenericParamTypeSpec::new(uuid::Uuid::parse_str("c42cd06f-69b4-40f6-8e8c-4af2526b19c3").unwrap())]
    }

    fn takes_args(&self) -> Vec<lang::ArgumentDefinition> {
        let generic = self.defines_generics().pop().unwrap();

        vec![lang::ArgumentDefinition::new_with_id(RANGE_ARGS[0],
                                                   lang::Type::with_params(&*lang::LIST_TYPESPEC,
                                                                           vec![lang::Type::from_spec(&generic)]),
                                                   "Collection".into()),
             lang::ArgumentDefinition::new_with_id(RANGE_ARGS[1],
                                                   lang::Type::with_params(&*lang::NUMBER_TYPESPEC,
                                                                           vec![]),
                                                   "Low".into()),
             lang::ArgumentDefinition::new_with_id(RANGE_ARGS[2],
                                                   lang::Type::with_params(&*lang::NUMBER_TYPESPEC,
                                                                           vec![]),
                                                   "High".into())
        ]
    }

    fn returns(&self) -> lang::Type {
        let generic = self.defines_generics().pop().unwrap();
        lang::Type::with_params(&*lang::LIST_TYPESPEC, vec![lang::Type::from_spec(&generic)])
    }
}
