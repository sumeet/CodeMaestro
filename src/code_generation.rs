use super::enums::Enum;
use super::lang;
use super::structs;
use std::collections::BTreeMap;

pub fn new_struct_literal_with_placeholders(strukt: &structs::Struct) -> lang::CodeNode {
    let fields = strukt.fields.iter()
        .map(|field| {
            lang::CodeNode::StructLiteralField(lang::StructLiteralField {
                id: lang::new_id(),
                struct_field_id: field.id,
                expr: Box::new(new_placeholder(
                    &field.name,
                    field.field_type.clone(),
                )),
            })
        })
        .collect();
    lang::CodeNode::StructLiteral(lang::StructLiteral {
        id: lang::new_id(),
        struct_id: strukt.id,
        fields,
    })
}

pub fn new_function_call_with_placeholder_args(func: &lang::Function) -> lang::CodeNode {
    let args = func.takes_args().iter()
        .map(|arg_def| {
            lang::CodeNode::Argument(lang::Argument {
                id: lang::new_id(),
                argument_definition_id: arg_def.id,
                expr: Box::new(
                    new_placeholder(&arg_def.short_name, arg_def.arg_type.clone())
                ),
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

pub fn new_variable_reference(assignment_id: lang::ID) -> lang::CodeNode {
    lang::CodeNode::VariableReference(lang::VariableReference {
        assignment_id,
        id: lang::new_id(),
    })
}

pub fn new_string_literal(string: &str) -> lang::CodeNode {
    lang::CodeNode::StringLiteral(lang::StringLiteral {
        value: string.to_string(),
        id: lang::new_id(),
    })
}

pub fn new_list_literal(typ: lang::Type) -> lang::CodeNode {
    lang::CodeNode::ListLiteral(lang::ListLiteral {
        id: lang::new_id(),
        element_type: typ,
        elements: vec![]
    })
}

pub fn new_placeholder(description: &str, typ: lang::Type) -> lang::CodeNode {
    lang::CodeNode::Placeholder(lang::Placeholder {
        id: lang::new_id(),
        description: description.to_string(),
        typ,
    })
}

pub fn new_conditional(for_type: &Option<lang::Type>) -> lang::CodeNode {
    let branch_type = for_type.clone().unwrap_or_else(
        || lang::Type::from_spec(&*lang::NULL_TYPESPEC));
    lang::CodeNode::Conditional(lang::Conditional {
        id: lang::new_id(),
        // TODO: change to boolean type once we add it
        condition: Box::new(new_placeholder(
            "Condition",
            lang::Type::from_spec(&*lang::BOOLEAN_TYPESPEC))),
        true_branch: Box::new(new_placeholder(
            "True branch",
            branch_type,
            )),
        else_branch: None,
    })
}

pub fn new_match(eneom: &Enum, enum_type: &lang::Type, match_expr: lang::CodeNode,
                 for_type: &Option<lang::Type>) -> lang::CodeNode {
    let branch_type = for_type.clone().unwrap_or_else(
        || lang::Type::from_spec(&*lang::NULL_TYPESPEC));

    let branch_by_variant_id : BTreeMap<_, _> = eneom.variant_types(&enum_type.params).into_iter()
        .map(|(variant, typ)| {
            (variant.id, new_placeholder(&variant.name, typ.clone()))
        }).collect();

    lang::CodeNode::Match(lang::Match {
        id: lang::new_id(),
        match_expression: Box::new(match_expr),
        branch_by_variant_id
    })
}

pub fn new_null_literal() -> lang::CodeNode {
    lang::CodeNode::NullLiteral
}

