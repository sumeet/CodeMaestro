use std::collections::HashSet;

use super::chat_program::ChatProgram;
use super::code_function;
use super::enums;
use super::env;
use super::json_http_client::JSONHTTPClient;
use super::jsstuff;
use super::lang;
use super::lang::Function;
use super::pystuff;
use super::structs;

use crate::lang::Value;
use itertools::Itertools;

pub struct TypeDisplayInfo {
    pub name: String,
    pub symbol: String,
}

pub struct EnvGenie<'a> {
    pub env: &'a env::ExecutionEnvironment,
}

impl<'a> EnvGenie<'a> {
    pub fn new(env: &'a env::ExecutionEnvironment) -> Self {
        Self { env }
    }

    pub fn get_last_executed_result(&self, code_node_id: lang::ID) -> Option<&lang::Value> {
        self.env.prev_eval_result_by_code_id.get(&code_node_id)
    }

    pub fn guess_type_of_value(&self, value: &lang::Value) -> lang::Type {
        match value {
            Value::Null => lang::Type::from_spec(&*lang::NULL_TYPESPEC),
            Value::Boolean(_) => lang::Type::from_spec(&*lang::BOOLEAN_TYPESPEC),
            Value::String(_) => lang::Type::from_spec(&*lang::STRING_TYPESPEC),
            Value::Number(_) => lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
            Value::List(list_of_type, _val) => {
                lang::Type::from_spec_id(lang::LIST_TYPESPEC.id, vec![list_of_type.clone()])
            }
            // this may need to change if structs can have generics
            Value::Struct { struct_id,
                            values: _values, } => lang::Type::from_spec_id(*struct_id, vec![]),
            Value::Future(_) => {
                panic!("currently unimplemented for futures, not sure if we'll ever need this")
            }
            Value::Shared(value) => self.guess_type_of_value(&value.borrow()),
            Value::EnumVariant { variant_id: _variant_id,
                                 value, } => self.guess_type_of_value(value),
            Value::AnonymousFunction(anonymous_function) => {
                lang::Type::with_params(&*lang::ANON_FUNC_TYPESPEC,
                                        vec![anonymous_function.takes_arg.arg_type.clone(),
                                             anonymous_function.returns.clone(),])
            }
        }
    }

    // TODO: this could be faster
    pub fn get_type_display_info(&self, typ: &lang::Type) -> Option<TypeDisplayInfo> {
        Some(TypeDisplayInfo { symbol: self.get_symbol_for_type(typ),
                               name: self.get_name_for_type(typ)? })
    }

    pub fn get_symbol_for_type(&self, t: &lang::Type) -> String {
        let typespec = self.find_typespec(t.typespec_id).unwrap();
        if typespec.num_params() == 0 {
            return typespec.symbol().to_string();
        }
        let joined_params = t.params
                             .iter()
                             .map(|p| self.get_symbol_for_type(p))
                             .join(", ");
        format!("{} {}", typespec.symbol(), joined_params)
    }

    pub fn get_name_for_type(&self, t: &lang::Type) -> Option<String> {
        let typespec = self.find_typespec(t.typespec_id)?;
        if typespec.num_params() == 0 {
            return Some(typespec.readable_name().to_string());
        }
        let joined_params = t.params
                             .iter()
                             .map(|p| {
                                 self.get_name_for_type(p)
                                     .unwrap_or_else(|| "(UNKNOWN NAME)".to_string())
                             })
                             .join(", ");
        Some(format!("{}\u{f053}{}\u{f054}",
                     typespec.readable_name(),
                     joined_params))
    }

    // ONEDAY: this search could be made faster if we kept an index!
    pub fn find_struct_field(&self, struct_field_id: lang::ID) -> Option<&structs::StructField> {
        self.list_structs()
            .flat_map(|strukt| &strukt.fields)
            .find(|field| field.id == struct_field_id)
    }

    pub fn get_struct_and_field(&self,
                                struct_id: lang::ID,
                                struct_field_id: lang::ID)
                                -> Option<(&structs::Struct, &structs::StructField)> {
        let strukt = self.find_struct(struct_id)?;
        let field = strukt.fields
                          .iter()
                          .find(|field| field.id == struct_field_id)?;
        Some((strukt, field))
    }

    pub fn find_struct_and_field(&self,
                                 struct_field_id: lang::ID)
                                 -> Option<(&structs::Struct, &structs::StructField)> {
        self.list_structs()
            .flat_map(|strukt| strukt.fields.iter().map(move |field| (strukt, field)))
            .find(|(_strukt, field)| field.id == struct_field_id)
    }

    pub fn find_typespec(&self, id: lang::ID) -> Option<&Box<dyn lang::TypeSpec>> {
        self.env.find_typespec(id)
    }

    pub fn find_function(&self, id: lang::ID) -> Option<&Box<dyn lang::Function>> {
        self.env.find_function(id)
    }

    pub fn get_code_func(&self, id: lang::ID) -> Option<&code_function::CodeFunction> {
        self.env
            .find_function(id)
            .and_then(|f| f.downcast_ref::<code_function::CodeFunction>())
    }

    pub fn get_json_http_client(&self, id: lang::ID) -> Option<&JSONHTTPClient> {
        self.env
            .find_function(id)
            .and_then(|f| f.downcast_ref::<JSONHTTPClient>())
    }

    pub fn get_chat_program(&self, id: lang::ID) -> Option<&ChatProgram> {
        self.env
            .find_function(id)
            .and_then(|f| f.downcast_ref::<ChatProgram>())
    }

    pub fn find_struct(&self, id: lang::ID) -> Option<&structs::Struct> {
        self.env.find_struct(id)
    }

    pub fn list_structs(&self) -> impl Iterator<Item = &structs::Struct> {
        self.typespecs()
            .filter_map(|ts| ts.downcast_ref::<structs::Struct>())
    }

    pub fn list_enums(&self) -> impl Iterator<Item = &enums::Enum> {
        self.typespecs()
            .filter_map(|ts| ts.as_ref().downcast_ref::<enums::Enum>())
    }

    pub fn find_enum_variant(&self, variant_id: lang::ID) -> Option<&enums::EnumVariant> {
        for eneom in self.list_enums() {
            for variant in &eneom.variants {
                if variant.id == variant_id {
                    return Some(variant);
                }
            }
        }
        None
    }

    pub fn find_enum(&self, enum_id: lang::ID) -> Option<&enums::Enum> {
        self.find_typespec(enum_id)
            .and_then(|ts| ts.downcast_ref::<enums::Enum>())
    }

    pub fn all_functions(&self) -> impl Iterator<Item = &Box<dyn lang::Function>> {
        self.env.list_functions()
    }

    pub fn list_jsfuncs(&self) -> impl Iterator<Item = &jsstuff::JSFunc> {
        self.env
            .list_functions()
            .filter_map(|f| f.downcast_ref::<jsstuff::JSFunc>())
    }

    pub fn list_code_funcs(&self) -> impl Iterator<Item = &code_function::CodeFunction> {
        self.env
            .list_functions()
            .filter_map(|f| f.downcast_ref::<code_function::CodeFunction>())
    }

    pub fn list_json_http_clients(&self) -> impl Iterator<Item = &JSONHTTPClient> {
        self.all_functions()
            .filter_map(|f| f.downcast_ref::<JSONHTTPClient>())
    }

    pub fn list_chat_programs(&self) -> impl Iterator<Item = &ChatProgram> {
        self.all_functions()
            .filter_map(|f| f.downcast_ref::<ChatProgram>())
    }

    pub fn list_pyfuncs(&self) -> impl Iterator<Item = &pystuff::PyFunc> {
        self.env
            .list_functions()
            .filter_map(|f| f.downcast_ref::<pystuff::PyFunc>())
    }

    pub fn read_console(&self) -> &str {
        &self.env.console
    }

    // this whole machinery cannot handle parameterized types yet :/
    pub fn find_types_matching(&'a self, str: &'a str) -> impl Iterator<Item = lang::Type> + 'a {
        self.env
            .list_typespecs()
            .filter(|ts| ts.num_params() == 0)
            .filter(move |ts| ts.readable_name().to_lowercase().contains(str))
            .map(|ts| lang::Type::from_spec_id(ts.id(), vec![]))
    }

    // TODO: probably this should filter out builtins too, because the two callers of this are
    // filtering it out themselves. and then maybe rename this func to public_editable instead of
    // just public
    pub fn list_public_structs(&self) -> impl Iterator<Item = &structs::Struct> {
        let private_struct_ids = self.private_struct_ids();
        self.list_structs()
            .filter(move |strukt| private_struct_ids.contains(&strukt.id))
    }

    pub fn find_public_structs_matching(&'a self,
                                        str: &'a str)
                                        -> impl Iterator<Item = &structs::Struct> + 'a {
        self.list_public_structs()
            .filter(move |strukt| strukt.name.to_lowercase().contains(str))
    }

    pub fn typespecs(&self) -> impl Iterator<Item = &Box<dyn lang::TypeSpec>> {
        self.env.list_typespecs()
    }

    pub fn get_function_containing_arg(&self,
                                       argument_definition_id: lang::ID)
                                       -> Option<&dyn lang::Function> {
        self.iterate_all_function_arguments()
            .find(|(_, arg)| arg.id == argument_definition_id)
            .map(|arg_def| arg_def.0)
    }

    pub fn get_arg_definition(&self,
                              argument_definition_id: lang::ID)
                              -> Option<lang::ArgumentDefinition> {
        self.iterate_all_function_arguments()
            .find(|(_, arg)| arg.id == argument_definition_id)
            .map(|arg_def| arg_def.1)
    }

    // TODO: this is the place that needs to be changed
    pub fn get_type_for_arg(&self, argument_definition_id: lang::ID) -> Option<lang::Type> {
        let argument_definition = self.get_arg_definition(argument_definition_id)?;
        Some(argument_definition.arg_type)
    }

    pub fn iterate_all_function_arguments(
        &self)
        -> impl Iterator<Item = (&dyn lang::Function, lang::ArgumentDefinition)> + '_ {
        self.env.list_functions().flat_map(move |func| {
                                     let args_for_func = get_args_for_func(func.as_ref());
                                     args_for_func.into_iter()
                                                  .flat_map(move |(_block_id, get_args)| {
                                                      get_args(func.as_ref())
                                                  })
                                                  .map(move |arg| (func.as_ref(), arg))
                                 })
    }

    pub fn code_takes_args(&'a self,
                           root_id: lang::ID)
                           -> impl Iterator<Item = lang::ArgumentDefinition> + 'a {
        self.all_functions()
            .flat_map(move |f| get_args_for_code_block(root_id, f.as_ref()))
    }

    // TODO: save this cache somewhere, it's probably expensive to compute this on every frame
    fn private_struct_ids(&self) -> HashSet<lang::ID> {
        self.list_json_http_clients()
            .map(|client| &client.intermediate_parse_structs)
            .flat_map(|structs| structs.iter().map(|strukt| strukt.id))
            .collect()
    }
}

fn get_args_for_code_block(code_block_id: lang::ID,
                           function: &dyn lang::Function)
                           -> impl Iterator<Item = lang::ArgumentDefinition> {
    for (current_code_block_id, get_args) in get_args_for_func(function) {
        if current_code_block_id == Some(code_block_id) {
            return get_args(function).into_iter();
        }
    }
    vec![].into_iter()
}

// returns a vector containing tuples of (code block ID, and a rust function that takes that lang::Function,
// returns a vector of arguments -- the idea is to be lazy with evaluating those)
//
// and the ID (first value of tuple) is None if there's no code block associated with that function
// (for built-in functions written in Rust)
fn get_args_for_func(
    function: &dyn lang::Function)
    -> Vec<(Option<lang::ID>, &dyn Fn(&dyn lang::Function) -> Vec<lang::ArgumentDefinition>)> {
    if let Some(code_func) = function.downcast_ref::<code_function::CodeFunction>() {
        vec![(Some(code_func.code_id()),
              &|function| {
                  let code_func = function.downcast_ref::<code_function::CodeFunction>()
                                          .unwrap();
                  code_func.takes_args()
              })]
    } else if let Some(json_http_client) = function.downcast_ref::<JSONHTTPClient>() {
        vec![(Some(json_http_client.gen_url_params_code.id),
              &|function| {
                  let json_http_client = function.downcast_ref::<JSONHTTPClient>().unwrap();
                  json_http_client.takes_args()
              }),
             (Some(json_http_client.gen_url_code.id),
              &|function| {
                  let json_http_client = function.downcast_ref::<JSONHTTPClient>().unwrap();
                  json_http_client.takes_args()
              }),
             (Some(json_http_client.transform_code.id),
              &|function| {
                  let json_http_client = function.downcast_ref::<JSONHTTPClient>().unwrap();
                  vec![json_http_client.intermediate_parse_argument.clone()]
              }),]
    } else if let Some(chat_program) = function.downcast_ref::<ChatProgram>() {
        vec![(Some(chat_program.code.id),
              &|function| {
                  let chat_program = function.downcast_ref::<ChatProgram>().unwrap();
                  chat_program.takes_args()
              }),]
    } else {
        vec![(None, &|function| function.takes_args())]
    }
}
