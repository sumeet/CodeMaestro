use super::env;
use super::function;
use super::lang;
use crate::lang::Value;
use objekt::clone_trait_object;
use std::collections::HashMap;

pub trait ModifyableFunc: objekt::Clone + lang::Function + function::SettableArgs {
    fn set_return_type(&mut self, return_type: lang::Type);
}

clone_trait_object!(ModifyableFunc);

// TODO: this is a mess. we need this in JS land but now in native. lol
#[allow(dead_code)]
pub struct ValueWithEnv<'a> {
    pub value: lang::Value,
    pub env: &'a env::ExecutionEnvironment,
}

#[allow(dead_code)]
pub fn to_named_args(func: &dyn lang::Function,
                     args: HashMap<lang::ID, lang::Value>)
                     -> impl Iterator<Item = (String, lang::Value)> {
    let mut short_name_by_id: HashMap<lang::ID, String> =
        func.takes_args()
            .into_iter()
            .map(|argdef| (argdef.id, argdef.short_name))
            .collect();
    args.into_iter()
        .map(move |(arg_id, value)| (short_name_by_id.remove(&arg_id).unwrap(), value))
}

pub fn resolve_futures(value: lang::Value) -> lang::Value {
    lang::Value::new_future(async {
        println!("start of the future");
        match value {
            lang::Value::Future(value_future) => {
                println!("future!");
                // need to recursive call here because even after resolving the
                // future, the Value could contain MORE nested futures!
                println!("awaiting {:?}", value_future.0);
                let awaited_value = value_future.0.await;
                println!("awaited");
                if contains_futures(&awaited_value) {
                    resolve_futures(awaited_value)
                } else {
                    awaited_value
                }
            }
            lang::Value::EarlyReturn(inner) => {
                println!("early_return!");
                lang::Value::EarlyReturn(Box::new(resolve_futures(*inner)))
            }
            lang::Value::List(typ, v) => {
                println!("list!");
                lang::Value::List(typ, v.into_iter().map(resolve_futures).collect())
            }
            lang::Value::Struct { values, struct_id } => {
                println!("struct!");
                lang::Value::Struct { struct_id,
                                      values: values.into_iter()
                                                    .map(|(value_id, value)| {
                                                        (value_id, resolve_futures(value))
                                                    })
                                                    .collect() }
            }
            lang::Value::EnumVariant { box value,
                                       variant_id, } => {
                lang::Value::EnumVariant { variant_id,
                                           value: Box::new(resolve_futures(value)) }
            }
            lang::Value::Null
            | lang::Value::String(_)
            | lang::Value::Number(_)
            | lang::Value::Boolean(_)
            | lang::Value::AnonymousFunction(_) => value,
        }
    })
}

pub async fn resolve_all_futures(mut val: lang::Value) -> lang::Value {
    println!("resolving {:?}", val);
    while contains_futures(&val) {
        val = resolve_futures(val);
        println!("now resolved partial {:?}", val);
        val = match val {
            lang::Value::Future(value_future) => {
                println!("now resolved partial in future {:?}", value_future);
                value_future.0.await
            }
            _ => {
                println!("now resolved partial out of future {:?}", val);
                val
            }
        }
    }
    println!("now resolved done {:?}", val);
    val
}

fn contains_futures(val: &lang::Value) -> bool {
    match val {
        lang::Value::Future(_value_future) => {
            // need to recursive call here because even after resolving the
            // future, the Value could contain MORE nested futures!
            true
        }
        lang::Value::List(_typ, v) => v.iter().any(contains_futures),
        lang::Value::Struct { values, .. } => values.iter().any(|(_id, val)| contains_futures(val)),
        lang::Value::EnumVariant { box value, .. } => contains_futures(value),
        lang::Value::Null
        | lang::Value::String(_)
        | lang::Value::Number(_)
        | lang::Value::Boolean(_)
        | lang::Value::AnonymousFunction(_) => false,
        Value::EarlyReturn(inner) => contains_futures(inner),
    }
}
