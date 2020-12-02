use crate::enums::Enum;
use crate::lang;
use crate::lang::AnonymousFunction;
use crate::structs;
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

pub fn new_function_call(func_id: lang::ID, args: Vec<lang::CodeNode>) -> lang::CodeNode {
    lang::CodeNode::FunctionCall(lang::FunctionCall {
        id: lang::new_id(),
        function_reference: Box::new(lang::CodeNode::FunctionReference(lang::FunctionReference {
            id: lang::new_id(),
            function_id: func_id,
        })),
        args,
    })
}

pub fn new_function_call_with_arg_exprs(func: &dyn lang::Function,
                                        arg_exprs: impl Iterator<Item = lang::CodeNode>)
                                        -> lang::CodeNode {
    let args = func.takes_args()
                   .into_iter()
                   .zip(arg_exprs)
                   .map(|(arg_def, arg_expr)| {
                       lang::CodeNode::Argument(lang::Argument { id: lang::new_id(),
                                                                 argument_definition_id:
                                                                     arg_def.id,
                                                                 expr: Box::new(arg_expr) })
                   })
                   .collect();
    new_function_call(func.id(), args)
}

pub fn new_function_call_with_placeholder_args(func: &dyn lang::Function) -> lang::CodeNode {
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
    new_function_call(func.id(), args)
}

pub fn new_function_call_with_wrapped_arg(func: &dyn lang::Function,
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

pub fn new_early_return() -> lang::CodeNode {
    lang::CodeNode::EarlyReturn(lang::EarlyReturn { id: lang::new_id(),
                                                    code: Box::new(new_placeholder("Early return".into(), lang::Type::from_spec(&*lang::ANY_TYPESPEC))) })
}

pub fn new_conditional(for_type: &Option<lang::Type>) -> lang::CodeNode {
    let (true_branch, else_branch) = match for_type {
        Some(for_type) => {
            (new_block(vec![new_placeholder("True branch".to_string(), for_type.clone())]),
             new_block(vec![new_placeholder("Else branch".to_string(), for_type.clone())]))
        }
        None => (new_block(vec![]), new_block(vec![])),
    };
    lang::CodeNode::Conditional(lang::Conditional {
        id: lang::new_id(),
        condition: Box::new(new_placeholder(
            "Condition".to_string(),
            lang::Type::from_spec(&*lang::BOOLEAN_TYPESPEC),
        )),
        true_branch: Box::new(lang::CodeNode::Block(true_branch)),
        else_branch: Some(Box::new(lang::CodeNode::Block(else_branch))),
    })
}

pub fn new_while_loop() -> lang::CodeNode {
    let null_type = lang::Type::from_spec(&*lang::NULL_TYPESPEC);
    let block = new_block(vec![new_placeholder("While loop body".to_string(), null_type)]);

    lang::CodeNode::WhileLoop(lang::WhileLoop {
        id: lang::new_id(),
        condition: Box::new(new_placeholder(
            "Condition".to_string(),
            lang::Type::from_spec(&*lang::BOOLEAN_TYPESPEC),
        )),
        body: Box::new(lang::CodeNode::Block(block)),
    })
}

pub fn new_block(inside_block: Vec<lang::CodeNode>) -> lang::Block {
    let mut block = lang::Block::new();
    block.expressions = inside_block;
    block
}

pub fn new_for_loop() -> lang::ForLoop {
    lang::ForLoop { id: lang::new_id(),
                    // really needs to be fixed
                    variable_name: "for_var".to_string(),
                    list_expression: Box::new(new_placeholder("List".into(), 
                    lang::Type::from_spec_id(lang::LIST_TYPESPEC.id,
                    vec![lang::Type::from_spec(&*lang::ANY_TYPESPEC)])
                    )),
                    body: Box::new(lang::CodeNode::Block(new_block(vec![]))) }
}

pub fn new_anon_func(takes_arg: lang::ArgumentDefinition, returns: lang::Type) -> lang::CodeNode {
    lang::CodeNode::AnonymousFunction(AnonymousFunction::new(takes_arg, returns))
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

pub fn new_assignment(name: String, expression: lang::CodeNode) -> lang::Assignment {
    lang::Assignment { id: lang::new_id(),
                       name,
                       expression: Box::new(expression) }
}

pub fn new_reassignment(assignment_id: lang::ID, expression: lang::CodeNode) -> lang::Reassignment {
    lang::Reassignment { id: lang::new_id(),
                         assignment_id,
                         expression: Box::new(expression) }
}

pub fn new_reassign_list_index(assignment_id: lang::ID,
                               typ: lang::Type)
                               -> lang::ReassignListIndex {
    lang::ReassignListIndex { id: lang::new_id(),
                              assignment_id,
                              index_expr: Box::new(new_placeholder("Index".to_string(), lang::Type::from_spec(&*lang::NUMBER_TYPESPEC))),
                              set_to_expr: Box::new(new_placeholder("Change to".to_string(),
                                                                    typ)) }
}

pub fn new_assignment_code_node(name: String, expression: lang::CodeNode) -> lang::CodeNode {
    lang::CodeNode::Assignment(new_assignment(name, expression))
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

pub fn new_enum_variant_literal(enum_name: String,
                                typ: lang::Type,
                                variant_id: lang::ID,
                                variant_type: lang::Type)
                                -> lang::EnumVariantLiteral {
    lang::EnumVariantLiteral { id: lang::new_id(),
                               typ,
                               variant_id,
                               variant_value_expr: Box::new(new_placeholder(enum_name,
                                                                            variant_type)) }
}

pub fn new_try(maybe_error_expr: lang::CodeNode, placeholder_typ: lang::Type) -> lang::Try {
    lang::Try { id: lang::new_id(),
                maybe_error_expr: Box::new(maybe_error_expr),
                or_else_expr: Box::new(new_placeholder("Or else".into(), placeholder_typ)) }
}
