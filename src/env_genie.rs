use super::enums;
use super::env;
use super::lang;
use super::structs;
use super::code_function;
use super::pystuff;
use super::jsstuff;

use itertools::Itertools;

pub struct EnvGenie<'a> {
    env: &'a env::ExecutionEnvironment,
}

impl<'a> EnvGenie<'a> {
    pub fn new(env: &'a env::ExecutionEnvironment) -> Self {
        Self { env }
    }

    pub fn get_symbol_for_type(&self, t: &lang::Type) -> String {
        let typespec = self.find_typespec(t.typespec_id).unwrap();
        if typespec.num_params() == 0 {
            return typespec.symbol().to_string()
        }
        let joined_params = t.params.iter()
            .map(|p| self.get_symbol_for_type(p))
            .join(", ");
        format!("{}\u{f053}{}\u{f054}", typespec.symbol(), joined_params)
    }

    pub fn get_type_for_arg(&self, argument_definition_id: lang::ID) -> Option<lang::Type> {
        for function in self.env.list_functions() {
            for arg_def in function.takes_args() {
                if arg_def.id == argument_definition_id {
                    return Some(arg_def.arg_type)
                }
            }
        }
        None
    }

    pub fn find_typespec(&self, id: lang::ID) -> Option<&Box<lang::TypeSpec>> {
        self.env.find_typespec(id)
    }

    pub fn find_function(&self, id: lang::ID) -> Option<&Box<lang::Function>> {
        self.env.find_function(id)
    }

    pub fn find_struct(&self, id: lang::ID) -> Option<&structs::Struct> {
        self.env.find_struct(id)
    }

    pub fn list_structs(&self) -> impl Iterator<Item = &structs::Struct> {
        self.typespecs()
            .filter_map(|ts| ts.as_ref().downcast_ref::<structs::Struct>())
    }

    pub fn list_enums(&self) -> impl Iterator<Item = &enums::Enum> {
        self.typespecs()
            .filter_map(|ts| ts.as_ref().downcast_ref::<enums::Enum>())
    }

    pub fn all_functions(&self) -> impl Iterator<Item = &Box<lang::Function>> {
        self.env.list_functions()
    }

    pub fn list_jsfuncs(&self) -> impl Iterator<Item = &jsstuff::JSFunc> {
        self.env.list_functions()
            .filter_map(|f| f.downcast_ref::<jsstuff::JSFunc>())
    }

    pub fn list_code_funcs(&self) -> impl Iterator<Item = &code_function::CodeFunction> {
        self.env.list_functions()
            .filter_map(|f| f.downcast_ref::<code_function::CodeFunction>())
    }

    pub fn list_pyfuncs(&self) -> impl Iterator<Item = &pystuff::PyFunc> {
        self.env.list_functions()
            .filter_map(|f| f.downcast_ref::<pystuff::PyFunc>())
    }

    pub fn read_console(&self) -> &str {
        &self.env.console
    }

    // this whole machinery cannot handle parameterized types yet :/
    pub fn find_types_matching(&'a self, str: &'a str) -> impl Iterator<Item = lang::Type> + 'a {
        self.env.list_typespecs()
            .filter(|ts| ts.num_params() == 0)
            .filter(move |ts| ts.readable_name().to_lowercase().contains(str))
            .map(|ts| lang::Type::from_spec_id(ts.id(), vec![]))
    }

    pub fn typespecs(&self) -> impl Iterator<Item = &Box<lang::TypeSpec>> {
        self.env.list_typespecs()
    }

    pub fn get_arg_definition(&self, argument_definition_id: lang::ID) -> Option<lang::ArgumentDefinition> {
        for function in self.all_functions() {
            for arg_def in function.takes_args() {
                if arg_def.id == argument_definition_id {
                    return Some(arg_def)
                }
            }
        }
        None
    }
}

