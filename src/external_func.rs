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

pub async fn resolve_futures(value: lang::Value) -> lang::Value {
    match value {
        lang::Value::Future(value_future) => {
            // need to recursive call here because even after resolving the
            // future, the Value could contain MORE nested futures!
//            await!(resolve_futures(await!(value_future)))
        }
        lang::Value::List(vs) => {
            let mut o = vec![];
            for v in vs.into_iter() {
                o.push(await!(resolve_futures(v)))
            }
            lang::Value::List(o)
        },
        lang::Value::Struct { values, struct_id } => {
            let mut resolved_values = HashMap::new();
            for (value_id, value) in values.into_iter() {
                resolved_values.insert(value_id, await!(resolve_futures(value)));
            }
            lang::Value::Struct {
                struct_id,
                values: resolved_values,
            }
        },
        lang::Value::Null | lang::Value::String(_) | lang::Value::Error(_) |
         lang::Value::Number(_) => value
    }
}

pub async fn preresolve_futures_if_external_func(func: Option<Box<lang::Function + 'static>>,
                                                 value: lang::Value) -> lang::Value {
    unimplemented!()
}