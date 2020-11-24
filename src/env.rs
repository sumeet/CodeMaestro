use super::lang;
use super::structs;

use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use crate::builtins;
use crate::builtins::{
    convert_lang_option_to_rust_option, convert_lang_value_to_rust_result, ok_result_value,
};
use crate::builtins::{err_result_string, err_result_value};
use crate::lang::CodeNode;
use crate::{enums, resolve_all_futures, EnvGenie};
use failure::_core::fmt::Formatter;
use futures_util::FutureExt;
use itertools::Itertools;
use std::convert::TryInto;

#[macro_export]
macro_rules! await_eval_result {
    ($e:expr) => {
        $crate::external_func::resolve_all_futures($e.await).await
    };
}

#[derive(Clone)]
pub struct Interpreter {
    pub env: Rc<RefCell<ExecutionEnvironment>>,
    pub locals: Rc<RefCell<HashMap<lang::ID, lang::Value>>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self { env: Rc::new(RefCell::new(ExecutionEnvironment::new())),
               locals: Rc::new(RefCell::new(HashMap::new())) }
    }

    // TODO: instead of setting local variables directly on `env`, set them on a per-interp `locals`
    // object... i think. keep this here like this until we have one
    pub fn set_local_variable(&mut self, id: lang::ID, value: lang::Value) {
        self.locals.borrow_mut().insert(id, value);
    }

    pub fn get_local_variable(&self, id: lang::ID) -> Option<lang::Value> {
        self.locals.borrow().get(&id).cloned()
    }

    pub fn with_env_and_new_locals(env: Rc<RefCell<ExecutionEnvironment>>) -> Self {
        Self { env,
               locals: Rc::new(RefCell::new(HashMap::new())) }
    }

    pub fn env(&self) -> Rc<RefCell<ExecutionEnvironment>> {
        Rc::clone(&self.env)
    }

    // TODO: this is insane that we have to clone the code just to evaluate it. this is gonna slow
    // down evaluation so much
    pub fn evaluate(&mut self, code_node: &lang::CodeNode) -> impl Future<Output = lang::Value> {
        let code_node = code_node.clone();
        let code_node_id = code_node.id();
        let result: Pin<Box<dyn Future<Output = lang::Value>>> = match code_node {
            lang::CodeNode::FunctionCall(function_call) => {
                // TODO: get rid of the unwrap and bubble up the error
                use futures_util::TryFutureExt;
                Box::pin(self.evaluate_function_call(&function_call)
                             .unwrap_or_else(|e| panic!(e)))
            }
            lang::CodeNode::Argument(argument) => {
                Box::pin(self.evaluate(std::borrow::Borrow::borrow(&argument.expr)))
            }
            lang::CodeNode::StringLiteral(string_literal) => {
                let val = string_literal.value;
                Box::pin(async move { lang::Value::String(val) })
            }
            lang::CodeNode::NumberLiteral(number_literal) => {
                let val = number_literal.value;
                Box::pin(async move { lang::Value::Number(val.into()) })
            }
            lang::CodeNode::Assignment(assignment) => {
                Box::pin(self.evaluate_assignment(&assignment))
            }
            lang::CodeNode::Reassignment(reassignment) => {
                Box::pin(self.evaluate_reassignment(&reassignment))
            }
            lang::CodeNode::Block(block) => {
                let mut interp = self.clone();
                Box::pin(async move {
                    let mut return_value = lang::Value::Null;
                    for exp in block.expressions {
                        return_value = await_eval_result!(interp.evaluate(&exp));
                        if return_value.is_early_return() {
                            break;
                        }
                    }
                    return_value
                })
            }
            lang::CodeNode::VariableReference(variable_reference) => {
                let interp = self.clone();
                Box::pin(async move {
                    interp.get_local_variable(variable_reference.assignment_id)
                          .unwrap()
                })
            }
            lang::CodeNode::FunctionReference(_) => Box::pin(async { lang::Value::Null }),
            // TODO: trying to evaluate a placeholder should probably panic... but we don't have a
            // concept of panic yet
            lang::CodeNode::Placeholder(_) => Box::pin(async { lang::Value::Null }),
            lang::CodeNode::NullLiteral(_) => Box::pin(async { lang::Value::Null }),
            lang::CodeNode::StructLiteral(struct_literal) => {
                let value_futures: HashMap<lang::ID, Pin<Box<dyn Future<Output = lang::Value>>>> =
                    struct_literal.fields()
                                  .map(|literal_field| {
                                      (literal_field.struct_field_id,
                                       self.evaluate(&literal_field.expr).boxed_local())
                                  })
                                  .collect();
                Box::pin(async move {
                    // TODO: use join to await them all at the same time
                    let mut values = HashMap::new();
                    for (id, value_future) in value_futures.into_iter() {
                        values.insert(id, await_eval_result!(value_future));
                    }
                    lang::Value::Struct { struct_id: struct_literal.struct_id,
                                          values }
                })
            }
            // i think these code nodes will actually never be evaluated, because they get evaluated
            // as part of the struct itself
            lang::CodeNode::StructLiteralField(_struct_literal_field) => {
                panic!("struct literal fields are never evaluated")
            }
            lang::CodeNode::Conditional(conditional) => {
                let mut interp = self.clone();
                Box::pin(async move {
                    if await_eval_result!(interp.evaluate(conditional.condition.as_ref())).as_boolean().unwrap() {
                        await_eval_result!(interp.evaluate(conditional.true_branch.as_ref()))
                    } else {
                        match conditional.else_branch.as_ref() {
                            Some(else_branch) => await_eval_result!(interp.evaluate(else_branch.as_ref())),
                            None => lang::Value::Null,
                        }
                    }
                })
            }
            CodeNode::WhileLoop(while_loop) => {
                let mut interp = self.clone();
                Box::pin(async move {
                    while await_eval_result!(interp.evaluate(&while_loop.condition)).as_boolean()
                                                                                    .unwrap()
                    {
                        let val = await_eval_result!(interp.evaluate(&while_loop.body));
                        if val.is_early_return() {
                            return val;
                        }
                    }
                    lang::Value::Null
                })
            }
            lang::CodeNode::Match(mut mach) => {
                let match_exp_fut = self.evaluate(&mach.match_expression);

                let mut new_interp = self.clone();
                Box::pin(async move {
                    let (variant_id, value) =
                        await_eval_result!(match_exp_fut).into_enum().unwrap();
                    let var_id = lang::Match::make_variable_id(mach.id, variant_id);
                    new_interp.set_local_variable(var_id, value);
                    let branch_code = mach.branch_by_variant_id.remove(&variant_id).unwrap();
                    await_eval_result!(new_interp.evaluate(&branch_code))
                })
            }
            lang::CodeNode::ListLiteral(list_literal) => {
                let futures = list_literal.elements
                                          .iter()
                                          .map(|e| self.evaluate(e))
                                          .collect_vec();
                let mut output_vec = vec![];
                Box::pin(async move {
                    // TODO: this can be done in parallel
                    for future in futures.into_iter() {
                        output_vec.push(await_eval_result!(future))
                    }
                    lang::Value::List(list_literal.element_type, output_vec)
                })
            }
            lang::CodeNode::StructFieldGet(sfg) => {
                let struct_fut = self.evaluate(sfg.struct_expr.as_ref());
                let field_id = sfg.struct_field_id;
                Box::pin(async move {
                    let strukt = await_eval_result!(struct_fut);
                    strukt.into_struct().unwrap().1.remove(&field_id).unwrap()
                })
            }
            lang::CodeNode::ListIndex(list_index) => {
                let mut interp = self.clone();
                Box::pin(async move {
                    let list_fut = interp.evaluate(list_index.list_expr.as_ref());
                    let index_fut = interp.evaluate(list_index.index_expr.as_ref());

                    let index = await_eval_result!(index_fut).as_i128().unwrap();
                    if index.is_negative() {
                        return err_result_string(format!("can't index into a list with a negative index: {}", index));
                    }
                    let index_usize: Option<usize> = index.try_into().ok();
                    if index_usize.is_none() {
                        return err_result_string(format!("{} isn't a valid index", index));
                    }

                    let index_usize = index_usize.unwrap();
                    let mut vec = await_eval_result!(list_fut).into_vec().unwrap();
                    if vec.len() == 0 || index_usize > vec.len() - 1 {
                        return err_result_string(format!("list of size {} doesn't contain index {}",
                                                        vec.len(),
                                                        index));
                    }
                    ok_result_value(vec.remove(index_usize))
                })
            }
            CodeNode::AnonymousFunction(anon_func) => {
                Box::pin(async move { lang::Value::AnonymousFunction(anon_func) })
            }
            // guess_type of this will return Result<Null, Number>
            // here, Number is the index that didn't exist in the list we're changing
            CodeNode::ReassignListIndex(rli) => {
                let mut interp = self.clone();
                Box::pin(async move {
                    let index =
                        await_eval_result!(interp.evaluate(rli.index_expr.as_ref())).as_i128()
                                                                                    .unwrap();
                    let set_to_val = await_eval_result!(interp.evaluate(rli.set_to_expr.as_ref()));

                    let mut current_local_var = resolve_all_futures(interp.get_local_variable(rli.assignment_id).unwrap()).await;
                    let vec_to_change = current_local_var.as_mut_vec().unwrap();
                    let index_exists = vec_to_change.get_mut(index as usize)
                                                    .map(|hole| *hole = set_to_val)
                                                    .is_some();
                    interp.set_local_variable(rli.assignment_id, current_local_var);
                    if index_exists {
                        ok_result_value(lang::Value::Null)
                    } else {
                        err_result_value(lang::Value::Number(index))
                    }
                })
            }
            CodeNode::EnumVariantLiteral(evl) => {
                let value_fut = self.evaluate(&evl.variant_value_expr);
                Box::pin(async move {
                    lang::Value::EnumVariant { variant_id: evl.variant_id,
                                               value: Box::new(await_eval_result!(value_fut)) }
                })
            }
            CodeNode::EarlyReturn(early_return) => {
                let expr_fut = self.evaluate(&early_return.code);
                Box::pin(async move { lang::Value::EarlyReturn(Box::new(await_eval_result!(expr_fut))) })
            }
            CodeNode::Try(trai) => {
                let mut interp = self.clone();
                Box::pin(async move {
                    let evaluated = await_eval_result!(interp.evaluate(&trai.maybe_error_expr));
                    let (enum_variant_id, _) = evaluated.as_enum().unwrap();
                    let enum_id =
                        EnvGenie::new(&interp.env.borrow()).find_enum_variant(enum_variant_id)
                                                           .unwrap()
                                                           .0
                                                           .id;
                    let opt = if enum_id == *builtins::RESULT_ENUM_ID {
                        // TODO: if result, we should be able to pass the error value to anonymous
                        // function inside of or_else_return_expr
                        convert_lang_value_to_rust_result(evaluated).ok()
                    } else if enum_id == *builtins::OPTION_ENUM_ID {
                        convert_lang_option_to_rust_option(evaluated)
                    } else {
                        panic!("expected Result or Option, but got enum {:?}",
                               interp.env.borrow().find_enum(enum_id))
                    };
                    match opt {
                        Some(ok_value) => ok_value,
                        None => {
                            lang::Value::EarlyReturn(Box::new(await_eval_result!(interp.evaluate(&trai.or_else_return_expr))))
                        }
                    }
                })
            }
        };
        let env = Rc::clone(&self.env);
        async move {
            let result = await_eval_result!(result);
            env.borrow_mut()
               .prev_eval_result_by_code_id
               .insert(code_node_id, result.clone());
            result
        }
    }

    fn evaluate_assignment(&mut self,
                           assignment: &lang::Assignment)
                           -> impl Future<Output = lang::Value> {
        let mut interp = self.clone();
        let assignment = assignment.clone();
        async move {
            let value = await_eval_result!(interp.evaluate(&assignment.expression));
            let assignment_id = assignment.id;
            interp.set_local_variable(assignment_id, value.clone());
            value
        }
    }

    fn evaluate_reassignment(&mut self,
                             reassignment: &lang::Reassignment)
                             -> impl Future<Output = lang::Value> {
        let mut interp = self.clone();
        let reassignment = reassignment.clone();
        async move {
            let value = await_eval_result!(interp.evaluate(&reassignment.expression));
            let assignment_id = reassignment.assignment_id;
            interp.set_local_variable(assignment_id, value.clone());
            value
        }
    }

    fn evaluate_function_call(&mut self,
                              function_call: &lang::FunctionCall)
                              -> impl Future<Output = Result<lang::Value, ExecutionError>> {
        let args_futures =
            function_call.args
                         .iter()
                         .map(|code_node| code_node.into_argument())
                         .map(|arg| (arg.argument_definition_id, self.evaluate(&arg.expr)))
                         .collect_vec();
        let function_id = function_call.function_reference().function_id;
        let env = self.env();
        let func = (*env).borrow().find_function(function_id).cloned();
        let interp = self.new_stack_frame();
        async move {
            // TODO: ok, can't pass the env in while we're borrowing the function, so we have to clone
            // it... figure out how to not do this :/

            let mut args = HashMap::new();
            for (arg_id, arg_future) in args_futures {
                let arg_value = await_eval_result!(arg_future);
                args.insert(arg_id, arg_value);
            }
            match func {
                Some(function) => {
                    // TODO: need to generate new copy of locals for stack, but rest of env should be the same

                    let returned_val = function.call(interp, args);
                    Ok(resolve_all_futures(returned_val).await
                                                        .unwrap_early_return())
                    // Ok(returned_val.unwrap_early_return())
                }
                None => Err(ExecutionError::UndefinedFunction),
            }
        }
    }

    pub fn new_stack_frame(&self) -> Self {
        Self::with_env_and_new_locals(Rc::clone(&self.env))
    }

    pub fn deep_clone_env(&self) -> Self {
        let env = self.env.as_ref();
        Self::with_env_and_new_locals(Rc::new(RefCell::new(env.borrow().clone())))
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionEnvironment {
    pub console: String,
    // TODO: lol, this is going to end up being stack frames, or smth like that
    // pub locals: HashMap<lang::ID, lang::Value>,
    pub functions: HashMap<lang::ID, Box<dyn lang::Function + 'static>>,
    pub typespecs: HashMap<lang::ID, Box<dyn lang::TypeSpec + 'static>>,
    pub prev_eval_result_by_code_id: HashMap<lang::ID, lang::Value>,
}

impl ExecutionEnvironment {
    pub fn new() -> ExecutionEnvironment {
        return ExecutionEnvironment { console: String::new(),
                                      prev_eval_result_by_code_id: HashMap::new(),
                                      functions: HashMap::new(),
                                      typespecs: Self::built_in_typespecs() };
    }

    fn built_in_typespecs() -> HashMap<lang::ID, Box<dyn lang::TypeSpec>> {
        let mut typespec_by_id: HashMap<lang::ID, Box<dyn lang::TypeSpec>> = HashMap::new();
        typespec_by_id.insert(lang::STRING_TYPESPEC.id,
                              Box::new(lang::STRING_TYPESPEC.clone()));
        typespec_by_id.insert(lang::NUMBER_TYPESPEC.id,
                              Box::new(lang::NUMBER_TYPESPEC.clone()));
        typespec_by_id.insert(lang::LIST_TYPESPEC.id,
                              Box::new(lang::LIST_TYPESPEC.clone()));
        typespec_by_id.insert(lang::NULL_TYPESPEC.id,
                              Box::new(lang::NULL_TYPESPEC.clone()));
        typespec_by_id.insert(lang::ERROR_TYPESPEC.id,
                              Box::new(lang::ERROR_TYPESPEC.clone()));
        typespec_by_id
    }

    pub fn add_function(&mut self, function: impl lang::Function + 'static) {
        for generic in function.defines_generics() {
            self.add_typespec(generic)
        }

        self.functions.insert(function.id(), Box::new(function));
    }

    pub fn add_function_box(&mut self, function: Box<dyn lang::Function>) {
        for generic in function.defines_generics() {
            self.add_typespec(generic)
        }

        self.functions.insert(function.id(), function);
    }

    pub fn find_function(&self, id: lang::ID) -> Option<&Box<dyn lang::Function>> {
        self.functions.get(&id)
    }

    pub fn delete_function(&mut self, id: lang::ID) {
        self.functions.remove(&id).unwrap();
    }

    pub fn delete_typespec(&mut self, id: lang::ID) {
        self.typespecs.remove(&id).unwrap();
    }

    pub fn list_functions(&self) -> impl Iterator<Item = &Box<dyn lang::Function>> {
        self.functions.iter().map(|(_, func)| func)
    }

    pub fn add_typespec<T: lang::TypeSpec + 'static>(&mut self, typespec: T) {
        self.typespecs.insert(typespec.id(), Box::new(typespec));
    }

    pub fn add_typespec_box(&mut self, typespec: Box<dyn lang::TypeSpec>) {
        self.typespecs.insert(typespec.id(), typespec);
    }

    pub fn list_typespecs(&self) -> impl Iterator<Item = &Box<dyn lang::TypeSpec>> {
        self.typespecs.values()
    }

    pub fn find_typespec(&self, id: lang::ID) -> Option<&Box<dyn lang::TypeSpec>> {
        self.typespecs.get(&id)
    }

    pub fn find_struct(&self, id: lang::ID) -> Option<&structs::Struct> {
        self.find_typespec(id)
            .and_then(|ts| ts.downcast_ref::<structs::Struct>())
    }

    pub fn find_enum(&self, id: lang::ID) -> Option<&enums::Enum> {
        self.find_typespec(id)
            .and_then(|ts| ts.downcast_ref::<enums::Enum>())
    }

    // pub fn set_local_variable(&mut self, id: lang::ID, value: lang::Value) {
    //     self.locals.insert(id, value);
    // }
    //
    // pub fn get_local_variable(&self, id: lang::ID) -> Option<&lang::Value> {
    //     self.locals.get(&id)
    // }

    pub fn println(&mut self, ln: &str) {
        // TODO: hopefully i never need this actual println ever again, because the in-editor rich
        // debug window is good
        println!("{}", ln);
        self.console.push_str(ln);
        self.console.push_str("\n")
    }

    pub fn read_console(&self) -> &str {
        &self.console
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ExecutionError {
    ArgumentNotFound,
    ArgumentWrongType,
    UndefinedFunction,
    PythonError,
    PythonDeserializationError,
    JavaScriptError,
    JavaScriptDeserializationError,
}

pub fn pp_struct(env: &ExecutionEnvironment, strukt: &structs::Struct) -> String {
    let env_genie = EnvGenie::new(env);
    let fields = strukt.fields
                       .iter()
                       .map(|field| {
                           format!("{}: {}",
                                   field.name,
                                   env_genie.get_name_for_type(&field.field_type)
                                            .unwrap_or("UnknownType".to_owned()))
                       })
                       .join(", ");
    format!("{} {{{}}}", strukt.name, fields)
}

impl std::fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
                        ExecutionError::ArgumentNotFound => "ArgumentNotFound",
                        ExecutionError::ArgumentWrongType => "ArgumentWrongType",
                        ExecutionError::UndefinedFunction => "UndefinedFunction",
                        ExecutionError::PythonError => "PythonError",
                        ExecutionError::PythonDeserializationError => "PythonDeserializationError",
                        ExecutionError::JavaScriptDeserializationError => {
                            "JavaScriptDeserializationError"
                        }
                        ExecutionError::JavaScriptError => "JavaScriptError",
                    })?;
        Ok(())
    }
}

impl std::error::Error for ExecutionError {}
