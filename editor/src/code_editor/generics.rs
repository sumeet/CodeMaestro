use crate::code_editor::CodeGenie;
use cs::lang::TypeSpec;
use cs::{lang, EnvGenie};

pub fn resolve_generics_for_function_call(func_call: &lang::FunctionCall,
                                          code_genie: &CodeGenie,
                                          env_genie: &EnvGenie)
                                          -> lang::Type {
    let func = env_genie.find_function(func_call.function_reference().function_id)
                        .unwrap();
    // let defined_generics = func.defines_generics();
    let mut return_typ = func.returns();
    let original_return_typ = return_typ.clone();

    'for_generic: for defined_generic in func.defines_generics() {
        let paths_to_generic_found_in_defined_return_typ =
            original_return_typ.find_typespec_id_in_params(defined_generic.id())
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
                    code_genie.guess_type(&func_call.iter_args().nth(i).unwrap().expr, env_genie)
                              .unwrap();
                for (path_to_generic_found_in_defined_arg, _) in
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
