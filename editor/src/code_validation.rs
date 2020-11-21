use super::code_editor::CodeGenie;
use super::code_generation;
use super::editor;
use cs::env;
use cs::env_genie;
use cs::lang;
use cs::lang::Function;

use crate::code_editor::{required_return_type, CodeLocation};
use crate::insert_code_menu::{find_all_locals_preceding, SearchPosition};
use cs::env_genie::EnvGenie;
use gen_iter::GenIter;
use std::iter::once;

// TODO: instead of applying fixes right away, show them in a popup modal and ask the user to either
// confirm or back out the change that caused it
pub fn validate_and_fix(env: &mut env::ExecutionEnvironment,
                        cmd_buffer: &mut editor::CommandBuffer) {
    Validator::new(env, cmd_buffer).validate_and_fix_all_code();
}

fn all_code<'a>(env_genie: &'a EnvGenie)
                -> impl Iterator<Item = (CodeLocation, &'a lang::Block)> + 'a {
    let chat_programs = env_genie.list_chat_programs()
                                 .map(|cp| (CodeLocation::ChatProgram(cp.id()), &cp.code));

    env_genie.list_code_funcs().map(|code_func| {
        (CodeLocation::Function(code_func.id()), &code_func.block)
    }).chain(chat_programs).chain(
        env_genie.list_json_http_clients().flat_map(|json_http_client| {
            once((CodeLocation::JSONHTTPClientURLParams(json_http_client.id()), &json_http_client.gen_url_params_code))
                .chain(once((CodeLocation::JSONHTTPClientURL(json_http_client.id()), &json_http_client.gen_url_code)))
                .chain(once((CodeLocation::JSONHTTPClientTestSection(json_http_client.id()), &json_http_client.test_code)))
                .chain(once((CodeLocation::JSONHTTPClientTransform(json_http_client.id()), &json_http_client.transform_code)))
        })
    )
}

// TODO: should we have a warning-style popup (can be a side menu as well) showing if the program
// changed anything automatically? like if we fixed the return type, telling the user what we did
// so they're not confused by what just happened.... probably...
#[derive(Debug)]
enum FixableProblem {
    InvalidReturnType {
        location: CodeLocation,
        block: lang::Block,
        required_return_type: lang::Type,
    },
    MissingVariableReference {
        location: CodeLocation,
        block: lang::Block,
        variable_reference_id: lang::ID,
        typ: lang::Type,
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
        let problem_finder = FixableProblemFinder::new(&env_genie);

        let problems = all_code(&env_genie).flat_map(|(location, block)| {
                                               problem_finder.find_problems(location, block)
                                           });

        for problem in problems {
            self.fix(problem)
        }
    }

    fn fix(&mut self, problem: FixableProblem) {
        match problem {
            FixableProblem::InvalidReturnType { location,
                                                block,
                                                required_return_type, } => {
                let new_block = self.fix_return_type(block, required_return_type);
                self.update_code(location, new_block);
            }
            FixableProblem::MissingVariableReference { location,
                                                       typ,
                                                       block,
                                                       variable_reference_id, } => {
                let mut code = lang::CodeNode::Block(block);

                code.replace_with(variable_reference_id,
                                  code_generation::new_placeholder("Missing variable".into(), typ));

                self.update_code(location, code.as_block().unwrap().clone());
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
            let typename = self.env_genie
                               .get_name_for_type(&required_return_type)
                               .unwrap();
            let placeholder_text = format!("The last expression must return a {}", typename);
            code_generation::new_placeholder(placeholder_text, required_return_type)
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
                json_http_client.gen_url_params_code = block;
                self.cmd_buffer.load_json_http_client(json_http_client)
            }
            CodeLocation::JSONHTTPClientURL(id) => {
                let mut json_http_client = self.env_genie.get_json_http_client(id).unwrap().clone();
                json_http_client.gen_url_code = block;
                self.cmd_buffer.load_json_http_client(json_http_client)
            }
            CodeLocation::JSONHTTPClientTestSection(id) => {
                let mut json_http_client = self.env_genie.get_json_http_client(id).unwrap().clone();
                json_http_client.gen_url_code = block;
                self.cmd_buffer.load_json_http_client(json_http_client)
            }
            CodeLocation::JSONHTTPClientTransform(id) => {
                let mut json_http_client = self.env_genie.get_json_http_client(id).unwrap().clone();
                json_http_client.transform_code = block;
                self.cmd_buffer.load_json_http_client(json_http_client)
            }
            CodeLocation::ChatProgram(id) => {
                let mut chat_program = self.env_genie.get_chat_program(id).unwrap().clone();
                chat_program.code = block;
                self.cmd_buffer.load_chat_program(chat_program)
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

struct FixableProblemFinder<'a> {
    env_genie: &'a env_genie::EnvGenie<'a>,
}

impl<'a> FixableProblemFinder<'a> {
    fn new(env_genie: &'a env_genie::EnvGenie<'a>) -> Self {
        Self { env_genie }
    }

    fn find_problems(&'a self,
                     location: CodeLocation,
                     block: &'a lang::Block)
                     -> impl Iterator<Item = FixableProblem> + 'a {
        GenIter(move || {
            for prob in self.find_variable_reference_problem(location, block) {
                yield prob;
            }
            if let Some(prob) = self.find_return_type_problem(location, block) {
                yield prob;
            }
        })
    }

    fn find_return_type_problem(&self,
                                location: CodeLocation,
                                block: &lang::Block)
                                -> Option<FixableProblem> {
        let required_return_type = required_return_type(location, self.env_genie)?;
        if !required_return_type.matches(&self.returned_type(block)) {
            Some(FixableProblem::InvalidReturnType { location,
                                                     block: block.clone(),
                                                     required_return_type })
        } else {
            None
        }
    }

    fn find_variable_reference_problem(&self,
                                       location: CodeLocation,
                                       block: &lang::Block)
                                       -> Vec<FixableProblem> {
        let block = block.clone();
        let code = lang::CodeNode::Block(block.clone());
        let code_genie = CodeGenie::new(code.clone());
        let var_refs =
            code.all_children_dfs_iter().filter_map(|code| match code {
                                            lang::CodeNode::VariableReference(var_ref) => {
                                                Some(var_ref)
                                            }
                                            _ => None,
                                        });
        var_refs.filter_map(move |var_ref| {
            let search_position = SearchPosition { before_code_id: var_ref.id,
                is_search_inclusive: false };
            if find_all_locals_preceding(search_position, &code_genie, self.env_genie).find(|variable| {
                variable.locals_id == var_ref.assignment_id
            }).is_none() {
                let code_node = lang::CodeNode::VariableReference(var_ref.clone());
                Some(FixableProblem::MissingVariableReference {
                    location,
                    block: block.clone(),
                    variable_reference_id: var_ref.id,
                    typ: code_genie.guess_type(&code_node, self.env_genie).unwrap(),
                })
            } else {
                None
            }
        }).collect()
    }

    fn returned_type(&self, block: &lang::Block) -> lang::Type {
        let code_genie = CodeGenie::new(lang::CodeNode::Block(block.clone()));
        code_genie.guess_type(code_genie.root(), self.env_genie)
                  .unwrap()
    }
}
