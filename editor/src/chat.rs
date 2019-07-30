use crate::code_generation::{new_function_call, new_string_literal};
use cs::builtins;
use cs::chat_trigger::ChatTrigger;
use cs::lang;
use lazy_static::lazy_static;
use rand::seq::sample_slice;

lazy_static! {
    static ref VERBS: Vec<&'static str> = { include_str!("../verbs.txt").lines().collect() };
}

pub fn example_chat_trigger() -> ChatTrigger {
    let arg = lang::CodeNode::Argument(lang::Argument { id: Default::default(),
                                              argument_definition_id:
                                                  *builtins::CHAT_REPLY_MESSAGE_ARG_ID,
                                              expr:
                                                  Box::new(new_string_literal("Hi there!".into())) });
    ChatTrigger { id: Default::default(),
                  prefix: format!(".{}", verb_me()),
                  code: lang::Block { expressions:
                                          vec![new_function_call(*builtins::CHAT_REPLY_FUNC_ID,
                                                                 vec![arg])],
                                      id: Default::default() } }
}

fn verb_me() -> &'static str {
    sample_slice(&mut rand::thread_rng(), &VERBS, 1)[0]
}
