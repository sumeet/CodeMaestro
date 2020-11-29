use crate::code_editor::{CodeGenie, InsertionPoint};
use crate::insert_code_menu;
use cs::{lang, EnvGenie};

use cs::lang::arg_typ_for_anon_func;
use serde_derive::{Deserialize, Serialize};

// just need this for debugging, tho maybe i'll keep it around, it's probably good to have
#[derive(Serialize, Deserialize, Debug)]
enum VariableAntecedent {
    Assignment {
        assignment_id: lang::ID,
    },
    AnonFuncArgument {
        anonymous_function_id: lang::ID,
    },
    Argument,
    MatchVariant {
        match_statement_id: lang::ID,
        variant_id: lang::ID,
    },
}

pub fn resolve_generics(variable: &Variable,
                        code_genie: &CodeGenie,
                        env_genie: &EnvGenie)
                        -> lang::Type {
    match variable.variable_type {
        // TODO: this is highly duped with stuff in try_to_resolve_generic
        //
        // i'm getting the feeling that this entire file shouldn't deal with types and instead the
        // callers to this should make their own call to look up the types if they want to
        VariableAntecedent::Assignment { assignment_id } => {
            let assignment = code_genie.find_node(assignment_id)
                                       .unwrap()
                                       .as_assignment()
                                       .unwrap();
            let guessed = code_genie.try_to_resolve_all_generics(&assignment.expression,
                                                                 variable.typ.clone(),
                                                                 env_genie);
            println!("guessed type for {:?}-----\n\n{:?}\n----------",
                     assignment.expression, guessed);
            guessed
        }
        VariableAntecedent::AnonFuncArgument { anonymous_function_id, } => {
            let anon_func = code_genie.find_node(anonymous_function_id).unwrap();
            let full_unresolved_typ = code_genie.guess_type_without_resolving_generics(anon_func,
                                                                                       env_genie)
                                                .unwrap();
            let resolved_anon_func_typ =
                code_genie.try_to_resolve_all_generics(anon_func, full_unresolved_typ, env_genie);
            arg_typ_for_anon_func(resolved_anon_func_typ)
        }
        VariableAntecedent::Argument => {
            // unhandled so far
            variable.typ.clone()
        }

        VariableAntecedent::MatchVariant { .. } => {
            // also unhandled
            variable.typ.clone()
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Variable {
    variable_type: VariableAntecedent,
    pub locals_id: lang::ID,
    pub(crate) typ: lang::Type,
    pub(crate) name: String,
}

#[derive(Copy, Clone)]
pub struct SearchPosition {
    pub before_code_id: lang::ID,
    pub is_search_inclusive: bool,
}

impl SearchPosition {
    pub fn not_inclusive(before_id: lang::ID) -> Self {
        Self { before_code_id: before_id,
               is_search_inclusive: false }
    }
}

impl From<InsertionPoint> for SearchPosition {
    fn from(ip: InsertionPoint) -> Self {
        let (insertion_id, is_search_inclusive) = insert_code_menu::assignment_search_position(ip);
        SearchPosition { before_code_id: insertion_id,
                         is_search_inclusive }
    }
}

// TODO: this should probably go near the code genie
pub fn find_all_locals_preceding_with_resolving_generics<'a>(
    search_position: SearchPosition,
    locals_search_params: LocalsSearchParams,
    code_genie: &'a CodeGenie,
    env_genie: &'a EnvGenie)
    -> impl Iterator<Item = Variable> + 'a {
    find_all_locals_preceding_without_resolving_generics(search_position,
                                                         locals_search_params,
                                                         code_genie,
                                                         env_genie).map(move |mut var| {
        var.typ = resolve_generics(&var, code_genie, env_genie);
        var
    })
}

pub fn find_all_locals_preceding_without_resolving_generics<'a>(
    search_position: SearchPosition,
    locals_search_params: LocalsSearchParams,
    code_genie: &'a CodeGenie,
    env_genie: &'a EnvGenie)
    -> impl Iterator<Item = Variable> + 'a {
    find_assignments_preceding(search_position, locals_search_params, code_genie, env_genie)
        .chain(find_function_args_preceding(search_position, locals_search_params, code_genie, env_genie))
        .chain(find_enum_variants_preceding(search_position, locals_search_params, code_genie, env_genie))
        .chain(find_anon_func_args_for(search_position, locals_search_params, code_genie, env_genie))
}

#[derive(Clone, Copy)]
pub enum LocalsSearchParams {
    NoFilter,
    LocalsID(lang::ID),
}

impl LocalsSearchParams {
    pub fn matches(&self, locals_id: lang::ID) -> bool {
        match self {
            Self::NoFilter => true,
            Self::LocalsID(id) => id == &locals_id,
        }
    }
}

pub fn find_assignments_preceding<'a>(search_position: SearchPosition,
                                      locals_search_params: LocalsSearchParams,
                                      code_genie: &'a CodeGenie,
                                      env_genie: &'a EnvGenie)
                                      -> impl Iterator<Item = Variable> + 'a {
    code_genie.find_assignments_that_come_before_code(search_position.before_code_id,
                                                      search_position.is_search_inclusive)
              .into_iter()
              .filter_map(move |assignment| {
                  if !locals_search_params.matches(assignment.id) {
                      return None
                  }
                  let assignment_clone: lang::Assignment = (*assignment).clone();
                  let guessed_type =
                      code_genie.guess_type_without_resolving_generics(&lang::CodeNode::Assignment(assignment_clone),
                                            env_genie);
                  Some(Variable { locals_id: assignment.id,
                             variable_type: VariableAntecedent::Assignment { assignment_id: assignment.id },
                             typ: guessed_type.unwrap(),
                             name: assignment.name.clone() })
              })
}

pub fn find_function_args_preceding<'a>(_search_position: SearchPosition,
                                        locals_search_params: LocalsSearchParams,
                                        code_genie: &'a CodeGenie,
                                        env_genie: &'a EnvGenie)
                                        -> impl Iterator<Item = Variable> + 'a {
    env_genie.code_takes_args(code_genie.root().id())
             .filter_map(move |arg| {
                 if !locals_search_params.matches(arg.id) {
                     return None;
                 }
                 Some(Variable { locals_id: arg.id,
                                 variable_type: VariableAntecedent::Argument,
                                 typ: arg.arg_type,
                                 name: arg.short_name })
             })
}

fn find_enum_variants_preceding<'a>(search_position: SearchPosition,
                                    locals_search_params: LocalsSearchParams,
                                    code_genie: &'a CodeGenie,
                                    env_genie: &'a EnvGenie)
                                    -> impl Iterator<Item = Variable> + 'a {
    code_genie.find_enum_variants_preceding_iter(search_position.before_code_id, env_genie)
              .filter_map(move |match_variant| {
                  let assignment_id = match_variant.assignment_id();
                  if !locals_search_params.matches(assignment_id) {
                      return None;
                  }
                  Some(Variable { locals_id: assignment_id,
                             variable_type:
                                 VariableAntecedent::MatchVariant { match_statement_id:
                                                                        match_variant.match_id,
                                                                    variant_id:
                                                                        match_variant.enum_variant
                                                                                     .id },
                             typ: match_variant.typ,
                             name: match_variant.enum_variant.name })
              })
}

fn find_anon_func_args_for<'a>(search_position: SearchPosition,
                               locals_search_params: LocalsSearchParams,
                               code_genie: &'a CodeGenie,
                               env_genie: &'a EnvGenie)
                               -> impl Iterator<Item = Variable> + 'a {
    code_genie.find_anon_func_parents(search_position.before_code_id)
              .filter_map(move |anon_func| {
                  let arg = &anon_func.as_anon_func().unwrap().takes_arg;
                  if !locals_search_params.matches(arg.id) {
                      return None;
                  }
                  let anon_func_typ = code_genie.guess_type_without_resolving_generics(anon_func,
                                                                                       env_genie)
                                                .unwrap();
                  // println!("arg name: {:?}", arg.short_name);
                  // println!("guessed typ for anon_func: {:?}", anon_func_typ);
                  // println!("variable typ: {:?}", anon_func_typ.params[0]);
                  Some(Variable { locals_id: arg.id,
                      variable_type:
                                 VariableAntecedent::AnonFuncArgument { anonymous_function_id:
                                                                            anon_func.id() },
                             typ: arg_typ_for_anon_func(anon_func_typ),
                             name: arg.short_name.clone() })
              })
}
