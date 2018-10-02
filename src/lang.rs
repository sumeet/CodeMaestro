use std::fmt;

use super::ExecutionEnvironment;

pub trait Function: objekt::Clone {
    fn call(&self, env: &mut ExecutionEnvironment, args: Vec<Value>) -> Value;
    fn name(&self) -> &str;
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<Function: {}>", self.name())
    }
}

clone_trait_object!(Function);

#[derive(Clone,Debug)]
pub enum CodeNode {
    FunctionCall(FunctionCall),
    StringLiteral(StringLiteral),
}

pub enum Error {
    ArgumentError
}

pub enum Value {
    Null,
    String(String),
    Result(Result<Box<Value>,Error>)
}

impl CodeNode {
    pub fn evaluate(&self, env: &mut ExecutionEnvironment) -> Value {
        match self {
            CodeNode::FunctionCall(function_call) => {
                let args: Vec<Value> = function_call.args.iter().map(|arg| arg.evaluate(env)).collect();
                function_call.function.call(env, args)
            }
            CodeNode::StringLiteral(string_literal) => {
                // xxx: can i get rid of this clone?
                Value::String(string_literal.value.clone())
            }
        }
    }

    pub fn description(&self) -> String {
        match self {
            CodeNode::FunctionCall(function_call) => {
                format!("Function call: {}", function_call.function.name())
            }
            CodeNode::StringLiteral(string_literal) => {
                format!("String literal: {}", string_literal.value)
            }
        }
    }

    // these are just placeholder IDs for now, because for hello world, there's no
    // need to further disambiguate
    pub fn id(&self) -> ID {
        match self {
            CodeNode::FunctionCall(function_call) => {
                function_call.id
            }
            CodeNode::StringLiteral(string_literal) => {
                string_literal.id
            }
        }
    }
}

pub type ID = u64;

#[derive(Clone, Debug)]
pub struct StringLiteral {
    pub value: String,
    pub id: ID,
}

#[derive(Clone,Debug)]
pub struct FunctionCall {
    pub function: Box<Function>,
    pub args: Vec<CodeNode>,
    pub id: ID,
}
