use crate::code_editor::{get_type_from_list, CodeGenie, InsertionPoint};
use crate::insert_code_menu;
use cs::{lang, EnvGenie};

use cs::lang::arg_typ_for_anon_func;
use serde_derive::{Deserialize, Serialize};

// just need this for debugging, tho maybe i'll keep it around, it's probably good to have
#[derive(Serialize, Deserialize, Debug)]
pub enum VariableAntecedent {
    Assignment {
        assignment_id: lang::ID,
    },
    ForLoop {
        for_loop_id: lang::ID,
    },
    AnonFuncArgument {
        anonymous_function_id: lang::ID,
        argument_id: lang::ID,
    },
    FunctionArgument {
        argument_definition_id: lang::ID,
    },
    MatchVariant {
        match_statement_id: lang::ID,
        variant_id: lang::ID,
    },
}

pub enum VariableAntecedentPlace {
    Assignment {
        assignment_id: lang::ID,
    },
    ForLoop {
        for_loop_id: lang::ID,
    },
    AnonFuncArgument {
        anonymous_function_id: lang::ID,
        argument_id: lang::ID,
    },
    FunctionArgument {
        argument_definition_id: lang::ID,
    },
    MatchVariant {
        match_statement_id: lang::ID,
        variant_id: lang::ID,
    },
}

impl VariableAntecedent {
    pub fn assignment_id(&self) -> lang::ID {
        match self {
            VariableAntecedent::Assignment { assignment_id } => *assignment_id,
            VariableAntecedent::ForLoop { for_loop_id } => *for_loop_id,
            VariableAntecedent::AnonFuncArgument { argument_id, .. } => *argument_id,
            VariableAntecedent::FunctionArgument { argument_definition_id, } => {
                *argument_definition_id
            }
            VariableAntecedent::MatchVariant { match_statement_id,
                                               variant_id, } => {
                lang::Match::make_variable_id(*match_statement_id, *variant_id)
            }
        }
    }
}

pub fn find_all_referencable_variables<'a>(search_position: SearchPosition,
                                           code_genie: &'a CodeGenie,
                                           env_genie: &'a EnvGenie)
                                           -> impl Iterator<Item = VariableAntecedent> + 'a {
    let assignments =
        code_genie.find_assignments_that_come_before_code(search_position.before_code_id,
                                                          search_position.is_search_inclusive)
                  .map(|assignment| VariableAntecedent::Assignment { assignment_id:
                                                                         assignment.id });
    let func_args = env_genie.code_takes_args(code_genie.root().id())
                             .map(|argument_definition| {
                                 VariableAntecedent::FunctionArgument { argument_definition_id:
                                                                            argument_definition.id }
                             });
    let anon_func_args =
        code_genie.find_anon_func_parents(search_position.before_code_id)
                  .map(|anon_func| {
                      let anon_func = anon_func.as_anon_func().unwrap();
                      // TODO: anon funcs will later take more than one arg... of course
                      let anon_func_arg = &anon_func.takes_arg;
                      VariableAntecedent::AnonFuncArgument { anonymous_function_id: anon_func.id,
                                                             argument_id: anon_func_arg.id }
                  });

    let match_statement_variants =
        code_genie.find_enum_variants_preceding_iter(search_position.before_code_id)
                  .map(|(match_id, variant_id)| {
                      VariableAntecedent::MatchVariant { match_statement_id: match_id,
                                                         variant_id }
                  });

    let for_loop_variables =
        code_genie.find_for_loops_scopes_preceding(search_position.before_code_id)
                  .map(|for_loop| VariableAntecedent::ForLoop { for_loop_id: for_loop.id() });

    assignments.chain(func_args)
               .chain(anon_func_args)
               .chain(match_statement_variants)
               .chain(for_loop_variables)
}

pub fn find_antecedent_for_variable_reference(vr: &lang::VariableReference,
                                              is_inclusive: bool,
                                              code_genie: &CodeGenie,
                                              env_genie: &EnvGenie)
                                              -> Option<VariableAntecedent> {
    find_all_referencable_variables(SearchPosition { before_code_id: vr.assignment_id,
                                                     is_search_inclusive: is_inclusive },
                                    code_genie,
                                    env_genie).find(|antecedent| {
                                                  antecedent.assignment_id() == vr.assignment_id
                                              })
}

// pub fn get_type_from_variable(antecedent: VariableAntecedent,
//                               code_genie: &CodeGenie,
//                               env_genie: &EnvGenie)
//                               -> &lang::Type {
//     match antecedent {
//         // TODO: this is highly duped with stuff in try_to_resolve_generic
//         //
//         // i'm getting the feeling that this entire file shouldn't deal with types and instead the
//         // callers to this should make their own call to look up the types if they want to
//         VariableAntecedent::Assignment { assignment_id } => {
//             let assignment = code_genie.find_node(assignment_id)
//                                        .unwrap()
//                                        .as_assignment()
//                                        .unwrap();
//             let guessed = code_genie.try_to_resolve_all_generics(&assignment.expression,
//                                                                  variable.typ.clone(),
//                                                                  env_genie);
//             // println!("guessed type for {:?}-----\n\n{:?}\n----------",
//             //          assignment.expression, guessed);
//             guessed
//         }
//         // TODO: this is highly duped with stuff in try_to_resolve_generic
//         //
//         // i'm getting the feeling that this entire file shouldn't deal with types and instead the
//         // callers to this should make their own call to look up the types if they want to
//         VariableAntecedent::ForLoop { for_loop_id } => {
//             let for_loop = code_genie.find_node(for_loop_id)
//                                      .unwrap()
//                                      .as_for_loop()
//                                      .unwrap();
//             // println!("old type: {:?}", variable.typ);
//             let guessed =
//                 code_genie.try_to_resolve_all_generics(&for_loop.list_expression,
//                                                        lang::Type::list_of(variable.typ.clone()),
//                                                        env_genie);
//             // println!("list_expression: {:?}", for_loop.list_expression);
//             // println!("guessed: {:?}", guessed);
//             get_type_from_list(guessed).unwrap()
//             // println!("guessed type for {:?}-----\n\n{:?}\n----------",
//             //          assignment.expression, guessed);
//             // guessed
//         }
//         VariableAntecedent::AnonFuncArgument { anonymous_function_id, } => {
//             let anon_func = code_genie.find_node(anonymous_function_id).unwrap();
//             let full_unresolved_typ = code_genie.guess_type(anon_func, env_genie).unwrap();
//             let resolved_anon_func_typ =
//                 code_genie.try_to_resolve_all_generics(anon_func, full_unresolved_typ, env_genie);
//             arg_typ_for_anon_func(resolved_anon_func_typ)
//         }
//         VariableAntecedent::Argument { .. } => {
//             // unhandled so far
//             variable.typ.clone()
//         }
//
//         VariableAntecedent::MatchVariant { .. } => {
//             // also unhandled
//             variable.typ.clone()
//         }
//     }
// }

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

// // TODO: this should probably go near the code genie
// pub fn find_all_locals_preceding_with_resolving_generics<'a>(
//     search_position: SearchPosition,
//     locals_search_params: LocalsSearchParams,
//     code_genie: &'a CodeGenie,
//     env_genie: &'a EnvGenie)
//     -> impl Iterator<Item = Variable> + 'a {
//     find_all_locals_preceding_without_resolving_generics(search_position,
//                                                          locals_search_params,
//                                                          code_genie,
//                                                          env_genie).map(move |mut var| {
//                                                                        // var.typ = resolve_generics(&var, code_genie, env_genie);
//                                                                        var
//                                                                    })
// }

// pub fn find_all_locals_preceding_without_resolving_generics<'a>(
//     search_position: SearchPosition,
//     locals_search_params: LocalsSearchParams,
//     code_genie: &'a CodeGenie,
//     env_genie: &'a EnvGenie)
//     -> impl Iterator<Item = Variable> + 'a {
//     find_assignments_preceding(search_position, locals_search_params, code_genie, env_genie)
//         .chain(find_function_args_preceding(search_position, locals_search_params, code_genie, env_genie))
//         .chain(find_enum_variants_preceding(search_position, locals_search_params, code_genie, env_genie))
//         .chain(find_anon_func_args_for(search_position, locals_search_params, code_genie, env_genie))
//         .chain(find_for_loop_assignments_preceding(search_position, locals_search_params, code_genie, env_genie))
// }

// #[derive(Debug, Clone, Copy)]
// pub enum LocalsSearchParams {
//     NoFilter,
//     LocalsID(lang::ID),
// }
//
// impl LocalsSearchParams {
//     pub fn matches(&self, locals_id: lang::ID) -> bool {
//         match self {
//             Self::NoFilter => true,
//             Self::LocalsID(id) => id == &locals_id,
//         }
//     }
// }
//
// pub fn find_assignments_preceding<'a>(search_position: SearchPosition,
//                                       locals_search_params: LocalsSearchParams,
//                                       code_genie: &'a CodeGenie,
//                                       env_genie: &'a EnvGenie)
//                                       -> impl Iterator<Item = Variable> + 'a {
//     code_genie.find_assignments_that_come_before_code(search_position.before_code_id,
//                                                       search_position.is_search_inclusive)
//               .into_iter()
//               .filter_map(move |assignment| {
//                   if !locals_search_params.matches(assignment.id) {
//                       return None;
//                   }
//                   let assignment_clone: lang::Assignment = (*assignment).clone();
//                   let guessed_type =
//                       code_genie.guess_type(&lang::CodeNode::Assignment(assignment_clone),
//                                             env_genie);
//                   Some(Variable { locals_id: assignment.id,
//                                   variable_type:
//                                       VariableAntecedent::Assignment { assignment_id:
//                                                                            assignment.id },
//                                   // typ: guessed_type.unwrap(),
//                                   name: assignment.name.clone() })
//               })
// }
//
// pub fn find_function_args_preceding<'a>(_search_position: SearchPosition,
//                                         locals_search_params: LocalsSearchParams,
//                                         code_genie: &'a CodeGenie,
//                                         env_genie: &'a EnvGenie)
//                                         -> impl Iterator<Item = Variable> + 'a {
//     env_genie.code_takes_args(code_genie.root().id())
//              .filter_map(move |arg| {
//                  if !locals_search_params.matches(arg.id) {
//                      return None;
//                  }
//                  Some(Variable { locals_id: arg.id,
//                                  variable_type:
//                                      VariableAntecedent::FunctionArgument { argument_definition_id: arg.id },
//                                  // typ: arg.arg_type,
//                                  name: arg.short_name })
//              })
// }
//
// fn find_enum_variants_preceding<'a>(search_position: SearchPosition,
//                                     locals_search_params: LocalsSearchParams,
//                                     code_genie: &'a CodeGenie,
//                                     env_genie: &'a EnvGenie)
//                                     -> impl Iterator<Item = Variable> + 'a {
//     code_genie.find_enum_variants_preceding_iter(search_position.before_code_id, env_genie)
//               .filter_map(move |match_variant| {
//                   let assignment_id = match_variant.assignment_id();
//                   if !locals_search_params.matches(assignment_id) {
//                       return None;
//                   }
//                   Some(Variable { locals_id: assignment_id,
//                              variable_type:
//                                  VariableAntecedent::MatchVariant { match_statement_id:
//                                                                         match_variant.match_id,
//                                                                     variant_id:
//                                                                         match_variant.enum_variant
//                                                                                      .id },
//                              // typ: match_variant.typ,
//                              name: match_variant.enum_variant.name })
//               })
// }
//
// fn find_anon_func_args_for<'a>(search_position: SearchPosition,
//                                locals_search_params: LocalsSearchParams,
//                                code_genie: &'a CodeGenie,
//                                env_genie: &'a EnvGenie)
//                                -> impl Iterator<Item = Variable> + 'a {
//     code_genie.find_anon_func_parents(search_position.before_code_id)
//               .filter_map(move |anon_func| {
//                   let arg = &anon_func.as_anon_func().unwrap().takes_arg;
//                   if !locals_search_params.matches(arg.id) {
//                       return None;
//                   }
//                   let anon_func_typ = code_genie.guess_type(anon_func, env_genie).unwrap();
//                   // println!("arg name: {:?}", arg.short_name);
//                   // println!("guessed typ for anon_func: {:?}", anon_func_typ);
//                   // println!("variable typ: {:?}", anon_func_typ.params[0]);
//                   Some(Variable { locals_id: arg.id,
//                       variable_type:
//                                  VariableAntecedent::AnonFuncArgument { anonymous_function_id:
//                                                                             anon_func.id() },
//                              // typ: arg_typ_for_anon_func(anon_func_typ),
//                              name: arg.short_name.clone() })
//               })
// }
//
// pub fn find_for_loop_assignments_preceding<'a>(search_position: SearchPosition,
//                                                locals_search_params: LocalsSearchParams,
//                                                code_genie: &'a CodeGenie,
//                                                env_genie: &'a EnvGenie)
//                                                -> impl Iterator<Item = Variable> + 'a {
//     code_genie.find_for_loops_scopes_preceding(search_position.before_code_id)
//               .filter_map(move |for_loop| {
//                   println!("found one: {:?}", for_loop);
//                   let for_loop = for_loop.as_for_loop().unwrap();
//                   if !locals_search_params.matches(for_loop.id) {
//                       println!("gave up on: {:?}", locals_search_params);
//                       return None;
//                   }
//                   println!("didn't give up: {:?}", locals_search_params);
//                   println!("guessing type for {:?}", for_loop.list_expression);
//                   let typ = code_genie.guess_type(for_loop.list_expression.as_ref(), env_genie)
//                                       .unwrap();
//                   println!("guessed type: {:?}", typ);
//                   let typ = get_type_from_list(typ).unwrap();
//                   Some(Variable { variable_type: VariableAntecedent::ForLoop { for_loop_id:
//                                                                                    for_loop.id },
//                                   locals_id: for_loop.id,
//                                   // typ,
//                                   name: for_loop.variable_name.clone() })
//               })
// }
