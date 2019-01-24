use super::env;
use super::lang;
use super::function;
use std::collections::HashMap;
use objekt::{clone_trait_object,__internal_clone_trait_object};

pub trait ModifyableFunc: objekt::Clone + lang::Function + function::SettableArgs {
    fn set_return_type(&mut self, return_type: lang::Type);
}

clone_trait_object!(ModifyableFunc);

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

pub fn resolve_futures(value: lang::Value) -> lang::Value {
    lang::Value::new_future(async {
        match value {
            lang::Value::Future(value_future) => {
                // need to recursive call here because even after resolving the
                // future, the Value could contain MORE nested futures!
                resolve_futures(await!(value_future))
            }
            lang::Value::List(v) => {
                lang::Value::List(v.into_iter().map(resolve_futures).collect())
            },
            lang::Value::Struct { values, struct_id } => {
                lang::Value::Struct {
                    struct_id,
                    values: values.into_iter().map(|(value_id, value)| {
                        (value_id, resolve_futures(value))
                    }).collect()
                }
            },
            lang::Value::Null | lang::Value::String(_) | lang::Value::Error(_) |
            lang::Value::Number(_) | lang::Value::Boolean(_) => value
        }
    })
}

pub async fn resolve_all_futures(mut val: lang::Value) -> lang::Value {
    while contains_futures(&val) {
        val = resolve_futures(val);
        val = match val {
            lang::Value::Future(value_future) => await!(value_future),
            _ => val,
        }
    }
    val
}

fn contains_futures(val: &lang::Value) -> bool {
    match val {
        lang::Value::Future(_value_future) => {
            // need to recursive call here because even after resolving the
            // future, the Value could contain MORE nested futures!
            true
        }
        lang::Value::List(v) => {
            v.iter().any(contains_futures)
        },
        lang::Value::Struct { values, .. } => {
            values.iter().any(|(_id, val)| contains_futures(val))
        },
        lang::Value::Null | lang::Value::String(_) | lang::Value::Error(_) |
        lang::Value::Number(_) | lang::Value::Boolean(_) => false
    }
}