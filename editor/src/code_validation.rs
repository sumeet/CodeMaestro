use super::code_editor::CodeGenie;
use super::code_generation;
use super::editor;
use cs::builtins;
use cs::env;
use cs::env_genie;
use cs::lang;
use cs::lang::Function;

use crate::code_editor::CodeLocation;
use cs::env_genie::EnvGenie;
use lazy_static::lazy_static;
use std::iter::once;

lazy_static! {
    static ref HTTP_FORM_PARAM_TYPE: lang::Type =
        { lang::Type::from_spec_id(*builtins::HTTP_FORM_PARAM_STRUCT_ID, vec![]) };
    static ref LIST_OF_FORM_PARAMS: lang::Type =
        { lang::Type::with_params(&*lang::LIST_TYPESPEC, vec![HTTP_FORM_PARAM_TYPE.clone()]) };
}

// TODO: instead of applying fixes right away, show them in a popup modal and ask the user to either
// confirm or back out the change that caused it
pub fn validate_and_fix(env: &mut env::ExecutionEnvironment,
                        cmd_buffer: &mut editor::CommandBuffer) {
    Validator::new(env, cmd_buffer).validate_and_fix_all_code();
}

fn all_code<'a>(env_genie: &'a EnvGenie)
                -> impl Iterator<Item = (CodeLocation, &'a lang::Block)> + 'a {
    env_genie.list_code_funcs().map(|code_func| {
        (CodeLocation::Function(code_func.id()), &code_func.block)
    }).chain(
        env_genie.list_json_http_clients().flat_map(|json_http_client| {
            once((CodeLocation::JSONHTTPClientURLParams(json_http_client.id()), &json_http_client.gen_url_params))
                .chain(once((CodeLocation::JSONHTTPClientURL(json_http_client.id()), &json_http_client.gen_url)))
        })
    )
}

#[derive(Debug)]
enum Problem {
    InvalidReturnType {
        location: CodeLocation,
        block: lang::Block,
        required_return_type: lang::Type,
    },
}

struct Validator<'a> {
    env: &'a env::ExecutionEnvironment,
    env_genie: EnvGenie<'a>,
    cmd_buffer: &'a mut editor::CommandBuffer,
}

impl<'a> Validator<'a> {
    fn new(env: &'a env::ExecutionEnvironment, cmd_buffer: &'a mut editor::CommandBuffer) -> Self {
        let env_genie = EnvGenie::new(env);
        Self { env,
               env_genie,
               cmd_buffer }
    }

    fn validate_and_fix_all_code(&mut self) {
        let env_genie = env_genie::EnvGenie::new(self.env);
        let problem_finder = ProblemFinder::new(&env_genie);

        let problems = all_code(&env_genie).flat_map(|(location, block)| {
                                               problem_finder.find_problems(location, block)
                                           });

        for problem in problems {
            self.fix(problem)
        }
    }

    fn fix(&mut self, problem: Problem) {
        match problem {
            Problem::InvalidReturnType { location,
                                         block,
                                         required_return_type, } => {
                let new_block = self.fix_return_type(block, required_return_type);
                self.update_code(location, new_block);
            }
        };
    }

    // fix the return type of a block by inserting an expression at the end of the correct type
    fn fix_return_type(&self,
                       mut block: lang::Block,
                       mut required_return_type: lang::Type)
                       -> lang::Block {
        // if it needs to return a list, then instead of inserting a Placeholder, we can do better
        // and insert an empty list literal. satisfies the type and might be closer to what the user
        // wants than a Placeholder
        let expr_to_insert_at_the_end = if required_return_type.matches_spec(&*lang::LIST_TYPESPEC)
        {
            // TODO: should we only do this list stuff for GenURLParams? because in a code func, if
            // the user keeps changing the list type, they'll end up with a huge stack of unused list
            // literals they'll have to clean up manually.
            code_generation::new_list_literal(required_return_type.params.remove(0))
        } else {
            // TODO: we probably also want something to clean up this "Return value" placeholder in
            // case the preceding line is actually the correct return type. i suppose that would be
            // another "Fixer"
            if is_placeholder_expression(block.expressions.last()) {
                block.expressions.pop();
            }
            code_generation::new_placeholder("Return value".to_string(), required_return_type)
        };
        block.expressions.push(expr_to_insert_at_the_end);
        block
    }

    fn update_code(&mut self, location: CodeLocation, block: lang::Block) {
        match location {
            CodeLocation::Function(func_id) => {
                let mut code_func = self.env_genie.get_code_func(func_id).unwrap().clone();
                code_func.block = block;
                self.cmd_buffer.load_code_func(code_func)
            }
            CodeLocation::Script(_) => panic!("we don't check scripts atm"),
            CodeLocation::Test(_) => panic!("we don't check tests atm"),
            CodeLocation::JSONHTTPClientURLParams(id) => {
                let mut json_http_client = self.env_genie.get_json_http_client(id).unwrap().clone();
                json_http_client.gen_url_params = block;
                self.cmd_buffer.load_json_http_client(json_http_client)
            }
            CodeLocation::JSONHTTPClientURL(id) => {
                let mut json_http_client = self.env_genie.get_json_http_client(id).unwrap().clone();
                json_http_client.gen_url = block;
                self.cmd_buffer.load_json_http_client(json_http_client)
            }
            CodeLocation::ChatTrigger(id) => {
                let mut chat_trigger = self.env_genie.get_chat_trigger(id).unwrap().clone();
                chat_trigger.code = block;
                self.cmd_buffer.load_chat_trigger(chat_trigger)
            }
        }
    }
}

fn is_placeholder_expression(code_node: Option<&lang::CodeNode>) -> bool {
    match code_node {
        Some(lang::CodeNode::Placeholder(_)) => true,
        _ => false,
    }
}

struct ProblemFinder<'a> {
    env_genie: &'a env_genie::EnvGenie<'a>,
}

impl<'a> ProblemFinder<'a> {
    fn new(env_genie: &'a env_genie::EnvGenie<'a>) -> Self {
        Self { env_genie }
    }

    fn find_problems(&self,
                     location: CodeLocation,
                     block: &lang::Block)
                     -> impl Iterator<Item = Problem> {
        self.find_return_type_problem(location, block).into_iter()
    }

    fn find_return_type_problem(&self,
                                location: CodeLocation,
                                block: &lang::Block)
                                -> Option<Problem> {
        let required_return_type = self.required_return_type(location)?;
        if !required_return_type.matches(&self.returned_type(block)) {
            Some(Problem::InvalidReturnType { location,
                                              block: block.clone(),
                                              required_return_type })
        } else {
            None
        }
    }

    fn required_return_type(&self, location: CodeLocation) -> Option<lang::Type> {
        match location {
            CodeLocation::Function(func_id) => {
                Some(self.env_genie.get_code_func(func_id).unwrap().returns())
            }
            CodeLocation::JSONHTTPClientURLParams(_) => Some(LIST_OF_FORM_PARAMS.clone()),
            CodeLocation::JSONHTTPClientURL(_) => {
                Some(lang::Type::from_spec(&*lang::STRING_TYPESPEC))
            }
            CodeLocation::ChatTrigger(_) | CodeLocation::Script(_) | CodeLocation::Test(_) => None,
        }
    }

    fn returned_type(&self, block: &lang::Block) -> lang::Type {
        let code_genie = CodeGenie::new(lang::CodeNode::Block(block.clone()));
        code_genie.guess_type(code_genie.root(), self.env_genie)
    }
}