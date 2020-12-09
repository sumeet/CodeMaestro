use cs::lang;
use cs::lang::CodeNode;
use std::iter::once;

// code can't be changed without going around mutation method
struct InferenceEnv {}

fn walk(root: &lang::CodeNode) {
    root.self_with_all_children_dfs()
        .filter_map(|code_node| resolve_via_code_inspection_only(&code_node));
}

fn resolve_via_code_inspection_only(code_node: &lang::CodeNode) -> Option<Resolution> {
    match code_node {
        // don't actually have the information for this from just only code
        CodeNode::FunctionCall(_) => None,
        CodeNode::FunctionReference(_) => None,
        CodeNode::Argument(arg) => resolve_via_code_inspection_only(&arg.expr),
        CodeNode::StringLiteral(_) => Some(Resolution::typ(lang::STRING_TYPESPEC.id)),
        CodeNode::NullLiteral(_) => Some(Resolution::typ(lang::NULL_TYPESPEC.id)),
        CodeNode::Assignment(assignment) => {
            Some(Resolution::link_to_code_id(assignment.expression.id()))
        }
        CodeNode::ForLoop(_) => Some(Resolution::typ(lang::NULL_TYPESPEC.id)),
        CodeNode::Reassignment(reassignment) => {
            Some(Resolution::link_to_code_id(reassignment.expression.id()))
        }
        CodeNode::Block(block) => {
            Some(match block.expressions.last() {
                     Some(last_exp) => Resolution::link_to_code_id(last_exp.id()),
                     None => Resolution::typ(lang::NULL_TYPESPEC.id),
                 })
        }
        CodeNode::AnonymousFunction(anon_func) => Some(ActualTypeSpec {}),
        CodeNode::VariableReference(_) => {}
        CodeNode::Placeholder(_) => {}
        CodeNode::StructLiteral(_) => {}
        CodeNode::StructLiteralField(_) => {}
        CodeNode::Conditional(_) => {}
        CodeNode::WhileLoop(_) => {}
        CodeNode::Match(_) => {}
        CodeNode::ListLiteral(_) => {}
        CodeNode::MapLiteral(_) => {}
        CodeNode::StructFieldGet(_) => {}
        CodeNode::NumberLiteral(_) => {}
        CodeNode::ListIndex(_) => {}
        CodeNode::ReassignListIndex(_) => {}
        CodeNode::EnumVariantLiteral(_) => {}
        CodeNode::EarlyReturn(_) => {}
        CodeNode::Try(_) => {}
    }
}

enum Resolution {
    ActualTypeSpec(ActualTypeSpec),
    FreeVariable(FreeVariableSource),
}

impl Resolution {
    pub fn typ(typespec_id: lang::ID) -> Self {
        Self::ActualTypeSpec(ActualTypeSpec { typespec_id,
                                              params: vec![] })
    }

    pub fn link_to_code_id(id: lang::ID) -> Self {
        Self::FreeVariable(FreeVariableSource::CodeNode(id))
    }
}

struct ActualTypeSpec {
    typespec_id: lang::ID,
    params: Vec<Resolution>,
}

#[derive(Clone, Copy)]
enum FreeVariableSource {
    Assignment {
        assignment_id: lang::ID,
    },
    CodeNode(lang::ID),
    Generic {
        function_call_id: lang::ID,
        generic_typespec_id: lang::ID,
    },
}
