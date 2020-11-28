use std::borrow::Borrow;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::iter;

#[cfg(target_arch = "wasm32")]
use stdweb::js;

use downcast_rs::impl_downcast;
use itertools::Itertools;
use lazy_static::lazy_static;
use objekt::clone_trait_object;
use serde_derive::{Deserialize, Serialize};
use uuid::Uuid;

use super::env;

lazy_static! {
    pub static ref NULL_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "Null".to_string(),
        description: "Null: Represents nothing".into(),
        id: uuid::Uuid::parse_str(&"daa07233-b887-4512-b06e-d6a53d415213").unwrap(),
        symbol: "\u{f192}".to_string(),
        num_params: 0,
    };
    pub static ref BOOLEAN_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "Boolean".to_string(),
        description: "Either true or false".into(),
        id: uuid::Uuid::parse_str(&"d00d688f-0c9e-43af-a19f-ab02e46b4c2c").unwrap(),
        symbol: "\u{f059}".to_string(),
        num_params: 0,
    };
    pub static ref STRING_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "String".to_string(),
        description: "Plain text".into(),
        id: uuid::Uuid::parse_str("e0e8271e-5f94-4d00-bad9-46a2ce4d6568").unwrap(),
        symbol: "\u{f10d}".to_string(),
        num_params: 0,
    };
    pub static ref NUMBER_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "Number".to_string(),
        description: "A numerical value. For example: 1, 2, -1, 10384, 42, etc.".into(),
        id: uuid::Uuid::parse_str("6dbe9096-4ff5-42f1-b2ff-36eacc3ced59").unwrap(),
        symbol: "\u{f292}".to_string(),
        num_params: 0,
    };
    pub static ref LIST_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "List".to_string(),
        description: "A collection of one or more items".into(),
        id: uuid::Uuid::parse_str("4c726a5e-d9c2-481b-bbe8-ca5319176aad").unwrap(),
        symbol: "\u{f03a}".to_string(),
        num_params: 1,
    };
    pub static ref ERROR_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "Error".to_string(),
        description: "Means there was an error".into(),
        id: uuid::Uuid::parse_str("a6ad92ed-1b21-44fe-9ad0-e08326acd6f6").unwrap(),
        symbol: "\u{f06a}".to_string(),
        num_params: 0,
    };
    pub static ref ANON_FUNC_TYPESPEC: BuiltInTypeSpec = BuiltInTypeSpec {
        readable_name: "Executable code".to_string(),
        description: "Callback code that can be run".into(),
        id: uuid::Uuid::parse_str("92fe8555-2f8c-4ae5-aca6-42353f6dc888").unwrap(),
        symbol: "\u{f661}".to_string(),
        num_params: 2,
    };
    pub static ref ANY_TYPESPEC: AnyTypeSpec = AnyTypeSpec { };

    pub static ref BUILT_IN_TYPESPECS : Vec<Box<dyn TypeSpec>> = vec![
        Box::new(NULL_TYPESPEC.clone()), Box::new(BOOLEAN_TYPESPEC.clone()),
        Box::new(STRING_TYPESPEC.clone()),
        Box::new(NUMBER_TYPESPEC.clone()),
        Box::new(LIST_TYPESPEC.clone()), Box::new(ERROR_TYPESPEC.clone()),
        Box::new(ANON_FUNC_TYPESPEC.clone()), Box::new(ANY_TYPESPEC.clone())];
}

#[derive(Clone, Serialize, Deserialize)]
pub struct GenericParamTypeSpec {
    id: ID,
    // resolved_param: Option<Type>,
}

impl GenericParamTypeSpec {
    pub fn new(id: ID) -> Self {
        Self { id }
        // resolved_param: None }
    }
}

#[typetag::serde()]
impl TypeSpec for GenericParamTypeSpec {
    fn readable_name(&self) -> &str {
        "Any Type"
    }

    fn description(&self) -> &str {
        "Any type can be used here"
    }

    fn id(&self) -> ID {
        self.id
    }

    fn symbol(&self) -> &str {
        "\u{f0c8}"
    }

    fn num_params(&self) -> usize {
        return 0;
    }

    fn matches(&self, _: ID) -> bool {
        return true;
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AnyTypeSpec {}

#[typetag::serde()]
impl TypeSpec for AnyTypeSpec {
    fn readable_name(&self) -> &str {
        "Any Type"
    }

    fn description(&self) -> &str {
        "Any type can be used here"
    }

    fn id(&self) -> ID {
        Uuid::parse_str("8b83b98f-2b2c-42c3-b819-bb6b29972320").unwrap()
    }

    fn symbol(&self) -> &str {
        "\u{f12a}"
    }

    fn num_params(&self) -> usize {
        return 0;
    }

    fn matches(&self, _: ID) -> bool {
        return true;
    }
}

#[typetag::serde(tag = "type")]
pub trait Function: objekt::Clone + downcast_rs::Downcast + Send + Sync {
    fn call(&self, interpreter: env::Interpreter, args: HashMap<ID, Value>) -> Value;
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn id(&self) -> ID;
    fn takes_args(&self) -> Vec<ArgumentDefinition>;
    fn returns(&self) -> Type;
    fn style(&self) -> &FunctionRenderingStyle {
        &FunctionRenderingStyle::Default
    }
    fn cs_code(&self) -> Box<dyn Iterator<Item = &Block> + '_> {
        Box::new(std::iter::empty())
    }
    fn defines_generics(&self) -> Vec<GenericParamTypeSpec> {
        vec![]
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub enum FunctionRenderingStyle {
    Default,
    Infix(Vec<ID>, String),
}

impl fmt::Debug for dyn Function {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<Function: {}>", self.id())
    }
}

clone_trait_object!(Function);
impl_downcast!(Function);

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub enum CodeNode {
    FunctionCall(FunctionCall),
    FunctionReference(FunctionReference),
    Argument(Argument),
    StringLiteral(StringLiteral),
    NullLiteral(ID),
    Assignment(Assignment),
    Reassignment(Reassignment),
    Block(Block),
    AnonymousFunction(AnonymousFunction),
    VariableReference(VariableReference),
    Placeholder(Placeholder),
    StructLiteral(StructLiteral),
    StructLiteralField(StructLiteralField),
    Conditional(Conditional),
    WhileLoop(WhileLoop),
    Match(Match),
    ListLiteral(ListLiteral),
    StructFieldGet(StructFieldGet),
    NumberLiteral(NumberLiteral),
    ListIndex(ListIndex),
    ReassignListIndex(ReassignListIndex),
    EnumVariantLiteral(EnumVariantLiteral),
    EarlyReturn(EarlyReturn),
    Try(Try),
}

use crate::code_generation::new_block;
use futures_util::future::Shared;
use futures_util::FutureExt;
use std::collections::BTreeMap;
use std::pin::Pin;

#[derive(Clone, Debug)]
pub struct ValueFuture(pub Shared<Pin<Box<dyn Future<Output = Value>>>>);

impl PartialEq for ValueFuture {
    fn eq(&self, _: &Self) -> bool {
        panic!("comparison doesn't work for futures")
    }
}

pub type StructValues = HashMap<ID, Value>;
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Boolean(bool),
    String(String),
    // TODO: be smarter amount infinite precision ints
    Number(i128),
    List(Type, Vec<Value>),
    Struct { struct_id: ID, values: StructValues },
    Future(ValueFuture),
    EnumVariant { variant_id: ID, value: Box<Value> },
    AnonymousFunction(AnonymousFunction),
    EarlyReturn(Box<Value>),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AnonymousFunction {
    pub id: ID,
    // TODO: take more than one argument
    pub takes_arg: ArgumentDefinition,
    pub returns: Type,
    pub block: Box<CodeNode>,
}

impl AnonymousFunction {
    pub fn new(takes_arg: ArgumentDefinition, returns: Type) -> Self {
        Self { id: new_id(),
               takes_arg,
               returns,
               block: Box::new(CodeNode::Block(new_block(vec![]))) }
    }
}

impl Value {
    pub fn unwrap_early_return(self) -> Self {
        match self {
            Value::EarlyReturn(inner) => *inner,
            otherwise => otherwise,
        }
    }

    pub fn is_early_return(&self) -> bool {
        match self {
            Value::EarlyReturn(_) => true,
            _ => false,
        }
    }

    pub fn into_anon_func(self) -> Result<AnonymousFunction, Box<dyn std::error::Error>> {
        match self {
            Value::AnonymousFunction(af) => Ok(af),
            otherwise => Err(format!("expected AnonFunc but got {:?}", otherwise).into()),
        }
    }

    pub fn new_future(async_fn: impl Future<Output = Value> + 'static) -> Value {
        Value::Future(Self::new_value_future(async_fn))
    }

    pub fn new_value_future(async_fn: impl Future<Output = Value> + 'static) -> ValueFuture {
        ValueFuture(FutureExt::shared(Box::pin(async_fn)))
    }

    pub fn as_anon_func(&self) -> Result<&AnonymousFunction, Box<dyn std::error::Error>> {
        match self {
            Value::AnonymousFunction(af) => Ok(af),
            otherwise => Err(format!("expected AnonFunc but got {:?}", otherwise).into()),
        }
    }

    // should we use TryFrom / TryInto here...
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Result<&str, Box<dyn std::error::Error>> {
        match self {
            Value::String(s) => Ok(s),
            _ => Err(format!("expected String, but got {:?}", self).into()),
        }
    }

    pub fn as_vec_with_type(&self) -> Option<(&Type, &Vec<Value>)> {
        match self {
            Value::List(typ, v) => Some((typ, v)),
            _ => None,
        }
    }

    pub fn as_vec(&self) -> Option<&Vec<Value>> {
        match self {
            Value::List(_, v) => Some(v),
            _ => None,
        }
    }

    pub fn as_mut_vec(&mut self) -> Result<&mut Vec<Value>, Box<dyn std::error::Error>> {
        match self {
            Value::List(_, v) => Ok(v),
            otherwise => Err(format!("expected List, but this was a {:?}", otherwise).into()),
        }
    }

    pub fn into_string(self) -> Result<String, Self> {
        match self {
            Self::String(s) => Ok(s),
            _ => Err(self),
        }
    }

    pub fn into_vec(self) -> Option<Vec<Value>> {
        match self {
            Value::List(_, v) => Some(v),
            _ => None,
        }
    }

    pub fn into_vec_with_type(self) -> Option<(Type, Vec<Value>)> {
        match self {
            Value::List(typ, v) => Some((typ, v)),
            _ => None,
        }
    }

    pub fn as_i128(&self) -> Option<i128> {
        match self {
            Value::Number(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_struct(&self) -> Option<(ID, &StructValues)> {
        match self {
            Value::Struct { struct_id, values } => Some((*struct_id, values)),
            _ => None,
        }
    }

    pub fn into_struct(self) -> Option<(ID, StructValues)> {
        match self {
            Value::Struct { struct_id, values } => Some((struct_id, values)),
            _ => None,
        }
    }

    pub fn as_enum(&self) -> Option<(ID, &Value)> {
        match self {
            Value::EnumVariant { variant_id,
                                 box value, } => Some((*variant_id, value)),
            _ => None,
        }
    }

    pub fn into_enum(self) -> Result<(ID, Value), Box<dyn std::error::Error>> {
        match self {
            Value::EnumVariant { variant_id,
                                 box value, } => Ok((variant_id, value)),
            _ => Err(format!("expected enum, but got {:?}", self).into()),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct BuiltInTypeSpec {
    pub readable_name: String,
    pub description: String,
    pub id: ID,
    pub symbol: String,
    pub num_params: usize,
}

#[typetag::serde(tag = "type")]
pub trait TypeSpec: objekt::Clone + downcast_rs::Downcast + Send + Sync {
    fn readable_name(&self) -> &str;
    fn description(&self) -> &str;
    fn id(&self) -> ID;
    fn symbol(&self) -> &str;
    fn num_params(&self) -> usize;
    fn matches(&self, typespec_id: ID) -> bool {
        if typespec_id == ANY_TYPESPEC.id() {
            return true;
        }
        self.id() == typespec_id
    }
}

pub fn is_generic(typespec: &dyn TypeSpec) -> bool {
    typespec.downcast_ref::<GenericParamTypeSpec>().is_some()
}

clone_trait_object!(TypeSpec);
impl_downcast!(TypeSpec);

impl fmt::Debug for dyn TypeSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<TypeSpec: {}>", self.id())
    }
}

#[typetag::serde]
impl TypeSpec for BuiltInTypeSpec {
    fn readable_name(&self) -> &str {
        &self.readable_name
    }

    fn description(&self) -> &str {
        &self.description
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

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Type {
    pub typespec_id: ID,
    pub params: Vec<Type>,
}

impl Type {
    pub fn list_of(typ: Self) -> Self {
        Self::with_params(&*LIST_TYPESPEC, vec![typ])
    }

    pub fn get_param_using_path(&self, param_path: &[usize]) -> &Self {
        let mut param = self;
        for i in param_path {
            param = &param.params[*i]
        }
        param
    }

    pub fn get_param_using_path_mut(&mut self, param_path: &[usize]) -> &mut Self {
        let mut param = self;
        for i in param_path {
            param = &mut param.params[*i]
        }
        param
    }

    // for types with no params
    pub fn from_spec<T: TypeSpec>(spec: &T) -> Self {
        Self::with_params(spec, vec![])
    }

    pub fn with_params<T: TypeSpec>(spec: &T, params: Vec<Self>) -> Self {
        if params.len() != spec.num_params() {
            panic!("wrong number of params")
        }
        Self { typespec_id: spec.id(),
               params }
    }

    pub fn from_spec_id(typespec_id: ID, params: Vec<Self>) -> Self {
        Self { typespec_id,
               params }
    }

    // TODO: i forget why we need this
    pub fn hash(&self) -> ID {
        let mut mashed_hashes = vec![self.typespec_id.to_string()];
        mashed_hashes.extend(self.params.iter().map(|t| t.hash().to_string()));
        // v5 uuids aren't random, they are hashes
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID,
                           mashed_hashes.join(":").as_bytes())
    }

    // pub fn matches(&self, other: &Self) -> bool {
    //     // // TODO: duplication, search for ANY_TYPESPEC
    //     // if self.typespec_id == ANY_TYPESPEC.id() || other.typespec_id == ANY_TYPESPEC.id() {
    //     //     return true;
    //     // }
    //     if !self.matches_spec_id(other.typespec_id) {
    //         return false;
    //     }
    //     for (our_param, their_param) in self.params.iter().zip(other.params.iter()) {
    //         if !our_param.matches(their_param) {
    //             return false;
    //         }
    //     }
    //     true
    // }

    // XXX: idk if this is right but it'll at least get me farther i think
    pub fn matches_spec(&self, spec: &BuiltInTypeSpec) -> bool {
        self.matches_spec_id(spec.id)
    }

    pub fn matches_spec_id(&self, spec_id: ID) -> bool {
        if self.typespec_id == ANY_TYPESPEC.id() || spec_id == ANY_TYPESPEC.id() {
            return true;
        }
        self.typespec_id == spec_id
    }

    pub fn paths_to_params_containing_self(&self) -> Vec<Vec<usize>> {
        self.paths_to_params_containing_self_rec(&mut vec![])
    }

    pub fn paths_to_params_containing_self_rec(&self,
                                               previous_path: &mut Vec<usize>)
                                               -> Vec<Vec<usize>> {
        let mut ret = vec![];
        ret.push(previous_path.clone());
        for (i, param) in self.params.iter().enumerate() {
            previous_path.push(i);
            ret.extend_from_slice(&param.paths_to_params_containing_self_rec(previous_path));
            previous_path.pop();
        }
        ret
    }

    // pub fn params_iter_mut_containing_self(&mut self, func: &mut dyn FnMut(&mut Self, &[usize])) {
    //     let mut v = vec![];
    //     self.params_iter_mut_containing_self_rec(&mut v, func)
    // }
    //
    // pub fn params_iter_mut_containing_self_rec(&mut self,
    //                                            path_before: &mut Vec<usize>,
    //                                            func: &mut dyn FnMut(&mut Self, &[usize])) {
    //     func(self, &path_before);
    //     for (i, param) in self.params.iter_mut().enumerate() {
    //         path_before.push(i);
    //         param.params_iter_mut_containing_self_rec(path_before, func);
    //         path_before.pop();
    //     }
    // }
}

impl CodeNode {
    pub fn as_anon_func(&self) -> Result<&AnonymousFunction, Box<dyn std::error::Error>> {
        match self {
            CodeNode::AnonymousFunction(ref af) => Ok(af),
            _ => Err(format!("expected anonymous func, got {:?} instead", self).into()),
        }
    }

    pub fn as_assignment(&self) -> Result<&Assignment, Box<dyn std::error::Error>> {
        match self {
            CodeNode::Assignment(assignment) => Ok(assignment),
            _ => Err(format!("expected assignment, got {:?} instead", self).into()),
        }
    }

    pub fn as_function_call(&self) -> Result<&FunctionCall, Box<dyn std::error::Error>> {
        match self {
            CodeNode::FunctionCall(ref fc) => Ok(fc),
            _ => Err(format!("expected function call, got {:?} instead", self).into()),
        }
    }

    pub fn as_function_reference(&self) -> Result<&FunctionReference, Box<dyn std::error::Error>> {
        match self {
            CodeNode::FunctionReference(ref fr) => Ok(fr),
            _ => Err(format!("expected function reference, got {:?} instead", self).into()),
        }
    }

    pub fn into_list_literal(&self) -> &ListLiteral {
        match self {
            CodeNode::ListLiteral(ref list_literal) => list_literal,
            _ => panic!("tried converting into list literal but this ain't an list literal"),
        }
    }

    pub fn into_argument(&self) -> &Argument {
        match self {
            CodeNode::Argument(ref argument) => argument,
            _ => panic!("tried converting into argument but this ain't an argument"),
        }
    }

    pub fn into_block(self) -> Option<Block> {
        if let CodeNode::Block(block) = self {
            Some(block)
        } else {
            None
        }
    }

    pub fn as_block(&self) -> Option<&Block> {
        if let CodeNode::Block(ref block) = self {
            Some(block)
        } else {
            None
        }
    }

    pub fn into_placeholder(&self) -> Option<&Placeholder> {
        if let CodeNode::Placeholder(ref placeholder) = self {
            Some(placeholder)
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

    pub fn as_variable_reference(&self) -> Option<&VariableReference> {
        if let CodeNode::VariableReference(vr) = self {
            Some(vr)
        } else {
            None
        }
    }

    pub fn description(&self) -> String {
        match self {
            CodeNode::FunctionCall(function_call) => format!("Function call: {}", function_call.id),
            CodeNode::StringLiteral(string_literal) => {
                format!("String literal: {}", string_literal.value)
            }
            CodeNode::NumberLiteral(number_literal) => {
                format!("Number literal: {}", number_literal.value)
            }
            CodeNode::Assignment(assignment) => format!("Assignment: {}", assignment.name),
            CodeNode::Reassignment(reassignment) => format!("Reassignment: {}", reassignment.id),
            CodeNode::Block(block) => format!("Code block: ID {}", block.id),
            CodeNode::VariableReference(variable_reference) => {
                format!("Variable reference: Assignment ID {}",
                        variable_reference.assignment_id)
            }
            CodeNode::FunctionReference(function_reference) => {
                format!("Function reference: {:?}", function_reference)
            }
            CodeNode::Argument(argument) => format!("Argument: ID {}", argument.id),
            CodeNode::Placeholder(placeholder) => {
                format!("Placeholder: {}", placeholder.description)
            }
            CodeNode::NullLiteral(id) => format!("NullLiteral: ID {}", id),
            CodeNode::StructLiteral(struct_literal) => {
                format!("Struct literal: {}", struct_literal.id)
            }
            CodeNode::StructLiteralField(field) => format!("Struct literal field: {}", field.id),
            CodeNode::Conditional(conditional) => format!("Conditional: {}", conditional.id),
            CodeNode::ListLiteral(list_literal) => format!("List literal: {}", list_literal.id),
            CodeNode::Match(mach) => format!("Match: {}", mach.id),
            CodeNode::StructFieldGet(sfg) => format!("Struct field get: {}", sfg.id),
            CodeNode::ListIndex(list_index) => format!("List index: {}", list_index.id),
            CodeNode::AnonymousFunction(anon_func) => {
                format!("Anonymous function: {}", anon_func.id)
            }
            CodeNode::ReassignListIndex(reassign_list_index) => {
                format!("Reassign List Index: {:?}", reassign_list_index.id)
            }
            CodeNode::WhileLoop(while_loop) => format!("While loop: {:?}", while_loop.id),
            CodeNode::EnumVariantLiteral(evl) => format!("Enum variant literal: {:?}", evl.id),
            CodeNode::EarlyReturn(early_return) => format!("Early return: {:?}", early_return.id),
            CodeNode::Try(trai) => format!("Try: {:?}", trai.id),
        }
    }

    // these are just placeholder IDs for now, because for hello world, there's no
    // need to further disambiguate
    pub fn id(&self) -> ID {
        match self {
            CodeNode::FunctionCall(function_call) => function_call.id,
            CodeNode::StringLiteral(string_literal) => string_literal.id,
            CodeNode::NumberLiteral(number_literal) => number_literal.id,
            CodeNode::Assignment(assignment) => assignment.id,
            CodeNode::Reassignment(reassignment) => reassignment.id,
            CodeNode::Block(block) => block.id,
            CodeNode::VariableReference(variable_reference) => variable_reference.id,
            CodeNode::FunctionReference(function_reference) => function_reference.id,
            CodeNode::Argument(argument) => argument.id,
            CodeNode::Placeholder(placeholder) => placeholder.id,
            CodeNode::NullLiteral(id) => *id,
            CodeNode::StructLiteral(struct_literal) => struct_literal.id,
            CodeNode::StructLiteralField(field) => field.id,
            CodeNode::Conditional(conditional) => conditional.id,
            CodeNode::ListLiteral(list_literal) => list_literal.id,
            CodeNode::Match(mach) => mach.id,
            CodeNode::StructFieldGet(sfg) => sfg.id,
            CodeNode::ListIndex(list_index) => list_index.id,
            CodeNode::AnonymousFunction(anon_func) => anon_func.id,
            CodeNode::ReassignListIndex(rli) => rli.id,
            CodeNode::WhileLoop(while_loop) => while_loop.id,
            CodeNode::EnumVariantLiteral(evl) => evl.id,
            CodeNode::EarlyReturn(evl) => evl.id,
            CodeNode::Try(trai) => trai.id,
        }
    }

    pub fn previous_child(&self, node_id: ID) -> Option<&CodeNode> {
        let children = self.children();
        let position = children.iter().position(|n| n.id() == node_id);
        if let Some(position) = position {
            if position > 0 {
                if let Some(next) = children.get(position - 1) {
                    return Some(next);
                }
            }
        }
        None
    }

    pub fn next_child(&self, node_id: ID) -> Option<&CodeNode> {
        let children = self.children();
        let position = children.iter().position(|n| n.id() == node_id);
        if let Some(position) = position {
            if let Some(next) = children.get(position + 1) {
                return Some(next);
            }
        }
        None
    }

    pub fn children(&self) -> Vec<&CodeNode> {
        self.children_iter().collect()
    }

    pub fn children_iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a CodeNode> + 'a> {
        match self {
            CodeNode::FunctionCall(function_call) => Box::new(
                iter::once(function_call.function_reference.as_ref())
                    .chain(function_call.args.iter()),
            ),
            CodeNode::StringLiteral(_) => Box::new(iter::empty()),
            CodeNode::NumberLiteral(_) => Box::new(iter::empty()),
            CodeNode::Assignment(assignment) => {
                Box::new(iter::once(assignment.expression.borrow()))
            }
            CodeNode::Reassignment(reassignment) => {
                Box::new(iter::once(reassignment.expression.borrow()))
            }
            CodeNode::Block(block) => Box::new(block.expressions.iter()),
            CodeNode::VariableReference(_) => Box::new(iter::empty()),
            CodeNode::FunctionReference(_) => Box::new(iter::empty()),
            CodeNode::Argument(argument) => Box::new(iter::once(argument.expr.borrow())),
            CodeNode::Placeholder(_) => Box::new(iter::empty()),
            CodeNode::NullLiteral(_) => Box::new(iter::empty()),
            CodeNode::StructLiteral(struct_literal) => Box::new(struct_literal.fields.iter()),
            CodeNode::StructLiteralField(field) => Box::new(iter::once(field.expr.borrow())),
            CodeNode::Conditional(ref conditional) => {
                let i = iter::once(conditional.condition.as_ref())
                    .chain(iter::once(conditional.true_branch.as_ref()));
                match &conditional.else_branch {
                    None => Box::new(i),
                    Some(else_branch) => Box::new(i.chain(iter::once(else_branch.as_ref()))),
                }
            }
            CodeNode::ListLiteral(list_literal) => Box::new(list_literal.elements.iter()),
            CodeNode::Match(mach) => Box::new(
                iter::once(mach.match_expression.borrow())
                    .chain(mach.branch_by_variant_id.values()),
            ),
            CodeNode::StructFieldGet(sfg) => Box::new(iter::once(sfg.struct_expr.as_ref())),
            CodeNode::ListIndex(list_index) => Box::new(
                iter::once(list_index.list_expr.as_ref())
                    .chain(iter::once(list_index.index_expr.as_ref())),
            ),
            CodeNode::AnonymousFunction(anon_func) => {
                Box::new(std::iter::once(anon_func.block.as_ref()))
            }
            CodeNode::ReassignListIndex(rli) => {
                Box::new(
                    std::iter::once(rli.index_expr.as_ref()).chain(
                        std::iter::once(rli.set_to_expr.as_ref())
                    )
                )
            }
            CodeNode::WhileLoop(while_loop) => {
                Box::new(
                    std::iter::once(while_loop.condition.as_ref()).chain(
                        std::iter::once(while_loop.body.as_ref())
                    )
                )
            }
            CodeNode::EnumVariantLiteral(evl) => {
                Box::new(std::iter::once(evl.variant_value_expr.as_ref()))
            }
            CodeNode::EarlyReturn(early_return) => {
                Box::new(std::iter::once(early_return.code.as_ref()))
            }
            CodeNode::Try(trai) => {
                Box::new(std::iter::once(trai.maybe_error_expr.as_ref()).chain(std::iter::once(trai.or_else_return_expr.as_ref())))
            }
        }
    }

    pub fn self_with_all_children_dfs(&self) -> impl Iterator<Item = &CodeNode> {
        iter::once(self).chain(self.all_children_dfs_iter())
    }

    pub fn all_children_dfs_iter<'a>(&'a self) -> Box<dyn Iterator<Item = &'a CodeNode> + 'a> {
        Box::new(self.children_iter()
                     .flat_map(|child| iter::once(child).chain(child.all_children_dfs_iter())))
    }

    pub fn all_children_dfs(&self) -> Vec<&CodeNode> {
        self.all_children_dfs_iter().collect()
    }

    pub fn children_mut(&mut self) -> Vec<&mut CodeNode> {
        match self {
            CodeNode::FunctionCall(function_call) => {
                let mut children: Vec<&mut CodeNode> = function_call.args.iter_mut().collect();
                children.insert(0, &mut function_call.function_reference);
                children
            }
            CodeNode::StringLiteral(_) => Vec::new(),
            CodeNode::NumberLiteral(_) => Vec::new(),
            CodeNode::Assignment(assignment) => vec![assignment.expression.borrow_mut()],
            CodeNode::Reassignment(reassignment) => vec![reassignment.expression.borrow_mut()],
            CodeNode::Block(block) => block.expressions.iter_mut().collect(),
            CodeNode::VariableReference(_) => Vec::new(),
            CodeNode::FunctionReference(_) => Vec::new(),
            CodeNode::Argument(argument) => vec![argument.expr.borrow_mut()],
            CodeNode::Placeholder(_placeholder) => vec![],
            CodeNode::NullLiteral(_) => vec![],
            CodeNode::StructLiteral(struct_literal) => struct_literal.fields.iter_mut().collect(),
            CodeNode::StructLiteralField(field) => vec![field.expr.borrow_mut()],
            CodeNode::Conditional(ref mut conditional) => {
                let i = iter::once(conditional.condition.as_mut())
                    .chain(iter::once(conditional.true_branch.as_mut()));
                match &mut conditional.else_branch {
                    None => i.collect(),
                    Some(else_branch) => i.chain(iter::once(else_branch.as_mut())).collect(),
                }
            }
            CodeNode::ListLiteral(list_literal) => list_literal.elements.iter_mut().collect_vec(),
            CodeNode::Match(mach) => {
                iter::once(mach.match_expression.borrow_mut()).chain(mach.branch_by_variant_id
                                                                         .values_mut())
                                                              .collect()
            }
            CodeNode::StructFieldGet(sfg) => vec![sfg.struct_expr.borrow_mut()],
            CodeNode::ListIndex(list_index) => vec![list_index.list_expr.borrow_mut(),
                                                    list_index.index_expr.borrow_mut(),],
            CodeNode::AnonymousFunction(anon_func) => vec![anon_func.block.borrow_mut()],
            CodeNode::ReassignListIndex(rli) => {
                vec![rli.index_expr.borrow_mut(), rli.set_to_expr.borrow_mut()]
            }
            CodeNode::WhileLoop(while_loop) => vec![while_loop.condition.borrow_mut(),
                                                    while_loop.body.borrow_mut()],
            CodeNode::EnumVariantLiteral(evl) => vec![evl.variant_value_expr.borrow_mut()],
            CodeNode::EarlyReturn(early_return) => vec![early_return.code.borrow_mut()],
            CodeNode::Try(trai) => vec![trai.maybe_error_expr.borrow_mut(),
                                        trai.or_else_return_expr.borrow_mut()],
        }
    }

    pub fn replace(&mut self, code_node: CodeNode) {
        self.replace_with(code_node.id(), code_node);
    }

    // the return value is meaningless, it's just used to thread the code so we don't have to copy it
    pub fn replace_with(&mut self, id: ID, mut replace_with: CodeNode) -> Option<CodeNode> {
        if self.id() == id {
            *self = replace_with;
            return None;
        }
        for child in self.children_mut() {
            let ret = child.replace_with(id, replace_with);
            if let Some(cn) = ret {
                replace_with = cn;
            } else {
                return None;
            }
        }
        return Some(replace_with);
    }

    pub fn find_node(&self, id: ID) -> Option<&CodeNode> {
        if self.id() == id {
            Some(self)
        } else {
            for child in self.children_iter() {
                if let Some(found_node) = child.find_node(id) {
                    return Some(found_node);
                }
            }
            None
        }
    }

    pub fn find_parent(&self, id: ID) -> Option<&CodeNode> {
        if self.id() == id {
            return None;
        } else {
            for child in self.children_iter() {
                if child.id() == id {
                    return Some(self);
                } else {
                    let found_parent = child.find_parent(id);
                    if let Some(code_node) = found_parent {
                        return Some(code_node);
                    }
                }
            }
        }
        None
    }
}

pub type ID = Uuid;

#[cfg(not(target_arch = "wasm32"))]
pub fn new_id() -> ID {
    Uuid::new_v4()
}

// this is weird. without this, we get an error about RNG not being available in the browser. got
// this browser uuid implementation utilizing crypto from https://stackoverflow.com/a/2117523/149987
#[cfg(target_arch = "wasm32")]
pub fn new_id() -> ID {
    let uuid = js! {
        return ([1e7]+-1e3+-4e3+-8e3+-1e11).replace(new RegExp("[018]", "g"), c =>
            (c ^ crypto.getRandomValues(new Uint8Array(1))[0] & 15 >> c / 4).toString(16)
        );
    };
    Uuid::parse_str(&uuid.into_string().unwrap()).unwrap()
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct StringLiteral {
    pub value: String,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct FunctionCall {
    pub function_reference: Box<CodeNode>,
    pub args: Vec<CodeNode>,
    pub id: ID,
}

impl FunctionCall {
    pub fn function_reference(&self) -> &FunctionReference {
        match *self.function_reference {
            CodeNode::FunctionReference(ref function_reference) => function_reference,
            _ => panic!("{:?} is not a FunctionReference", self.function_reference),
        }
    }

    pub fn args(&self) -> Vec<&Argument> {
        self.iter_args().collect()
    }

    pub fn iter_args(&self) -> impl Iterator<Item = &Argument> + '_ {
        self.args.iter().map(|arg| arg.into_argument())
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Assignment {
    pub name: String,
    // TODO: consider differentiating between CodeNodes and Expressions.
    pub expression: Box<CodeNode>,
    pub id: ID,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Reassignment {
    pub id: ID,
    pub assignment_id: ID,
    // TODO: consider differentiating between CodeNodes and Expressions.
    pub expression: Box<CodeNode>,
}

impl Into<CodeNode> for Reassignment {
    fn into(self) -> CodeNode {
        CodeNode::Reassignment(self)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ReassignListIndex {
    pub id: ID,
    // this must refer to a list of something, that's the list we'll be mutating
    pub assignment_id: ID,
    pub index_expr: Box<CodeNode>,
    // this must evaluate to the T in List<T> (which assignment_id refers to)
    pub set_to_expr: Box<CodeNode>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Block {
    pub expressions: Vec<CodeNode>,
    pub id: ID,
}

impl Block {
    pub fn new() -> Self {
        Self { id: new_id(),
               expressions: vec![] }
    }

    pub fn find_position(&self, exp_id: ID) -> Option<usize> {
        self.expressions.iter().position(|code| code.id() == exp_id)
    }
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

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
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
        ArgumentDefinition { id,
                             short_name,
                             arg_type }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Argument {
    pub id: ID,
    pub argument_definition_id: ID,
    pub expr: Box<CodeNode>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Placeholder {
    pub id: ID,
    pub description: String,
    pub typ: Type,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct StructLiteral {
    pub id: ID,
    pub struct_id: ID,
    pub fields: Vec<CodeNode>,
}

impl StructLiteral {
    pub fn fields(&self) -> impl Iterator<Item = &StructLiteralField> {
        self.fields
            .iter()
            .map(|f| f.into_struct_literal_field().unwrap())
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct StructLiteralField {
    pub id: ID,
    pub struct_field_id: ID,
    pub expr: Box<CodeNode>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Conditional {
    pub id: ID,
    // this can be any expression
    pub condition: Box<CodeNode>,
    // this'll be a block
    pub true_branch: Box<CodeNode>,
    // this'll be a block
    pub else_branch: Option<Box<CodeNode>>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct WhileLoop {
    pub id: ID,
    // this can be any expression returning a boolean
    pub condition: Box<CodeNode>,
    // this'll be a block
    pub body: Box<CodeNode>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Match {
    pub id: ID,
    pub match_expression: Box<CodeNode>,
    // BTreeMap because we want a consistent ordering for these branches. so they
    // show up in the code in the same order, and so navigating them is stable
    pub branch_by_variant_id: BTreeMap<ID, CodeNode>,
}

impl Match {
    pub fn make_variable_id(match_id: ID, variant_id: ID) -> ID {
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID,
                           [match_id, variant_id].iter().join(":").as_bytes())
    }

    pub fn variable_id(&self, variant_id: ID) -> ID {
        Self::make_variable_id(self.id, variant_id)
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ListLiteral {
    pub id: ID,
    pub element_type: Type,
    pub elements: Vec<CodeNode>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct StructFieldGet {
    pub id: ID,
    pub struct_expr: Box<CodeNode>,
    pub struct_field_id: ID,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct NumberLiteral {
    pub id: ID,
    // TODO: this should be i128 to match Value, but there's some weird serde error in serializing
    // this, when we serialize into the DB. i have no idea what the problem is, but making this i64
    // for now just fixes the problem. we need to actually figure out the number system in this
    // language later anyway
    pub value: i64,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct ListIndex {
    pub id: ID,
    pub list_expr: Box<CodeNode>,
    pub index_expr: Box<CodeNode>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct EnumVariantLiteral {
    pub id: ID,
    pub typ: Type,
    pub variant_id: ID,
    pub variant_value_expr: Box<CodeNode>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct EarlyReturn {
    pub id: ID,
    pub code: Box<CodeNode>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq)]
pub struct Try {
    pub id: ID,
    pub maybe_error_expr: Box<CodeNode>,
    pub or_else_return_expr: Box<CodeNode>,
}
