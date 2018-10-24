use super::{lang};

use std::collections::HashMap;
use std::borrow::Borrow;

pub struct ExecutionEnvironment {
    pub console: String,
    pub locals: HashMap<lang::ID, lang::Value>,
    pub functions: HashMap<lang::ID, Box<lang::Function>>
}

impl ExecutionEnvironment {
    pub fn new() -> ExecutionEnvironment {
        return ExecutionEnvironment {
            console: String::new(),
            locals: HashMap::new(),
            functions: HashMap::new(),
        }
    }

    pub fn add_function(&mut self, function: Box<lang::Function>) {
        self.functions.insert(function.id(), function);
    }

    pub fn list_functions(&self) -> Vec<Box<lang::Function>> {
        self.functions.iter().map(|(_, func)| func.clone()).collect()
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
                let mut expressions = block.expressions.iter().peekable();
                while let Some(expression) = expressions.next() {
                    if expressions.peek().is_some() {
                        // not the last
                        self.evaluate(expression);
                    } else {
                        return self.evaluate(expression)
                    }
                }
                // if there are no expressions in this block, then it will evaluate to null
                lang::Value::Null
            }
            lang::CodeNode::VariableReference(variable_reference) => {
                self.get_local_variable(variable_reference.assignment_id).unwrap().clone()
            }
            lang::CodeNode::FunctionReference(_) => { lang::Value::Null }
            lang::CodeNode::FunctionDefinition(_) => { lang::Value::Null }
        }
    }

    fn evaluate_function_call(&mut self, function_call: &lang::FunctionCall) -> lang::Value {
        let args: HashMap<lang::ID,lang::Value> = function_call.args.iter()
            .map(|code_node| code_node.into_argument())
            .map(|arg| (arg.argument_definition_id, self.evaluate(&arg.expr)))
            .collect();
        let function_id = function_call.function_reference.function_id;
        match self.find_function(function_id) {
            Some(function) => {
                function.clone().call(self, args)
            }
            None => {
                lang::Value::Result(Err(lang::Error::UndefinedFunctionError(function_id)))
            }
        }
    }

    pub fn find_function(&self, id: lang::ID) -> Option<&Box<lang::Function>> {
        self.functions.get(&id)
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

