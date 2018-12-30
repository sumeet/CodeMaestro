use super::lang;
use super::structs;
use super::async_executor;

use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::future::Future;
use std ::pin::Pin;
use std::rc::Rc;

pub struct Interpreter {
    env: Rc<RefCell<ExecutionEnvironment>>,
    pub env2: ExecutionEnvironment,
    // TODO: the way i've shittily coded things, the ExecutionEnvironment needs to share the interp
    // with the main function, so the way i've drafted things, i SHOULD be able to get rid of this
    // rc refcell
    pub async_executor: Rc<RefCell<async_executor::AsyncExecutor>>,
}

impl Interpreter {
    pub fn new() -> Self {
        let async_executor =
            Rc::new(RefCell::new(async_executor::AsyncExecutor::new()));
        let async_executor2 = Rc::clone(&async_executor);
        let async_executor3 = Rc::clone(&async_executor);
        Self {
            env: Rc::new(RefCell::new(ExecutionEnvironment::new(async_executor))),
            env2: ExecutionEnvironment::new(async_executor3),
            async_executor: async_executor2,
        }
    }
}


pub struct ExecutionEnvironment {
    pub console: String,
    // TODO: lol, this is going to end up being stack frames, or smth like that
    pub locals: HashMap<lang::ID, lang::Value>,
    pub functions: HashMap<lang::ID, Box<lang::Function + 'static>>,
    pub typespecs: HashMap<lang::ID, Box<lang::TypeSpec + 'static>>,
    pub async_executor: Rc<RefCell<async_executor::AsyncExecutor>>,
}

impl ExecutionEnvironment {
    pub fn new(async_executor: Rc<RefCell<async_executor::AsyncExecutor>>) -> ExecutionEnvironment {
        return ExecutionEnvironment {
            console: String::new(),
            locals: HashMap::new(),
            functions: HashMap::new(),
            typespecs: Self::built_in_typespecs(),
            async_executor,
        }
    }

    fn built_in_typespecs() -> HashMap<lang::ID, Box<lang::TypeSpec>> {
        let mut typespec_by_id : HashMap<lang::ID, Box<lang::TypeSpec>> = HashMap::new();
        typespec_by_id.insert(lang::STRING_TYPESPEC.id, Box::new(lang::STRING_TYPESPEC.clone()));
        typespec_by_id.insert(lang::NUMBER_TYPESPEC.id, Box::new(lang::NUMBER_TYPESPEC.clone()));
        typespec_by_id.insert(lang::LIST_TYPESPEC.id, Box::new(lang::LIST_TYPESPEC.clone()));
        typespec_by_id.insert(lang::NULL_TYPESPEC.id, Box::new(lang::NULL_TYPESPEC.clone()));
        typespec_by_id
    }

    pub fn add_function(&mut self, function: Box<lang::Function>) {
        self.functions.insert(function.id(), function);
    }

    pub fn find_function(&self, id: lang::ID) -> Option<&Box<lang::Function>> {
        self.functions.get(&id)
    }

    pub fn delete_function(&mut self, id: lang::ID) {
        self.functions.remove(&id).unwrap();
    }

    pub fn list_functions(&self) -> impl Iterator<Item = &Box<lang::Function>> {
        self.functions.iter().map(|(_, func)| func)
    }

    pub fn add_typespec<T: lang::TypeSpec + 'static>(&mut self, typespec: T) {
        self.typespecs.insert(typespec.id(), Box::new(typespec));
    }

    pub fn list_typespecs(&self) -> impl Iterator<Item = &Box<lang::TypeSpec>> {
        self.typespecs.values()
    }

    pub fn find_typespec(&self, id: lang::ID) -> Option<&Box<lang::TypeSpec>> {
        self.typespecs.get(&id)
    }

    pub fn find_struct(&self, id: lang::ID) -> Option<&structs::Struct> {
        self.find_typespec(id)
            .and_then(|ts| ts.downcast_ref::<structs::Struct>())
    }

    pub fn run<F: FnOnce(lang::Value) + 'static>(&mut self, code_node: &lang::CodeNode, callback: F) {
        let fut = self.evaluate(code_node);
        self.async_executor.borrow_mut().exec(async move {
            callback(await!(fut));
            let ok : Result<(), ()> = Ok(());
            ok
        })
    }

    pub fn evaluate(&mut self, code_node: &lang::CodeNode) -> Pin<Box<Future<Output = lang::Value>>> {
        let code_node = code_node.clone();
        match code_node {
            lang::CodeNode::FunctionCall(function_call) => {
                Box::pin(self.evaluate_function_call(function_call))
            }
            lang::CodeNode::Argument(argument) => {
                Box::pin(self.evaluate(argument.expr.borrow()))
            }
            lang::CodeNode::StringLiteral(string_literal) => {
                let val = string_literal.value.clone();
                Box::pin( async { lang::Value::String(val) })
            }
            lang::CodeNode::Assignment(assignment) => {
                Box::pin(self.evaluate_assignment(&assignment))
            }
            lang::CodeNode::Block(block) => {
                Box::pin(async {
                    lang::Value::Null
                })
                // if there are no expressions in this block, then it will evaluate to Null
//                let mut return_value = lang::Value::Null;
//                for expression in block.expressions.iter() {
//                    return_value = self.evaluate(expression)
//                }
//                Box::new(return_value)

            }
            lang::CodeNode::VariableReference(variable_reference) => {
                let var = self.get_local_variable(variable_reference.assignment_id).unwrap().clone();
                Box::pin(async { var })
            }
            lang::CodeNode::FunctionReference(_) => Box::pin(async { lang::Value::Null }),
            lang::CodeNode::FunctionDefinition(_) => Box::pin(async { lang::Value::Null }),
            // TODO: trying to evaluate a placeholder should probably panic... but we don't have a
            // concept of panic yet
            lang::CodeNode::Placeholder(_) => Box::pin(async { lang::Value::Null }),
            lang::CodeNode::NullLiteral => Box::pin(async { lang::Value::Null }),
            lang::CodeNode::StructLiteral(struct_literal) => {
                let value_futures : HashMap<lang::ID, Pin<Box<Future<Output = lang::Value>>>> = struct_literal.fields().map(|literal_field| {
                    (literal_field.struct_field_id, self.evaluate(&literal_field.expr))
                }).collect();
                Box::pin(async move {
                    // TODO: use join to await them all at the same time
                    let mut values = HashMap::new();
                    for (id, value_future) in value_futures.into_iter() {
                        values.insert(id, await!(value_future));
                    }
                    lang::Value::Struct {
                        struct_id: struct_literal.struct_id,
                        values,
                    }
                })
            }
            // i think these code nodes will actually never be evaluated
                lang::CodeNode::StructLiteralField(_struct_literal_field) => Box::pin(async { lang::Value::Null }),
        }
    }

    fn evaluate_assignment(&mut self, assignment: &lang::Assignment) -> impl Future<Output = lang::Value> {
        async { lang::Value::Null }
//        let value = self.evaluate(&assignment.expression);
//        // TODO: pretty sure i'll have to return an Rc<Value> in evaluate
//        self.set_local_variable(assignment.id, value.clone());
//        // the result of an assignment is the value being assigned
//        value
    }

    fn evaluate_function_call(&mut self, function_call: lang::FunctionCall) -> impl Future<Output = lang::Value> {
        async { lang::Value::Null }
//        let args: HashMap<lang::ID,lang::Value> = function_call.args.iter()
//            .map(|code_node| code_node.into_argument())
//            .map(|arg| (arg.argument_definition_id, self.evaluate(&arg.expr)))
//            .collect();
//        let function_id = function_call.function_reference().function_id;
//        match self.find_function(function_id) {
//            Some(function) => {
//                function.clone().call(self, args)
//            }
//            None => {
//                lang::Value::Error(lang::Error::UndefinedFunctionError(function_id))
//            }
//        }
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
