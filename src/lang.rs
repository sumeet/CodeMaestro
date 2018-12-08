use std::fmt;
use std::borrow::Borrow;
use std::borrow::BorrowMut;
use std::collections::HashMap;

use super::ExecutionEnvironment;
use super::uuid::Uuid;


lazy_static! {
    pub static ref NULL_TYPESPEC: TypeSpec = TypeSpec {
        readable_name: "Null".to_string(),
        id: uuid::Uuid::parse_str(&"daa07233-b887-4512-b06e-d6a53d415213").unwrap(),
        symbol: "\u{f192}".to_string(),
        num_params: 0,
    };

    pub static ref STRING_TYPESPEC: TypeSpec = TypeSpec {
        readable_name: "String".to_string(),
        id: uuid::Uuid::parse_str("e0e8271e-5f94-4d00-bad9-46a2ce4d6568").unwrap(),
        symbol: "\u{f10d}".to_string(),
        num_params: 0,
    };

    pub static ref RESULT_TYPESPEC: TypeSpec = TypeSpec {
        readable_name: "Result".to_string(),
        id: uuid::Uuid::parse_str("0613664d-eead-4d83-99a0-9759a5023887").unwrap(),
        symbol: "\u{f493}".to_string(),
        num_params: 0,
    };

    pub static ref NUMBER_TYPESPEC: TypeSpec = TypeSpec {
        readable_name: "Number".to_string(),
        id: uuid::Uuid::parse_str("6dbe9096-4ff5-42f1-b2ff-36eacc3ced59").unwrap(),
        symbol: "\u{f292}".to_string(),
        num_params: 0,
    };


    pub static ref LIST_TYPESPEC: TypeSpec = TypeSpec {
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
    Assignment(Assignment),
    Block(Block),
    VariableReference(VariableReference),
    FunctionDefinition(FunctionDefinition),
    Placeholder(Placeholder),
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
}

#[derive(Deserialize, Serialize, Clone ,Debug, PartialEq)]
pub struct TypeSpec {
    pub readable_name: String,
    pub id: ID,
    pub symbol: String,
    pub num_params: usize,
}

#[derive(Deserialize, Serialize, Clone ,Debug, PartialEq)]
pub struct Type {
    // TODO: should this just be a TypeSpec ID?
    pub typespec: TypeSpec,
    pub params: Vec<Type>,
}

impl Type {
    // for types with no params
    pub fn from_spec(spec: &TypeSpec) -> Self {
        Self::with_params(spec, vec![])
    }

    pub fn with_params(spec: &TypeSpec, params: Vec<Self>) -> Self {
        if params.len() != spec.num_params {
            panic!("wrong number of params")
        }
        Self {
            typespec: spec.clone(),
            params,
        }
    }

    pub fn id(&self) -> ID {
        let mut mashed_hashes = vec![self.typespec.id.to_string()];
        mashed_hashes.extend(self.params.iter().map(|t| t.id().to_string()));
        uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_OID,
            mashed_hashes.join(":").as_bytes())
    }

    pub fn readable_name(&self) -> String {
        if self.params.is_empty() {
            return self.typespec.readable_name.clone()
        }
        let param_names : Vec<String> = self.params.iter()
            .map(|param| param.readable_name())
            .collect();
        format!("{}<{}>", self.typespec.readable_name, param_names.join(","))
    }

    pub fn symbol(&self) -> String {
        if self.params.is_empty() {
            return self.typespec.symbol.clone()
        }
        let param_symbols : Vec<String> = self.params.iter()
            .map(|param| param.symbol())
            .collect();
        format!("{}<{}>", self.typespec.symbol, param_symbols.join(","))
    }

    pub fn matches(&self, other: &Self) -> bool {
        self.id() == other.id()
    }

    // XXX: idk if this is right but it'll at least get me farther i think
    pub fn matches_spec(&self, spec: &TypeSpec) -> bool {
        self.typespec.id == spec.id
    }
}

impl CodeNode {
    pub fn into_argument(&self) -> &Argument {
        match self {
            CodeNode::Argument(argument) => argument,
            _ => panic!("tried converting into argument but this ain't an argument")
        }
    }

    pub fn into_assignment(&self) -> Option<&Assignment> {
        match self {
            CodeNode::Assignment(assignment) => Some(assignment),
            _ => None,
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
            }
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
        match self {
            CodeNode::FunctionCall(function_call) => {
                let mut children : Vec<&CodeNode> = function_call.args.iter().collect();
                children.insert(0, function_call.function_reference.as_ref());
                children
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
                vec![]
            }
            CodeNode::FunctionDefinition(_) => {
                vec![]
            }
            CodeNode::FunctionReference(_) => {
                vec![]
            }
            CodeNode::Argument(argument) => {
                vec![argument.expr.borrow()]
            }
            CodeNode::Placeholder(_) => {
                vec![]
            }
        }
    }

    pub fn all_children_dfs(&self) -> Vec<&CodeNode> {
        let mut all_children = vec![];
        for child in self.children() {
            all_children.push(child);
            all_children.extend(child.all_children_dfs().iter())
        }
        all_children
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
            for child in self.children() {
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
            let children = self.children();
            for child in children {
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
    // ok this is weird. it looks like i only need an ID here for the type... but in other places,
    // like the ArgumentDefinition, i store the entire type. i'm pretty sure i can go back into the
    // ArgumentDefinition and change that into a type ID instead of the entire type. there's just
    // this inconsistency.
    pub type_id: ID,
}
