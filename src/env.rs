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
use itertools::Itertools;
use std::convert::TryInto;
use std::panic::panic_any;

#[macro_export]
macro_rules! await_eval_result {
    ($e:expr) => {
        $crate::external_func::resolve_all_futures($e.await).await
    };
}

#[derive(Clone, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct SharedLocals(pub Rc<RefCell<BTreeMap<lang::ID, lang::Value>>>);

use std::collections::BTreeMap;
impl std::hash::Hash for SharedLocals {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0
            .borrow()
            .iter()
            .collect::<BTreeMap<_, _>>()
            .hash(state)
    }
}

#[derive(Clone)]
pub struct Interpreter {
    pub env: Rc<RefCell<ExecutionEnvironment>>,
    pub locals: SharedLocals,
}

impl Interpreter {
    pub fn new() -> Self {
        Self { env: Rc::new(RefCell::new(ExecutionEnvironment::new())),
               locals: SharedLocals(Rc::new(RefCell::new(BTreeMap::new()))) }
    }

    // TODO: instead of setting local variables directly on `env`, set them on a per-interp `locals`
    // object... i think. keep this here like this until we have one
    pub fn set_local_variable(&mut self, id: lang::ID, value: lang::Value) {
        self.locals.0.borrow_mut().insert(id, value);
    }

    pub fn modify_local_variable<T>(&self,
                                    id: lang::ID,
                                    change_fn: impl FnOnce(&mut lang::Value) -> T)
                                    -> Option<T> {
        let mut locals = self.locals.0.borrow_mut();
        locals.get_mut(&id).map(change_fn)
    }

    pub fn get_local_variable(&self, id: lang::ID) -> Option<lang::Value> {
        self.locals.0.borrow().get(&id).cloned()
    }

    pub fn with_env_and_new_locals(env: Rc<RefCell<ExecutionEnvironment>>) -> Self {
        Self { env,
               locals: SharedLocals(Rc::new(RefCell::new(BTreeMap::new()))) }
    }

    pub fn env(&self) -> Rc<RefCell<ExecutionEnvironment>> {
        Rc::clone(&self.env)
    }

    // TODO: this is insane that we have to clone the code just to evaluate it. this is gonna slow
    // down evaluation so much
    pub fn evaluate<'a>(&'a mut self,
                        code_node: &'a lang::CodeNode)
                        -> impl Future<Output = lang::Value> + 'a {
        let start_time = std::time::SystemTime::now();
        let prev_eval_result = Rc::clone(&self.env.borrow().eval_result_by_code_id);
        let result: Pin<Box<dyn Future<Output = lang::Value>>> = match code_node {
            lang::CodeNode::FunctionCall(function_call) => {
                // TODO: get rid of the unwrap and bubble up the error
                use futures_util::TryFutureExt;
                Box::pin(self.evaluate_function_call(&function_call)
                             .unwrap_or_else(|e| panic_any(e)))
            }
            lang::CodeNode::Argument(argument) => {
                Box::pin(self.evaluate(std::borrow::Borrow::borrow(&argument.expr)))
            }
            lang::CodeNode::StringLiteral(string_literal) => {
                let val = string_literal.value.clone();
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
                    for exp in &block.expressions {
                        return_value = await_eval_result!(interp.evaluate(exp));
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
                let mut interp = self.clone();
                Box::pin(async move {
                    // let mut values = HashMap::with_capacity(struct_literal.fields.len());
                    let mut values = BTreeMap::new();
                    for literal_field in struct_literal.fields() {
                        values.insert(literal_field.struct_field_id,
                                      await_eval_result!(interp.evaluate(&literal_field.expr)));
                    }
                    lang::Value::Struct { struct_id: struct_literal.struct_id,
                                          values: lang::StructValues(values) }
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
            lang::CodeNode::Match(mach) => {
                let mut new_interp = self.clone();
                Box::pin(async move {
                    let match_exp_fut = new_interp.evaluate(&mach.match_expression);
                    let (variant_id, value) =
                        await_eval_result!(match_exp_fut).into_enum().unwrap();
                    let var_id = lang::Match::make_variable_id(mach.id, variant_id);
                    new_interp.set_local_variable(var_id, value);
                    let branch_code = mach.branch_by_variant_id.get(&variant_id).unwrap();
                    await_eval_result!(new_interp.evaluate(&branch_code))
                })
            }
            lang::CodeNode::ListLiteral(list_literal) => {
                let mut interp = self.clone();
                Box::pin(async move {
                    let mut output_vec = Vec::with_capacity(list_literal.elements.len());

                    for element in &list_literal.elements {
                        // TODO: these can be awaited in parallel
                        output_vec.push(await_eval_result!(interp.evaluate(element)));
                    }

                    lang::Value::List(list_literal.element_type.clone(), output_vec)
                })
            }
            CodeNode::MapLiteral(lang::MapLiteral { id: _,
                                                    from_type,
                                                    to_type,
                                                    elements, }) => {
                // TODO: need to use the elements from the map literal
                let mut interp = self.clone();
                Box::pin(async move {
                    let mut output = BTreeMap::new();

                    for (key_element, value_element) in elements {
                        // TODO: these can be awaited in parallel
                        output.insert(await_eval_result!(interp.evaluate(key_element)),
                                      await_eval_result!(interp.evaluate(value_element)));
                    }

                    lang::Value::Map { from: from_type.clone(),
                                       to: to_type.clone(),
                                       value: output }
                })
            }
            lang::CodeNode::StructFieldGet(sfg) => {
                let struct_fut = self.evaluate(sfg.struct_expr.as_ref());
                let field_id = sfg.struct_field_id;
                Box::pin(async move {
                    let strukt = await_eval_result!(struct_fut);
                    (strukt.into_struct().unwrap().1).0
                                                     .remove(&field_id)
                                                     .unwrap()
                })
            }
            lang::CodeNode::ListIndex(list_index) => {
                let mut interp = self.clone();
                Box::pin(async move {
                    let index = await_eval_result!(interp.evaluate(list_index.index_expr.as_ref()))
                                      .as_i128()
                                      .unwrap();
                    // let index = await_eval_result!(index_fut).as_i128().unwrap();
                    if index.is_negative() {
                        return err_result_string(format!("can't index into a list with a negative index: {}", index));
                    }

                    let list_fut = interp.evaluate(list_index.list_expr.as_ref());

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
            CodeNode::AnonymousFunction(anon_func) => Box::pin(async move {
                lang::Value::AnonymousFunction(anon_func.clone(),
                                               // TODO: is there be a function that already does this?
                                               SharedLocals(Rc::clone(&self.locals.0)))
            }),
            // guess_type of this will return Result<Null, Number>
            // here, Number is the index that didn't exist in the list we're changing
            CodeNode::ReassignListIndex(rli) => {
                let mut interp = self.clone();
                Box::pin(async move {
                    let index =
                        await_eval_result!(interp.evaluate(rli.index_expr.as_ref())).as_i128()
                                                                                    .unwrap();
                    let set_to_val = await_eval_result!(interp.evaluate(rli.set_to_expr.as_ref()));

                    let index_exists = interp.modify_local_variable(rli.assignment_id, |val| {
                                                 let vec_to_change = val.as_mut_vec().unwrap();
                                                 vec_to_change.get_mut(index as usize)
                                                              .map(|hole| *hole = set_to_val)
                                                              .is_some()
                                             });
                    // let mut current_local_var = resolve_all_futures(interp.get_local_variable(rli.assignment_id).unwrap()).await;
                    // let vec_to_change = current_local_var.as_mut_vec().unwrap();
                    // let index_exists = vec_to_change.get_mut(index as usize)
                    //                                 .map(|hole| *hole = set_to_val)
                    //                                 .is_some();
                    // interp.set_local_variable(rli.assignment_id, current_local_var);
                    if index_exists == Some(true) {
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
                        None => await_eval_result!(interp.evaluate(&trai.or_else_expr)),
                    }
                })
            }
            CodeNode::ForLoop(for_loop) => {
                let mut interp = self.clone();
                Box::pin(async move {
                    let list =
                        await_eval_result!(interp.evaluate(&for_loop.list_expression)).into_vec()
                                                                                      .unwrap();
                    for v in list {
                        interp.set_local_variable(for_loop.id, v);
                        let result = await_eval_result!(interp.evaluate(&for_loop.body));
                        if result.is_early_return() {
                            return result;
                        }
                    }
                    lang::Value::Null
                })
            }
        };
        // result
        // interp runs faster when it doesn't have to copy and clone all the results
        async move {
            let result = await_eval_result!(result);
            let duration = std::time::SystemTime::now().duration_since(start_time)
                                                       .unwrap();
            append_result(&mut prev_eval_result.borrow_mut(),
                          code_node.id(),
                          duration,
                          result.clone());
            result
        }
        // async move {
        //     let result = await_eval_result!(result);
        //     prev_eval_result.borrow_mut()
        //                     .insert(code_node.id(), result.clone());
        //     result
        // }
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

    fn evaluate_function_call<'a>(
        &'a mut self,
        function_call: &'a lang::FunctionCall)
        -> impl Future<Output = Result<lang::Value, ExecutionError>> + 'a {
        let mut interp = self.clone();
        async move {
            let mut args = HashMap::with_capacity(function_call.args.len());
            // for (arg_id, arg_value_future) in
            //     function_call.args
            //                  .iter()
            //                  .map(|code_node| code_node.into_argument())
            //                  .map(|arg| (arg.argument_definition_id, interp.evaluate(&arg.expr)))
            for function_call_arg in &function_call.args {
                let function_call_arg = function_call_arg.into_argument();
                let arg_id = function_call_arg.argument_definition_id;
                args.insert(arg_id,
                            await_eval_result!(interp.evaluate(&function_call_arg.expr)));
            }

            let new_stack_frame = interp.new_stack_frame();
            let function_id = function_call.function_reference().function_id;

            // XXX: CAUTION: this seems fishy to me.... before, we used to clone the function so
            // we didn't have to keep the env borrowed. pretty sure this means that the function
            // itself wouldn't be able to borrow_mut the env
            // let func = new_stack_frame.env
            //                           .borrow()
            //                           .find_function(function_id)
            //                           .cloned();
            let env = interp.env.borrow();
            let func = env.find_function(function_id);

            match func {
                Some(function) => {
                    // TODO: need to generate new copy of locals for stack, but rest of env should be the same

                    let returned_val = function.call(new_stack_frame, args);
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

#[derive(Debug)]
pub struct EvaluationDebugResult {
    pub time_elapsed: std::time::Duration,
    pub last_value: lang::Value,
}

fn append_result(result_by_code_id: &mut HashMap<lang::ID, EvaluationDebugResult>,
                 code_id: lang::ID,
                 time: std::time::Duration,
                 val: lang::Value) {
    if result_by_code_id.contains_key(&code_id) {
        let result = result_by_code_id.get_mut(&code_id).unwrap();
        result.time_elapsed += time;
        result.last_value = val;
    } else {
        result_by_code_id.insert(code_id,
                                 EvaluationDebugResult { time_elapsed: time,
                                                         last_value: val });
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionEnvironment {
    pub console: String,
    pub functions: HashMap<lang::ID, Box<dyn lang::Function + 'static>>,
    pub typespecs: HashMap<lang::ID, Box<dyn lang::TypeSpec + 'static>>,

    // TODO: not sure where to put this
    pub eval_result_by_code_id: Rc<RefCell<HashMap<lang::ID, EvaluationDebugResult>>>,
}

impl ExecutionEnvironment {
    pub fn new() -> ExecutionEnvironment {
        return ExecutionEnvironment { console: String::new(),
                                      eval_result_by_code_id:
                                          Rc::new(RefCell::new(HashMap::new())),
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
