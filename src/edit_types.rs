use super::lang;

pub fn set_typespec<T: lang::TypeSpec>(t: &mut lang::Type, typespec: &T,
                    nesting_level: &[usize]) {
    let mut type_to_modify = t;
    for param_index in nesting_level {
        type_to_modify = &mut type_to_modify.params[*param_index]
    }
    type_to_modify.typespec_id = typespec.id();
    type_to_modify.params.truncate(typespec.num_params());
    let num_missing_params = typespec.num_params() - type_to_modify.params.len();
    for _ in 0..num_missing_params {
        type_to_modify.params.push(lang::Type::from_spec(&lang::NULL_TYPESPEC))
    }
}
