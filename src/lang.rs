use std::fmt;
use std::borrow::BorrowMut;
use std::rc::Rc;

use super::{erased_serde};
use serde::ser::{Serialize, Serializer, SerializeSeq, SerializeMap};

use super::ExecutionEnvironment;

pub trait Function: objekt::Clone {
    fn call(&self, env: &mut ExecutionEnvironment, args: Vec<Value>) -> Value;
    fn uuid(&self) -> &str;
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<Function: {}>", self.uuid())
    }
}

// TODO: this way of serializing functions won't fly. later, figure out how the program should
// actually reference functions
impl Serialize for Function {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("temp_function_by_uuid", self.uuid())?;
        map.end()
    }
}

clone_trait_object!(Function);
//serialize_trait_object!(Function);

#[derive(Serialize, Clone ,Debug)]
pub enum CodeNode {
    FunctionCall(FunctionCall),
    StringLiteral(StringLiteral),
    Assignment(Assignment),
    Block(Block),
    VariableReference(VariableReference),
    FunctionReference(FunctionReference),
    FunctionDefinition(FunctionDefinition),
}

pub trait BuiltinFunction {
    fn call(&self, env: &mut ExecutionEnvironment, args: Vec<Value>) -> Value;
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
            CodeNode::VariableReference(variable_reference) => {
                env.get_local_variable(variable_reference.assignment_id).unwrap().clone()
            }
            CodeNode::FunctionReference(_) => { Value::Null }
            CodeNode::FunctionDefinition(_) => { Value::Null }
        }
    }

    pub fn description(&self) -> String {
        match self {
            CodeNode::FunctionCall(function_call) => {
                format!("Function call: {}", function_call.function.uuid())
            }
            CodeNode::StringLiteral(string_literal) => {
                format!("String literal: {}", string_literal.value)
            }
            CodeNode::Assignment(assignment) => {
                format!("Assignment: {}", assignment.name)
            }
            CodeNode::Block(block) => {
                format!("Code block: ID {}", block.id)
            }
            CodeNode::VariableReference(variable_reference) => {
                format!("Variable reference: Assignment ID {}", variable_reference.assignment_id)
            }
            CodeNode::FunctionReference(function_reference) => {
                format!("Function reference: {:?}", function_reference)
            }
            CodeNode::FunctionDefinition(function_definition) => {
                format!("Function reference: Name {}", function_definition.name)
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
            CodeNode::VariableReference(variable_reference) => {
                variable_reference.id
            }
            CodeNode::FunctionDefinition(function_definition) => {
                function_definition.id
            }
            CodeNode::FunctionReference(function_reference) => {
                function_reference.id
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
            CodeNode::VariableReference(_) => {
                Vec::new()
            }
            CodeNode::FunctionDefinition(_) => {
                Vec::new()
            }
            CodeNode::FunctionReference(_) => {
                Vec::new()
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

    pub fn find_node(&mut self, id: ID) -> Option<&CodeNode> {
        if self.id() == id {
            Some(self)
        } else {
            for child in self.children() {
                if let(Some(found_node)) = child.find_node(id) {
                    return Some(found_node)
                }
            }
            None
        }
    }
}

pub type ID = u64;

#[derive(Serialize, Clone,Debug)]
pub struct StringLiteral {
    pub value: String,
    pub id: ID,
}

#[derive(Serialize, Clone,Debug)]
pub struct FunctionCall {
    pub function: Box<Function>,
    pub args: Vec<CodeNode>,
    pub id: ID,
}

#[derive(Serialize, Clone,Debug)]
pub struct Assignment {
    pub name: String,
    // TODO: consider differentiating between CodeNodes and Expressions.
    pub expression: Box<CodeNode>,
    pub id: ID,
}

#[derive(Serialize, Clone,Debug)]
pub struct Block {
    pub expressions: Vec<CodeNode>,
    pub id: ID,
}

#[derive(Serialize, Clone,Debug)]
pub struct VariableReference {
    pub assignment_id: ID,
    pub id: ID,
}


#[derive(Serialize, Clone, Debug)]
pub struct FunctionReference {
    function_id: ID,
    id: ID,
}

#[derive(Serialize, Clone,Debug)]
pub struct FunctionDefinition {
    pub name: String,
    pub id: ID,
}
