use crate::code_generation::{new_function_call, new_string_literal};
use cs::builtins;
use cs::chat_program::ChatProgram;
use cs::lang;
use lazy_static::lazy_static;
use rand::rngs::OsRng;
use rand::seq::SliceRandom;

lazy_static! {
    static ref VERBS: Vec<&'static str> = { include_str!("../verbs.txt").lines().collect() };
}

pub fn example_chat_program() -> ChatProgram {
    let arg = lang::CodeNode::Argument(lang::Argument { id: lang::new_id(),
                                              argument_definition_id:
                                                  *builtins::CHAT_REPLY_MESSAGE_ARG_ID,
                                              expr:
                                                  Box::new(new_string_literal("Hi there!".into())) });
    ChatProgram { id: lang::new_id(),
                  prefix: format!(".{}", verb_me()),
                  code: lang::Block { expressions:
                                          vec![new_function_call(*builtins::CHAT_REPLY_FUNC_ID,
                                                                 vec![arg])],
                                      id: lang::new_id() } }
}

fn verb_me() -> &'static str {
    VERBS.choose(&mut OsRng).unwrap()
}
