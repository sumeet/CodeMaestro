// TODO: maybe i'll need this file later
use super::lang;
use super::env;

pub enum CodeErrorType<'a> {
    MissingArgument(&'a lang::Type)
}

pub struct CodeValidationError<'a> {
    pub code_id: lang::ID,
    pub error_type: CodeErrorType<'a>,
}

pub struct Validator<'a> {
    execution_env: &'a env::ExecutionEnvironment,
}

impl<'a> Validator<'a> {
    pub fn new(env: &env::ExecutionEnvironment) -> Validator {
        Validator { execution_env: env }
    }

    // TODO: add validation that no two code nodes have the same uuid
    pub fn validate(&self, code_node: &lang::CodeNode) -> Vec<CodeValidationError> {
        match code_node {
            lang::CodeNode::FunctionCall(function_call) => {
                vec![]
            }
            lang::CodeNode::StringLiteral(string_literal) => {
                vec![]
            }
            lang::CodeNode::Assignment(assignment) => {
                vec![]
            }
            lang::CodeNode::Block(block) => {
                vec![]
            }
            lang::CodeNode::VariableReference(variable_reference) => {
                vec![]
            }
            lang::CodeNode::FunctionReference(ref function_reference) => {
                self.validate_function_reference(function_reference)
            }
            lang::CodeNode::FunctionDefinition(function_definition) => {
                vec![]
            }
        }
    }

    fn validate_function_reference(&self, function_reference: &lang::FunctionReference) -> Vec<CodeValidationError> {
        vec![]
    }
}
