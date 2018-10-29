use super::lang;

pub fn new_function_call_with_placeholder_args(func: &lang::Function) -> lang::CodeNode {
    let args = func.takes_args().iter()
        .map(|arg_def| {
            lang::CodeNode::Argument(lang::Argument {
                id: lang::new_id(),
                argument_definition_id: arg_def.id,
                expr: Box::new(lang::CodeNode::Placeholder(lang::Placeholder {
                    id: lang::new_id(),
                    description: arg_def.short_name.clone(),
                }))
            })
        })
        .collect();
    lang::CodeNode::FunctionCall(lang::FunctionCall {
        id: lang::new_id(),
        function_reference: Box::new(
            lang::CodeNode::FunctionReference(lang::FunctionReference {
                id: lang::new_id(),
                function_id: func.id()})),
        args: args,
    })
}

pub fn new_variable_reference(assignment: &lang::Assignment) -> lang::CodeNode {
    lang::CodeNode::VariableReference(lang::VariableReference {
        assignment_id: assignment.id,
        id: lang::new_id(),
    })
}
