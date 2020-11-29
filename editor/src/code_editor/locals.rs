use crate::code_editor::{CodeGenie, InsertionPoint};
use crate::insert_code_menu;
use cs::{lang, EnvGenie};

use serde_derive::{Deserialize, Serialize};

// just need this for debugging, tho maybe i'll keep it around, it's probably good to have
#[derive(Serialize, Deserialize, Debug)]
enum VariableType {
    Assignment,
    Argument,
    MatchVariant,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Variable {
    variable_type: VariableType,
    pub locals_id: lang::ID,
    pub(crate) typ: lang::Type,
    pub(crate) name: String,
}

fn find_anon_func_args_for<'a>(search_position: SearchPosition,
                               code_genie: &'a CodeGenie,
                               env_genie: &'a EnvGenie)
                               -> impl Iterator<Item = Variable> + 'a {
    code_genie.find_anon_func_parents(search_position.before_code_id)
              .map(move |anon_func| {
                  let arg = &anon_func.as_anon_func().unwrap().takes_arg;
                  let anon_func_typ = code_genie.guess_type(anon_func, env_genie).unwrap();
                  println!("arg name: {:?}", arg.short_name);
                  println!("guessed typ for anon_func: {:?}", anon_func_typ);
                  println!("variable typ: {:?}", anon_func_typ.params[0]);
                  Variable { variable_type: VariableType::Argument,
                             locals_id: arg.id,
                             // TODO: clean up this magic number
                             typ: anon_func_typ.params[0].clone(),
                             name: arg.short_name.clone() }
              })
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
pub fn find_all_locals_preceding<'a>(search_position: SearchPosition,
                                     code_genie: &'a CodeGenie,
                                     env_genie: &'a EnvGenie)
                                     -> impl Iterator<Item = Variable> + 'a {
    find_assignments_and_function_args_preceding(search_position, code_genie, env_genie)
        .chain(find_enum_variants_preceding(search_position, code_genie, env_genie))
        .chain(find_anon_func_args_for(search_position, code_genie, env_genie))
}

pub fn find_assignments_preceding<'a>(search_position: SearchPosition,
                                      code_genie: &'a CodeGenie,
                                      env_genie: &'a EnvGenie)
                                      -> impl Iterator<Item = Variable> + 'a {
    code_genie.find_assignments_that_come_before_code(search_position.before_code_id,
                                                      search_position.is_search_inclusive)
              .into_iter()
              .map(move |assignment| {
                  let assignment_clone: lang::Assignment = (*assignment).clone();
                  let guessed_type =
                      code_genie.guess_type(&lang::CodeNode::Assignment(assignment_clone),
                                            env_genie);
                  Variable { locals_id: assignment.id,
                             variable_type: VariableType::Assignment,
                             typ: guessed_type.unwrap(),
                             name: assignment.name.clone() }
              })
}

pub fn find_assignments_and_function_args_preceding<'a>(search_position: SearchPosition,
                                                        code_genie: &'a CodeGenie,
                                                        env_genie: &'a EnvGenie)
                                                        -> impl Iterator<Item = Variable> + 'a {
    find_assignments_preceding(search_position, code_genie, env_genie)
              .chain(env_genie.code_takes_args(code_genie.root().id())
                              .map(|arg| Variable { locals_id: arg.id,
                                                    variable_type: VariableType::Argument,
                                                    typ: arg.arg_type,
                                                    name: arg.short_name }))
}

fn find_enum_variants_preceding<'a>(search_position: SearchPosition,
                                    code_genie: &'a CodeGenie,
                                    env_genie: &'a EnvGenie)
                                    -> impl Iterator<Item = Variable> + 'a {
    code_genie.find_enum_variants_preceding_iter(search_position.before_code_id, env_genie)
              .map(|match_variant| Variable { locals_id: match_variant.assignment_id(),
                                              variable_type: VariableType::MatchVariant,
                                              typ: match_variant.typ,
                                              name: match_variant.enum_variant.name })
}
