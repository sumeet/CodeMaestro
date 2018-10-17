use std::fmt;
use std::borrow::Borrow;
use std::borrow::BorrowMut;
use std::rc::Rc;
use std::collections::HashMap;

use serde::ser::{Serialize, Serializer, SerializeSeq, SerializeMap};

use super::ExecutionEnvironment;
use super::uuid::Uuid;

lazy_static! {
    pub static ref NULL_TYPE: Type = Type {
        readable_name: "Null".to_string(),
        id: uuid::Uuid::parse_str(&"daa07233-b887-4512-b06e-d6a53d415213").unwrap(),
    };

    pub static ref STRING_TYPE: Type = Type {
        readable_name: "String".to_string(),
        id: uuid::Uuid::parse_str("e0e8271e-5f94-4d00-bad9-46a2ce4d6568").unwrap(),
    };

    pub static ref RESULT_TYPE: Type = Type {
        readable_name: "Result".to_string(),
        id: uuid::Uuid::parse_str("0613664d-eead-4d83-99a0-9759a5023887").unwrap(),
    };
}

pub trait Function: objekt::Clone {
    fn call(&self, env: &mut ExecutionEnvironment, args: HashMap<ID,Value>) -> Value;
    fn name(&self) -> &str;
    fn id(&self) -> ID;
    fn takes_args(&self) -> Vec<ArgumentDefinition>;
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<Function: {}>", self.id())
    }
}

clone_trait_object!(Function);

#[derive(Deserialize, Serialize, Clone ,Debug)]
pub enum CodeNode {
    FunctionCall(FunctionCall),
    StringLiteral(StringLiteral),
    Assignment(Assignment),
    Block(Block),
    VariableReference(VariableReference),
    FunctionReference(FunctionReference),
    FunctionDefinition(FunctionDefinition),
}

#[derive(Clone, Debug)]
pub enum Error {
    ArgumentError,
    UndefinedFunctionError(ID),
}

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    String(String),
    Result(Result<Box<Value>,Error>)
}

impl Value {
    pub fn get_type(&self) -> &'static Type {
        match self {
            Value::Null => &NULL_TYPE,
            Value::String(_) => &STRING_TYPE,
            Value::Result(_) => &RESULT_TYPE,
        }
    }
}

#[derive(Deserialize, Serialize, Clone ,Debug)]
pub struct Type {
    readable_name: String,
    id: ID,
}

impl CodeNode {
    pub fn description(&self) -> String {
        match self {
            CodeNode::FunctionCall(function_call) => {
                format!("Function call: {}", function_call.id)
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

    pub fn previous_child(&mut self, node_id: ID) -> Option<CodeNode> {
        let children = self.children_mut();
        let position = children.iter().position(|n| n.id() == node_id);
        if let(Some(position)) = position {
            if position > 0 {
                if let (Some(next)) = children.get(position - 1) {
                    return Some((*next).clone())
                }
            }
        }
        None
    }

    pub fn next_child(&mut self, node_id: ID) -> Option<CodeNode> {
        let children = self.children_mut();
        let position = children.iter()
            .position(|n| n.id() == node_id);
        if let(Some(position)) = position {
            if let (Some(next)) = children.get(position + 1) {
                return Some((*next).clone())
            }
        }
        None
    }

    pub fn children(&self) -> Vec<&CodeNode> {
        match self {
            CodeNode::FunctionCall(function_call) => {
                function_call.args.iter().map(|arg| &arg.expr).collect()
            }
            CodeNode::StringLiteral(_) => {
                Vec::new()
            }
            CodeNode::Assignment(assignment) => {
                vec![assignment.expression.borrow()]
            }
            CodeNode::Block(block) => {
                block.expressions.iter().collect()
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

    pub fn children_mut(&mut self) -> Vec<&mut CodeNode> {
        match self {
            CodeNode::FunctionCall(function_call) => {
                function_call.args.iter_mut().map(|arg| &mut arg.expr).collect()
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
            for child in self.children_mut() {
                child.replace(code_node)
            }
        }
    }

    pub fn find_node(&mut self, id: ID) -> Option<&CodeNode> {
        if self.id() == id {
            Some(self)
        } else {
            for child in self.children_mut() {
                if let(Some(found_node)) = child.find_node(id) {
                    return Some(found_node)
                }
            }
            None
        }
    }

    pub fn find_parent(&mut self, id: ID) -> Option<CodeNode> {
        if self.id() == id {
            return None
        } else {
            let children = self.children_mut();
            for child in children {
                if child.id() == id {
                    return Some(self.clone())
                } else {
                    let found_parent = child.find_parent(id);
                    if let(Some(code_node)) = found_parent {
                        return Some(code_node)
                    }
                }
            }
        }
        None
    }
}

pub type ID = Uuid;

#[derive(Deserialize, Serialize, Clone,Debug)]
pub struct StringLiteral {
    pub value: String,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone,Debug)]
pub struct FunctionCall {
    pub function_reference: FunctionReference,
    pub args: Vec<Argument>,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone,Debug)]
pub struct Assignment {
    pub name: String,
    // TODO: consider differentiating between CodeNodes and Expressions.
    pub expression: Box<CodeNode>,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone,Debug)]
pub struct Block {
    pub expressions: Vec<CodeNode>,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct VariableReference {
    pub assignment_id: ID,
    pub id: ID,
}


#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct FunctionReference {
    pub function_id: ID,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone,Debug)]
pub struct FunctionDefinition {
    pub name: String,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone ,Debug)]
pub struct ArgumentDefinition {
    pub id: ID,
    pub arg_type: Type,
    pub short_name: String,
}

impl ArgumentDefinition {
    pub fn new(id: ID, arg_type: Type, short_name: String) -> ArgumentDefinition {
        ArgumentDefinition { id, short_name, arg_type }
    }
}

#[derive(Deserialize, Serialize, Clone ,Debug)]
pub struct Argument {
    pub id: ID,
    pub argument_definition_id: ID,
    pub expr: CodeNode,
}
