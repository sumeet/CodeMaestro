use super::env;
use super::lang;
use super::pystuff::PyFunc;
use super::jsstuff::JSFunc;
use std::collections::HashMap;
use std::future::Future;

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
            lang::Value::Number(_) => value
        }
    })
}

pub fn preresolve_futures_if_external_func(
    func: Option<Box<lang::Function + 'static>>,
    value: lang::Value) -> lang::ValueFuture {

    if let lang::Value::Future(fut) = resolve_futures(value) {
        println!("preresolving taking path 2");
        return lang::Value::new_value_future(async move { await!(fut) })
    } else {
        panic!("there's no way this could happen")
    }

//    if func.is_none() {
//        println!("preresolving taking path 1");
//        return lang::Value::new_value_future(async move { value })
//    }
//    let func = func.as_ref().unwrap();
//    if func.downcast_ref::<PyFunc>().is_some() || func.downcast_ref::<JSFunc>().is_some() {
//        if let lang::Value::Future(fut) = resolve_futures(value) {
//            println!("preresolving taking path 2");
//            return lang::Value::new_value_future(async move { await!(fut) })
//        } else {
//            panic!("there's no way this could happen")
//        }
//    }
//    println!("preresolving taking path 3");
//    lang::Value::new_value_future(async move { value })
}