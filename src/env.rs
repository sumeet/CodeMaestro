use super::{lang,structs};

use std::collections::HashMap;
use std::borrow::Borrow;

pub struct ExecutionEnvironment {
    pub async_executor: Box<AsyncExecutor>,
    pub console: String,
    // TODO: lol, this is going to end up being stack frames, or smth like that
    pub locals: HashMap<lang::ID, lang::Value>,
    pub functions: HashMap<lang::ID, Box<lang::Function>>,
    pub typespecs: HashMap<lang::ID, Box<lang::TypeSpec + 'static>>,
}

impl ExecutionEnvironment {
    pub fn new<T: AsyncExecutor + 'static>(async_executor: T) -> ExecutionEnvironment {
        return ExecutionEnvironment {
            console: String::new(),
            locals: HashMap::new(),
            functions: HashMap::new(),
            typespecs: Self::built_in_typespecs(),
            async_executor: Box::new(async_executor),
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

    pub fn evaluate(&mut self, code_node: &lang::CodeNode) -> lang::Value {
        match code_node {
            lang::CodeNode::FunctionCall(function_call) => {
                self.evaluate_function_call(function_call)
            }
            lang::CodeNode::Argument(argument) => {
                self.evaluate(argument.expr.borrow())
            }
            lang::CodeNode::StringLiteral(string_literal) => {
                lang::Value::String(string_literal.value.clone())
            }
            lang::CodeNode::Assignment(assignment) => {
                let value = self.evaluate(&assignment.expression);
                // TODO: pretty sure i'll have to return an Rc<Value> in evaluate
                self.set_local_variable(assignment.id, value.clone());
                // the result of an assignment is the value being assigned
                value
            }
            lang::CodeNode::Block(block) => {
                // if there are no expressions in this block, then it will evaluate to Null
                let mut return_value = lang::Value::Null;
                for expression in block.expressions.iter() {
                    return_value = self.evaluate(expression)
                }
                return_value

            }
            lang::CodeNode::VariableReference(variable_reference) => {
                self.get_local_variable(variable_reference.assignment_id).unwrap().clone()
            }
            lang::CodeNode::FunctionReference(_) => lang::Value::Null,
            lang::CodeNode::FunctionDefinition(_) => lang::Value::Null,
            // TODO: trying to evaluate a placeholder should probably panic... but we don't have a
            // concept of panic yet
            lang::CodeNode::Placeholder(_) => lang::Value::Null,
            lang::CodeNode::NullLiteral => lang::Value::Null,
            lang::CodeNode::StructLiteral(struct_literal) => {
                lang::Value::Struct {
                    struct_id: struct_literal.struct_id,
                    values: struct_literal.fields().map(|literal_field| {
                        (literal_field.struct_field_id,
                         self.evaluate(&literal_field.expr))
                    }).collect()
                }
            }
            // i think these code nodes will actually never be evaluated
            lang::CodeNode::StructLiteralField(_struct_literal_field) => lang::Value::Null,
        }
    }

    fn evaluate_function_call(&mut self, function_call: &lang::FunctionCall) -> lang::Value {
        let args: HashMap<lang::ID,lang::Value> = function_call.args.iter()
            .map(|code_node| code_node.into_argument())
            .map(|arg| (arg.argument_definition_id, self.evaluate(&arg.expr)))
            .collect();
        let function_id = function_call.function_reference().function_id;
        match self.find_function(function_id) {
            Some(function) => {
                function.clone().call(self, args)
            }
            None => {
                lang::Value::Error(lang::Error::UndefinedFunctionError(function_id))
            }
        }
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


use std::future::Future;

pub trait AsyncExecutor {
    // runs but you don't get any results back lol
    fn exec<I, E, F: Future<Output = Result<I, E>> + Send + 'static>(&mut self, future: F) where Self: Sized;
}
