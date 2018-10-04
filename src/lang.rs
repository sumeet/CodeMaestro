use std::fmt;
use std::borrow::BorrowMut;
use std::rc::Rc;

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
    Assignment(Assignment),
    Block(Block),
}

#[derive(Clone)]
pub enum Error {
    ArgumentError
}

#[derive(Clone)]
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
                Value::String(string_literal.value.clone())
            }
            CodeNode::Assignment(assignment) => {
                let value = assignment.expression.evaluate(env);
                // TODO: pretty sure i'll have to return an Rc<Value> in evaluate
                env.set_local_variable(assignment.id, value.clone());
                // the result of an assignment is the value being assigned
                value
            }
            CodeNode::Block(block) => {
                let mut expressions = block.expressions.iter().peekable();
                while let Some(expression) = expressions.next() {
                    if expressions.peek().is_some() {
                        // not the last
                        expression.evaluate(env);
                    } else {
                        return expression.evaluate(env)
                    }
                }
                // if there are no expressions in this block, then it will evaluate to null
                Value::Null
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
            CodeNode::Assignment(assignment) => {
                format!("Assignment: {}", assignment.name)
            }
            CodeNode::Block(block) => {
                format!("Code block: {}", block.id)
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
            CodeNode::Assignment(assignment) => {
                assignment.id
            }
            CodeNode::Block(block) => {
                block.id
            }
        }
    }

    pub fn children(&mut self) -> Vec<&mut CodeNode> {
        match self {
            CodeNode::FunctionCall(function_call) => {
                function_call.args.iter_mut().collect()
            }
            CodeNode::StringLiteral(_) => {
                Vec::new()
            }
            CodeNode::Assignment(assignment) => {
                vec![assignment.expression.borrow_mut()]
            }
            CodeNode::Block(block) => {
                block.expressions.iter_mut().collect()
            }
        }
    }

    pub fn replace(&mut self, code_node: &CodeNode) {
        if self.id() == code_node.id() {
            *self = code_node.clone()
        } else {
            for child in self.children() {
                child.replace(code_node)
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

#[derive(Clone,Debug)]
pub struct Assignment {
    pub name: String,
    // TODO: consider differentiating between CodeNodes and Expressions.
    pub expression: Box<CodeNode>,
    pub id: ID,
}

#[derive(Clone,Debug)]
pub struct Block {
    pub expressions: Vec<CodeNode>,
    pub id: ID,
}
