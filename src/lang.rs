use std::fmt;
use std::borrow::Borrow;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::iter;

#[cfg(feature = "javascript")]
use stdweb::{js,_js_impl};

use uuid::Uuid;
use lazy_static::lazy_static;
use objekt::{clone_trait_object,__internal_clone_trait_object};
use downcast_rs::impl_downcast;
use serde_derive::{Serialize,Deserialize};

use super::ExecutionEnvironment;

lazy_static! {
    pub static ref NULL_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "Null".to_string(),
        id: uuid::Uuid::parse_str(&"daa07233-b887-4512-b06e-d6a53d415213").unwrap(),
        symbol: "\u{f192}".to_string(),
        num_params: 0,
    };

    pub static ref STRING_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "String".to_string(),
        id: uuid::Uuid::parse_str("e0e8271e-5f94-4d00-bad9-46a2ce4d6568").unwrap(),
        symbol: "\u{f10d}".to_string(),
        num_params: 0,
    };

    pub static ref RESULT_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "Result".to_string(),
        id: uuid::Uuid::parse_str("0613664d-eead-4d83-99a0-9759a5023887").unwrap(),
        symbol: "\u{f493}".to_string(),
        num_params: 0,
    };

    pub static ref NUMBER_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "Number".to_string(),
        id: uuid::Uuid::parse_str("6dbe9096-4ff5-42f1-b2ff-36eacc3ced59").unwrap(),
        symbol: "\u{f292}".to_string(),
        num_params: 0,
    };


    pub static ref LIST_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "List".to_string(),
        id: uuid::Uuid::parse_str("4c726a5e-d9c2-481b-bbe8-ca5319176aad").unwrap(),
        symbol: "\u{f03a}".to_string(),
        num_params: 1,
    };
}

pub trait Function: objekt::Clone + downcast_rs::Downcast {
    fn call(&self, env: &mut ExecutionEnvironment, args: HashMap<ID,Value>) -> Value;
    fn name(&self) -> &str;
    fn id(&self) -> ID;
    fn takes_args(&self) -> Vec<ArgumentDefinition>;
    fn returns(&self) -> Type;
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<Function: {}>", self.id())
    }
}

clone_trait_object!(Function);
impl_downcast!(Function);

#[derive(Deserialize, Serialize, Clone ,Debug, PartialEq)]
pub enum CodeNode {
    FunctionCall(FunctionCall),
    FunctionReference(FunctionReference),
    Argument(Argument),
    StringLiteral(StringLiteral),
    NullLiteral,
    Assignment(Assignment),
    Block(Block),
    VariableReference(VariableReference),
    FunctionDefinition(FunctionDefinition),
    Placeholder(Placeholder),
    StructLiteral(StructLiteral),
    StructLiteralField(StructLiteralField),
}

#[derive(Clone, Debug)]
pub enum Error {
    ArgumentError,
    UndefinedFunctionError(ID),
    // TODO: add metadata into here
    PythonError(String, String),
    PythonDeserializationError,
    JavaScriptDeserializationError,
    JavaScriptError(String, String),
}

#[derive(Clone, Debug)]
pub enum Value {
    Null,
    String(String),
    Error(Error),
    // TODO: be smarter amount infinite precision ints
    Number(i128),
    List(Vec<Value>),
    Struct {struct_id: ID, values: StructValues}
}

type StructValues = HashMap<ID, Value>;

#[derive(Deserialize, Serialize, Clone ,Debug, PartialEq)]
pub struct BuiltInTypeSpec {
    pub readable_name: String,
    pub id: ID,
    pub symbol: String,
    pub num_params: usize,
}

pub trait TypeSpec : objekt::Clone + downcast_rs::Downcast {
    fn readable_name(&self) -> &str;
    fn id(&self) -> ID;
    fn symbol(&self) -> &str;
    fn num_params(&self) -> usize;
    fn matches(&self, typespec_id: ID) -> bool {
        self.id() == typespec_id
    }
}

clone_trait_object!(TypeSpec);
impl_downcast!(TypeSpec);

impl TypeSpec for BuiltInTypeSpec {
    fn readable_name(&self) -> &str {
        &self.readable_name
    }

    fn id(&self) -> ID {
        self.id
    }

    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn num_params(&self) -> usize {
        self.num_params
    }
}

impl BuiltInTypeSpec {
    pub fn matches(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

#[derive(Deserialize, Serialize, Clone ,Debug, PartialEq)]
pub struct Type {
    pub typespec_id: ID,
    pub params: Vec<Type>,
}

impl Type {
    // for types with no params
    pub fn from_spec<T: TypeSpec>(spec: &T) -> Self {
        Self::with_params(spec, vec![])
    }

    pub fn with_params<T: TypeSpec>(spec: &T, params: Vec<Self>) -> Self {
        if params.len() != spec.num_params() {
            panic!("wrong number of params")
        }
        Self {
            typespec_id: spec.id(),
            params,
        }
    }

    pub fn id(&self) -> ID {
        let mut mashed_hashes = vec![self.typespec_id.to_string()];
        mashed_hashes.extend(self.params.iter().map(|t| t.id().to_string()));
        // v5 uuids aren't random, they are hashes
        uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_OID,
            mashed_hashes.join(":").as_bytes())
    }

    pub fn matches(&self, other: &Self) -> bool {
        self.id() == other.id()
    }

    // XXX: idk if this is right but it'll at least get me farther i think
    pub fn matches_spec(&self, spec: &BuiltInTypeSpec) -> bool {
        self.typespec_id == spec.id
    }
}

impl CodeNode {
    pub fn into_argument(&self) -> &Argument {
        match self {
            CodeNode::Argument(ref argument) => argument,
            _ => panic!("tried converting into argument but this ain't an argument")
        }
    }

    pub fn into_assignment(&self) -> Option<&Assignment> {
        match self {
            CodeNode::Assignment(ref assignment) => Some(assignment),
            _ => None,
        }
    }

    pub fn into_block(&self) -> Option<&Block> {
        if let CodeNode::Block(ref block) = self {
            Some(block)
        } else {
            None
        }
    }

    pub fn into_struct_literal(&self) -> Option<&StructLiteral> {
        if let CodeNode::StructLiteral(ref struct_literal) = self {
            Some(struct_literal)
        } else {
            None
        }
    }

    pub fn into_struct_literal_field(&self) -> Option<&StructLiteralField> {
        if let CodeNode::StructLiteralField(ref field) = self {
            Some(field)
        } else {
            None
        }
    }

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
            CodeNode::Argument(argument) => {
                format!("Argument: ID {}", argument.id)
            }
            CodeNode::Placeholder(placeholder) => {
                format!("Placeholder: {}", placeholder.description)
            },
            CodeNode::NullLiteral => {
                "NullLiteral".to_string()
            },
            CodeNode::StructLiteral(struct_literal) => {
                format!("Struct literal: {}", struct_literal.id)
            },
            CodeNode::StructLiteralField(field) => {
                format!("Struct literal field: {}", field.id)
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
            CodeNode::Argument(argument) => {
                argument.id
            }
            CodeNode::Placeholder(placeholder) => {
                placeholder.id
            },
            CodeNode::NullLiteral => {
                uuid::Uuid::parse_str("1a2de9c5-043c-43c8-ad05-622bb278d5ab").unwrap()
            },
            CodeNode::StructLiteral(struct_literal) => struct_literal.id,
            CodeNode::StructLiteralField(field) => field.id,
        }
    }

    pub fn previous_child(&self, node_id: ID) -> Option<&CodeNode> {
        let children = self.children();
        let position = children.iter()
            .position(|n| n.id() == node_id);
        if let Some(position) = position {
            if position > 0 {
                if let Some(next) = children.get(position - 1) {
                    return Some(next)
                }
            }
        }
        None
    }

    pub fn next_child(&self, node_id: ID) -> Option<&CodeNode> {
        let children = self.children();
        let position = children.iter()
            .position(|n| n.id() == node_id);
        if let Some(position) = position {
            if let Some(next) = children.get(position + 1) {
                return Some(next)
            }
        }
        None
    }

    pub fn children(&self) -> Vec<&CodeNode> {
        self.children_iter().collect()
    }

    pub fn children_iter<'a>(&'a self) -> Box<Iterator<Item = &'a CodeNode> +'a> {
        match self {
            CodeNode::FunctionCall(function_call) => {
               Box::new(iter::once(function_call.function_reference.as_ref())
                    .chain(function_call.args.iter()))
            }
            CodeNode::StringLiteral(_) => {
                Box::new(iter::empty())
            }
            CodeNode::Assignment(assignment) => {
                Box::new(iter::once(assignment.expression.borrow()))
            }
            CodeNode::Block(block) => {
                Box::new(block.expressions.iter())
            }
            CodeNode::VariableReference(_) => {
                Box::new(iter::empty())
            }
            CodeNode::FunctionDefinition(_) => {
                Box::new(iter::empty())
            }
            CodeNode::FunctionReference(_) => {
                Box::new(iter::empty())
            }
            CodeNode::Argument(argument) => {
                Box::new(iter::once(argument.expr.borrow()))
            }
            CodeNode::Placeholder(_) => {
                Box::new(iter::empty())
            }
            CodeNode::NullLiteral => Box::new(iter::empty()),
            CodeNode::StructLiteral(struct_literal) => {
                Box::new(struct_literal.fields.iter())
            },
            CodeNode::StructLiteralField(field) => {
                Box::new(iter::once(field.expr.borrow()))
            }
        }
    }

    pub fn self_with_all_children_dfs(&self) -> impl Iterator<Item = &CodeNode> {
        iter::once(self).chain(self.all_children_dfs_iter())
    }

    pub fn all_children_dfs_iter<'a>(&'a self) -> Box<Iterator<Item = &'a CodeNode> + 'a> {
        Box::new(self.children_iter()
            .flat_map(|child| {
                iter::once(child).chain(child.all_children_dfs_iter())
            })
        )
    }

    pub fn all_children_dfs(&self) -> Vec<&CodeNode> {
        self.all_children_dfs_iter().collect()
    }

    pub fn children_mut(&mut self) -> Vec<&mut CodeNode> {
        match self {
            CodeNode::FunctionCall(function_call) => {
                let mut children : Vec<&mut CodeNode> = function_call.args
                    .iter_mut()
                    .collect();
                children.insert(0, &mut function_call.function_reference);
                children
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
            CodeNode::Argument(argument) => {
                vec![argument.expr.borrow_mut()]
            }
            CodeNode::Placeholder(_placeholder) => {
                vec![]
            }
            CodeNode::NullLiteral => vec![],
            CodeNode::StructLiteral(struct_literal) => {
                struct_literal.fields.iter_mut().collect()
            },
            CodeNode::StructLiteralField(field) => {
                vec![field.expr.borrow_mut()]
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

    pub fn find_node(&self, id: ID) -> Option<&CodeNode> {
        if self.id() == id {
            Some(self)
        } else {
            for child in self.children_iter() {
                if let Some(found_node) = child.find_node(id) {
                    return Some(found_node)
                }
            }
            None
        }
    }

    pub fn find_parent(&self, id: ID) -> Option<&CodeNode> {
        if self.id() == id {
            return None
        } else {
            for child in self.children_iter() {
                if child.id() == id {
                    return Some(self)
                } else {
                    let found_parent = child.find_parent(id);
                    if let Some(code_node) = found_parent {
                        return Some(code_node)
                    }
                }
            }
        }
        None
    }
}

pub type ID = Uuid;

#[cfg(feature = "default")]
pub fn new_id() -> ID {
    Uuid::new_v4()
}

// this is weird. without this, we get an error about RNG not being available in the browser. got
// this browser uuid implementation utilizing crypto from https://stackoverflow.com/a/2117523/149987
#[cfg(feature = "javascript")]
pub fn new_id() -> ID {
    let uuid = js! {
        return ([1e7]+-1e3+-4e3+-8e3+-1e11).replace(new RegExp("[018]", "g"), c =>
            (c ^ crypto.getRandomValues(new Uint8Array(1))[0] & 15 >> c / 4).toString(16)
        );
    };
    Uuid::parse_str(&uuid.into_string().unwrap()).unwrap()
}

#[derive(Deserialize, Serialize, Clone,Debug, PartialEq)]
pub struct StringLiteral {
    pub value: String,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone,Debug, PartialEq)]
pub struct FunctionCall {
    pub function_reference: Box<CodeNode>,
    pub args: Vec<CodeNode>,
    pub id: ID,
}

impl FunctionCall {
    pub fn function_reference(&self) -> &FunctionReference {
        match *self.function_reference {
            CodeNode::FunctionReference(ref function_reference) => function_reference,
            _ => panic!("tried converting into argument but this ain't an argument")
        }
    }

    pub fn args(&self) -> Vec<&Argument> {
        self.args.iter().map(|arg| arg.into_argument()).collect()
    }
}

#[derive(Deserialize, Serialize, Clone,Debug, PartialEq)]
pub struct Assignment {
    pub name: String,
    // TODO: consider differentiating between CodeNodes and Expressions.
    pub expression: Box<CodeNode>,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone,Debug, PartialEq)]
pub struct Block {
    pub expressions: Vec<CodeNode>,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct VariableReference {
    pub assignment_id: ID,
    pub id: ID,
}


#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct FunctionReference {
    pub function_id: ID,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone,Debug, PartialEq)]
pub struct FunctionDefinition {
    pub name: String,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone ,Debug, PartialEq)]
pub struct ArgumentDefinition {
    pub id: ID,
    pub arg_type: Type,
    pub short_name: String,
}

impl ArgumentDefinition {
    pub fn new(arg_type: Type, short_name: String) -> ArgumentDefinition {
        Self::new_with_id(new_id(), arg_type, short_name)
    }

    pub fn new_with_id(id: ID, arg_type: Type, short_name: String) -> ArgumentDefinition {
        ArgumentDefinition { id, short_name, arg_type }
    }
}

#[derive(Deserialize, Serialize, Clone ,Debug, PartialEq)]
pub struct Argument {
    pub id: ID,
    pub argument_definition_id: ID,
    pub expr: Box<CodeNode>,
}

#[derive(Deserialize, Serialize, Clone ,Debug, PartialEq)]
pub struct Placeholder {
    pub id: ID,
    pub description: String,
    pub type_id: ID,
}

#[derive(Deserialize, Serialize, Clone ,Debug, PartialEq)]
pub struct StructLiteral {
    pub id: ID,
    pub struct_id: ID,
    pub fields: Vec<CodeNode>,
}

impl StructLiteral {
    pub fn fields(&self) -> impl Iterator<Item = &StructLiteralField> {
        self.fields.iter().map(|f| f.into_struct_literal_field().unwrap())
    }
}

#[derive(Deserialize, Serialize, Clone ,Debug, PartialEq)]
pub struct StructLiteralField {
    pub id: ID,
    pub struct_field_id: ID,
    pub expr: Box<CodeNode>,
}
