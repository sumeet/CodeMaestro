use super::lang;
use super::structs;

use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std ::pin::Pin;
use std::rc::Rc;

use itertools::Itertools;
use std::convert::TryInto;
use crate::builtins::ok_result;
use crate::builtins::err_result;

#[macro_export]
macro_rules! await_eval_result {
    ($e:expr) => {
        await!($crate::external_func::resolve_all_futures(await!($e)))
    }
}

pub struct Interpreter {
    pub env: Rc<RefCell<ExecutionEnvironment>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Self {
            env: Rc::new(RefCell::new(ExecutionEnvironment::new())),
        }
    }

    // TODO: instead of setting local variables directly on `env`, set them on a per-interp `locals`
    // object... i think. keep this here like this until we have one
    pub fn set_local_variable(&mut self, id: lang::ID, value: lang::Value) {
        self.env.borrow_mut().set_local_variable(id, value.clone());
    }

    pub fn with_env(env: Rc<RefCell<ExecutionEnvironment>>) -> Self {
        Self { env }
    }

    pub fn env(&self) -> Rc<RefCell<ExecutionEnvironment>> {
        Rc::clone(&self.env)
    }

    // TODO: this is insane that we have to clone the code just to evaluate it. this is gonna slow
    // down evaluation so much
    pub fn evaluate(&mut self, code_node: &lang::CodeNode) -> Pin<Box<dyn Future<Output = lang::Value>>> {
        let code_node = code_node.clone();
        match code_node {
            lang::CodeNode::FunctionCall(function_call) => {
                Box::pin(self.evaluate_function_call(&function_call))
            }
            lang::CodeNode::Argument(argument) => {
                Box::pin(self.evaluate(argument.expr.borrow()))
            }
            lang::CodeNode::StringLiteral(string_literal) => {
                let val = string_literal.value;
                Box::pin( async move { lang::Value::String(val) })
            }
            lang::CodeNode::NumberLiteral(number_literal) => {
                let val = number_literal.value;
                Box::pin( async move { lang::Value::Number(val.into()) })
            }
            lang::CodeNode::Assignment(assignment) => {
                Box::pin(self.evaluate_assignment(&assignment))
            }
            // TODO: pretty sure i need something here to ensure these futures run serially
            // it exists, check out futures_ordered. will have to do a little bit of hacking to get
            // it to work with async / await i believe
            lang::CodeNode::Block(block) => {
                let futures = block.expressions.iter()
                    .map(|exp| self.evaluate(exp))
                    .collect_vec();
                Box::pin(async move {
                    let mut return_value = lang::Value::Null;
                    for future in futures.into_iter() {
                        return_value = await_eval_result!(future)
                    }
                    return_value
                })
            }
            lang::CodeNode::VariableReference(variable_reference) => {
                let env = Rc::clone(&self.env);
                Box::pin(async move {
                    (*env).borrow().get_local_variable(variable_reference.assignment_id).unwrap().clone()
                })
            }
            lang::CodeNode::FunctionReference(_) => Box::pin(async { lang::Value::Null }),
            // TODO: trying to evaluate a placeholder should probably panic... but we don't have a
            // concept of panic yet
            lang::CodeNode::Placeholder(_) => Box::pin(async { lang::Value::Null }),
            lang::CodeNode::NullLiteral(_) => Box::pin(async { lang::Value::Null }),
            lang::CodeNode::StructLiteral(struct_literal) => {
                let value_futures : HashMap<lang::ID, Pin<Box<dyn Future<Output = lang::Value>>>> = struct_literal.fields().map(|literal_field| {
                    (literal_field.struct_field_id, self.evaluate(&literal_field.expr))
                }).collect();
                Box::pin(async move {
                    // TODO: use join to await them all at the same time
                    let mut values = HashMap::new();
                    for (id, value_future) in value_futures.into_iter() {
                        values.insert(id, await_eval_result!(value_future));
                    }
                    lang::Value::Struct {
                        struct_id: struct_literal.struct_id,
                        values,
                    }
                })
            }
            // i think these code nodes will actually never be evaluated, because they get evaluated
            // as part of the struct itself
            lang::CodeNode::StructLiteralField(_struct_literal_field) => panic!("struct literals are never evaluated"),
            lang::CodeNode::Conditional(conditional) => {
                let condition_fut = self.evaluate(conditional.condition.as_ref());
                let true_branch_fut = self.evaluate(conditional.true_branch.as_ref());
                // TODO: does the else branch get evaluated just by nature of creating the future,
                // even if we never await it?
                let else_branch_fut = match conditional.else_branch.as_ref() {
                    Some(else_branch) => self.evaluate(else_branch.as_ref()),
                    None => Box::pin(async { lang::Value::Null }),
                };
                Box::pin(async move {
                    if await_eval_result!(condition_fut).as_boolean().unwrap() {
                        await_eval_result!(true_branch_fut)
                    } else {
                        await_eval_result!(else_branch_fut)
                    }
                })
            },
            lang::CodeNode::Match(mach) => {
                let match_exp_fut = self.evaluate(&mach.match_expression);
                let mut branch_by_variant_id : HashMap<_, _> = mach.branch_by_variant_id.iter()
                    .map(|(variant_id, branch_expression)| {
                        (*variant_id, self.evaluate(branch_expression))
                    }).collect();
                let env = Rc::clone(&self.env);
                let match_id = mach.id;
                Box::pin(async move {
                    let (variant_id, value) = await_eval_result!(match_exp_fut).into_enum().unwrap();
                    env.borrow_mut().set_local_variable(lang::Match::variable_id(match_id, variant_id),
                                                        value);
                    await_eval_result!(branch_by_variant_id.remove(&variant_id).unwrap())
                })
            },
            lang::CodeNode::ListLiteral(list_literal) => {
                let futures = list_literal
                    .elements.iter().map(|e| self.evaluate(e))
                    .collect_vec();
                let mut output_vec = vec![];
                Box::pin(async move {
                    // TODO: this can be done in parallel
                    for future in futures.into_iter() {
                        output_vec.push(await_eval_result!(future))
                    }
                    lang::Value::List(output_vec)
                })
            },
            lang::CodeNode::StructFieldGet(sfg) => {
                let struct_fut = self.evaluate(
                    sfg.struct_expr.as_ref());
                let field_id = sfg.struct_field_id;
                Box::pin(async move {
                    let strukt = await_eval_result!(struct_fut);
                    strukt.into_struct().unwrap().1.remove(&field_id).unwrap()
                })
            },
            lang::CodeNode::ListIndex(list_index) => {
                let list_fut = self.evaluate(list_index.list_expr.as_ref());
                let index_fut = self.evaluate(list_index.index_expr.as_ref());
                Box::pin(async move {
                    let index = await_eval_result!(index_fut).into_i128().unwrap();
                    if index.is_negative() {
                        return err_result(format!("can't index into a list with a negative index: {}", index))
                    }
                    let index_usize : Option<usize> = index.try_into().ok();
                    if index_usize.is_none() {
                        return err_result(format!("{} isn't a valid index", index))
                    }

                    let index_usize = index_usize.unwrap();
                    let mut vec = await_eval_result!(list_fut).into_vec().unwrap();
                    if vec.len() == 0 || index_usize > vec.len() - 1 {
                        return err_result(format!("list of size {} doesn't contain index {}", vec.len(), index))
                    }
                    ok_result(vec.remove(index_usize))
                })
            }
        }
    }

    fn evaluate_assignment(&mut self, assignment: &lang::Assignment) -> impl Future<Output = lang::Value> {
        let value_future = self.evaluate(&assignment.expression);
        let env = Rc::clone(&self.env);
        let assignment_id = assignment.id;
        async move {
            let value = await_eval_result!(value_future);
            env.borrow_mut().set_local_variable(assignment_id, value.clone());
            // the result of an assignment is the value being assigned
            value
        }
    }

    fn evaluate_function_call(&mut self, function_call: &lang::FunctionCall) -> impl Future<Output = lang::Value> {
        let args_futures = function_call.args.iter()
            .map(|code_node| code_node.into_argument())
            .map(|arg| (arg.argument_definition_id, self.evaluate(&arg.expr)))
            .collect_vec();
        let function_id = function_call.function_reference().function_id;
        let env = self.env();
        let func = (*env).borrow().find_function(function_id).cloned();
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
                    let interp = Self::with_env(Rc::clone(&env));
                    function.call(interp, args)
                }
                None => {
                    lang::Value::Error(lang::Error::UndefinedFunctionError(function_id))
                }
            }
        }
    }

    pub fn dup(&self) -> Self {
        Self::with_env(Rc::clone(&self.env))
    }
}

#[derive(Debug)]
pub struct ExecutionEnvironment {
    pub console: String,
    // TODO: lol, this is going to end up being stack frames, or smth like that
    pub locals: HashMap<lang::ID, lang::Value>,
    pub functions: HashMap<lang::ID, Box<dyn lang::Function + 'static>>,
    pub typespecs: HashMap<lang::ID, Box<dyn lang::TypeSpec + 'static>>,
}

impl ExecutionEnvironment {
    pub fn new() -> ExecutionEnvironment {
        return ExecutionEnvironment {
            console: String::new(),
            locals: HashMap::new(),
            functions: HashMap::new(),
            typespecs: Self::built_in_typespecs(),
        }
    }

    fn built_in_typespecs() -> HashMap<lang::ID, Box<dyn lang::TypeSpec>> {
        let mut typespec_by_id : HashMap<lang::ID, Box<dyn lang::TypeSpec>> = HashMap::new();
        typespec_by_id.insert(lang::STRING_TYPESPEC.id, Box::new(lang::STRING_TYPESPEC.clone()));
        typespec_by_id.insert(lang::NUMBER_TYPESPEC.id, Box::new(lang::NUMBER_TYPESPEC.clone()));
        typespec_by_id.insert(lang::LIST_TYPESPEC.id, Box::new(lang::LIST_TYPESPEC.clone()));
        typespec_by_id.insert(lang::NULL_TYPESPEC.id, Box::new(lang::NULL_TYPESPEC.clone()));
        typespec_by_id.insert(lang::ERROR_TYPESPEC.id, Box::new(lang::ERROR_TYPESPEC.clone()));
        typespec_by_id
    }

    pub fn add_function(&mut self, function: impl lang::Function + 'static) {
        self.functions.insert(function.id(), Box::new(function));
    }

    pub fn add_function_box(&mut self, function: Box<dyn lang::Function>) {
        self.functions.insert(function.id(), function);
    }

    pub fn find_function(&self, id: lang::ID) -> Option<&Box<dyn lang::Function>> {
        self.functions.get(&id)
    }

    pub fn delete_function(&mut self, id: lang::ID) {
        self.functions.remove(&id).unwrap();
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

    pub fn set_local_variable(&mut self, id: lang::ID, value: lang::Value) {
        self.locals.insert(id, value);
    }

    pub fn get_local_variable(&self, id: lang::ID) -> Option<&lang::Value> {
        self.locals.get(&id)

    }

    pub fn println(&mut self, ln: &str) {
        self.console.push_str(ln);
        self.console.push_str("\n")
    }

    pub fn read_console(&self) -> &str {
        &self.console
    }
}
