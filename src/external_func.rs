use super::env;
use super::lang;
use std::collections::HashMap;

pub trait ModifyableFunc: lang::Function {
    fn set_return_type(&mut self, return_type: lang::Type);
    fn set_args(&mut self, args: Vec<lang::ArgumentDefinition>);
    fn clone(&self) -> Self;
}

pub struct ValueWithEnv<'a> {
    pub value: lang::Value,
    pub env: &'a env::ExecutionEnvironment,
}

pub fn to_named_args(func: &lang::Function,
                     args: HashMap<lang::ID, lang::Value>) -> impl Iterator<Item=(String, lang::Value)>
{
    let mut short_name_by_id : HashMap<lang::ID, String> = func.takes_args().into_iter()
        .map(|argdef| (argdef.id, argdef.short_name))
        .collect();
    args.into_iter()
        .map(move |(arg_id, value)| {
            (short_name_by_id.remove(&arg_id).unwrap(), value)
        })
}
