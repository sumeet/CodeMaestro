use crate::code_editor::CodeGenie;
use cs::lang::TypeSpec;
use cs::{lang, EnvGenie};
use std::collections::HashSet;

pub fn resolve_generics_for_function_call(func_call: &lang::FunctionCall,
                                          code_genie: &CodeGenie,
                                          env_genie: &EnvGenie,
                                          prev_node_ids: HashSet<lang::ID>)
                                          -> lang::Type {
    let func = env_genie.find_function(func_call.function_reference().function_id)
                        .unwrap();
    let original_return_typ = func.returns();
    resolve_generic_type_using_function_call_args(&original_return_typ,
                                                  func.as_ref(),
                                                  &func_call,
                                                  code_genie,
                                                  env_genie,
                                                  prev_node_ids)
}

pub fn resolve_generics_for_function_call_argument(argument: &lang::Argument,
                                                   code_genie: &CodeGenie,
                                                   env_genie: &EnvGenie,
                                                   prev_node_ids: HashSet<lang::ID>)
                                                   -> lang::Type {
    let func_call = code_genie.find_parent(argument.id)
                              .unwrap()
                              .as_function_call()
                              .unwrap();
    let (func, arg_def) = env_genie.get_arg_definition(argument.argument_definition_id)
                                   .unwrap();

    resolve_generic_type_using_function_call_args(&arg_def.arg_type,
                                                  func,
                                                  func_call,
                                                  code_genie,
                                                  env_genie,
                                                  prev_node_ids)
}

// pub fn resolve_generics_for_anonymous_function_return_typ(original_typ: &lang::Type,
//                                                           anon_func: &lang::AnonymousFunction,
//                                                           code_genie: &CodeGenie,
//                                                           env_genie: &EnvGenie)
//                                                           -> lang::Type {
//     let mut paths_and_generic_typs =
//         original_typ.paths_and_types_containing_self()
//                     .filter(|(path, typ)| env_genie.is_generic(typ.typespec_id))
//                     .peekable();
//     if paths_and_generic_typs.peek().is_none() {
//         return original_typ.clone();
//     }
// }

fn resolve_generic_type_using_function_call_args(original_typ: &lang::Type,
                                                 func: &dyn lang::Function,
                                                 func_call: &lang::FunctionCall,
                                                 code_genie: &CodeGenie,
                                                 env_genie: &EnvGenie,
                                                 prev_node_ids: HashSet<lang::ID>)
                                                 -> lang::Type {
    let mut return_typ = original_typ.clone();

    println!("the original type we're resolving the generic for is: {:?}",
             env_genie.get_type_display_info(&original_typ));
    println!("in other words, {:?}", original_typ);

    println!("looking through generics defined in func {}: {:?}",
             func.name(),
             func.defines_generics());
    'for_generic: for defined_generic in func.defines_generics() {
        let paths_to_generic_found_in_defined_return_typ =
            original_typ.find_typespec_id_in_params(defined_generic.id())
                        .collect::<Vec<_>>();
        if paths_to_generic_found_in_defined_return_typ.is_empty() {
            continue;
        }

        for (i, defined_arg) in func.takes_args().into_iter().enumerate() {
            let mut paths_to_generic_found_in_defined_arg =
                defined_arg.arg_type
                           .find_typespec_id_in_params(defined_generic.id())
                           .peekable();
            if paths_to_generic_found_in_defined_arg.peek().is_some() {
                let guessed_typ_from_executing_param =
                    dbg!(code_genie.guess_type_rec(dbg!(&func_call.iter_args()
                                                                  .nth(i)
                                                                  .unwrap()
                                                                  .expr),
                                                   env_genie,
                                                   prev_node_ids.clone())
                                   .unwrap());
                for (path_to_generic_found_in_defined_arg, _typ) in
                    paths_to_generic_found_in_defined_arg
                {
                    let possibly_filled_in_generic = guessed_typ_from_executing_param.get_param_using_path(&path_to_generic_found_in_defined_arg);
                    if possibly_filled_in_generic.typespec_id != defined_generic.id() {
                        for (path_to_generic_found_in_defined_return_typ, _) in
                            &paths_to_generic_found_in_defined_return_typ
                        {
                            *return_typ.get_param_using_path_mut(&path_to_generic_found_in_defined_return_typ) = possibly_filled_in_generic.clone();
                            continue 'for_generic;
                        }
                    }
                }
            }
        }
    }

    return_typ
}
