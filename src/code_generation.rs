use super::enums::Enum;
use super::lang;
use super::structs;
use std::collections::BTreeMap;

pub fn new_struct_literal_with_placeholders(strukt: &structs::Struct) -> lang::CodeNode {
    let fields = strukt.fields
                       .iter()
                       .map(|field| {
                           lang::CodeNode::StructLiteralField(lang::StructLiteralField {
                id: lang::new_id(),
                struct_field_id: field.id,
                expr: Box::new(new_placeholder(
                    field.name.to_string(),
                    field.field_type.clone(),
                )),
            })
                       })
                       .collect();
    lang::CodeNode::StructLiteral(lang::StructLiteral { id: lang::new_id(),
                                                        struct_id: strukt.id,
                                                        fields })
}

pub fn new_function_call_with_placeholder_args(func: &lang::Function) -> lang::CodeNode {
    let args = func.takes_args()
                   .into_iter()
                   .map(|arg_def| {
                       lang::CodeNode::Argument(lang::Argument {
                id: lang::new_id(),
                argument_definition_id: arg_def.id,
                expr: Box::new(new_placeholder(arg_def.short_name, arg_def.arg_type)),
            })
                   })
                   .collect();
    lang::CodeNode::FunctionCall(lang::FunctionCall {
        id: lang::new_id(),
        function_reference: Box::new(lang::CodeNode::FunctionReference(lang::FunctionReference {
            id: lang::new_id(),
            function_id: func.id(),
        })),
        args: args,
    })
}

pub fn new_function_call_with_wrapped_arg(func: &lang::Function,
                                          arg_def_id: lang::ID,
                                          wrapped_node: lang::CodeNode)
                                          -> lang::CodeNode {
    let args = func.takes_args()
                   .into_iter()
                   .map(move |arg_def| {
                       let expr = if arg_def_id == arg_def.id {
                           wrapped_node.clone()
                       } else {
                           new_placeholder(arg_def.short_name, arg_def.arg_type)
                       };

                       lang::CodeNode::Argument(lang::Argument { id: lang::new_id(),
                                                                 argument_definition_id:
                                                                     arg_def.id,
                                                                 expr: Box::new(expr) })
                   })
                   .collect();
    lang::CodeNode::FunctionCall(lang::FunctionCall {
        id: lang::new_id(),
        function_reference: Box::new(lang::CodeNode::FunctionReference(lang::FunctionReference {
            id: lang::new_id(),
            function_id: func.id(),
        })),
        args,
    })
}

pub fn new_variable_reference(assignment_id: lang::ID) -> lang::CodeNode {
    lang::CodeNode::VariableReference(lang::VariableReference { assignment_id,
                                                                id: lang::new_id() })
}

pub fn new_string_literal(value: String) -> lang::CodeNode {
    lang::CodeNode::StringLiteral(lang::StringLiteral { value,
                                                        id: lang::new_id() })
}

pub fn new_number_literal(value: i128) -> lang::CodeNode {
    lang::CodeNode::NumberLiteral(lang::NumberLiteral { value: value as i64,
                                                        id: lang::new_id() })
}

pub fn new_list_literal(typ: lang::Type) -> lang::CodeNode {
    lang::CodeNode::ListLiteral(lang::ListLiteral { id: lang::new_id(),
                                                    element_type: typ,
                                                    elements: vec![] })
}

pub fn new_placeholder(description: String, typ: lang::Type) -> lang::CodeNode {
    lang::CodeNode::Placeholder(lang::Placeholder { id: lang::new_id(),
                                                    description,
                                                    typ })
}

pub fn new_conditional(for_type: &Option<lang::Type>) -> lang::CodeNode {
    let branch_type = for_type.clone()
                              .unwrap_or_else(|| lang::Type::from_spec(&*lang::NULL_TYPESPEC));
    lang::CodeNode::Conditional(lang::Conditional {
        id: lang::new_id(),
        // TODO: change to boolean type once we add it
        condition: Box::new(new_placeholder(
            "Condition".to_string(),
            lang::Type::from_spec(&*lang::BOOLEAN_TYPESPEC),
        )),
        true_branch: Box::new(new_placeholder("True branch".to_string(), branch_type)),
        else_branch: None,
    })
}

pub fn new_match(eneom: &Enum,
                 enum_type: &lang::Type,
                 match_expr: lang::CodeNode)
                 -> lang::CodeNode {
    let branch_by_variant_id: BTreeMap<_, _> = eneom.variant_types(&enum_type.params)
                                                    .into_iter()
                                                    .map(|(variant, typ)| {
                                                        let mut block = lang::Block::new();
                                                        block
                .expressions
                .push(new_placeholder(variant.name.clone(), typ.clone()));
                                                        (variant.id, lang::CodeNode::Block(block))
                                                    })
                                                    .collect();

    lang::CodeNode::Match(lang::Match { id: lang::new_id(),
                                        match_expression: Box::new(match_expr),
                                        branch_by_variant_id })
}

pub fn new_null_literal() -> lang::CodeNode {
    lang::CodeNode::NullLiteral(lang::new_id())
}

pub fn new_assignment(name: String, expression: lang::CodeNode) -> lang::CodeNode {
    lang::CodeNode::Assignment(lang::Assignment { id: lang::new_id(),
                                                  name,
                                                  expression: Box::new(expression) })
}

pub fn new_struct_field_get(struct_expr: lang::CodeNode,
                            struct_field_id: lang::ID)
                            -> lang::CodeNode {
    lang::CodeNode::StructFieldGet(lang::StructFieldGet { id: lang::new_id(),
                                                          struct_expr: Box::new(struct_expr),
                                                          struct_field_id })
}

pub fn new_list_index(list_expr: lang::CodeNode) -> lang::CodeNode {
    lang::CodeNode::ListIndex(lang::ListIndex {
        id: lang::new_id(),
        list_expr: Box::new(list_expr),
        index_expr: Box::new(new_placeholder(
            "Index".to_string(),
            lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
        )),
    })
}
