use super::code_editor::CodeGenie;
use super::code_generation;
use super::editor;
use cs::env_genie;
use cs::lang;
use cs::lang::Function;
use cs::{env, structs};

use crate::code_editor::locals::find_antecedent_for_variable_reference;
use crate::code_editor::{required_return_type, CodeLocation};
use crate::editor::Controller;
use cs::env_genie::EnvGenie;
use gen_iter::GenIter;
use std::collections::{HashMap, HashSet};
use std::iter::once;

// TODO: instead of applying fixes right away, show them in a popup modal and ask the user to either
// confirm or back out the change that caused it
pub fn validate_and_fix(env: &mut env::ExecutionEnvironment,
                        controller: &Controller,
                        cmd_buffer: &mut editor::CommandBuffer) {
    Validator::new(env, controller, cmd_buffer).validate_and_fix_all_code();
}

fn all_code<'a>(env_genie: &'a EnvGenie,
                controller: &'a Controller)
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
    ).chain(
        controller.list_scripts().map(|script| {
            (CodeLocation::Script(script.id()), &script.code)
        }))
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
    FieldsMissingInStructLiteral {
        location: CodeLocation,
        block: lang::Block,
        struct_literal_id: lang::ID,
        missing_fields: Vec<structs::StructField>,
        extra_field_ids: HashSet<lang::ID>,
    },
}

struct Validator<'a> {
    env: &'a env::ExecutionEnvironment,
    controller: &'a Controller,
    env_genie: EnvGenie<'a>,
    cmd_buffer: &'a mut editor::CommandBuffer,
}

impl<'a> Validator<'a> {
    fn new(env: &'a env::ExecutionEnvironment,
           controller: &'a Controller,
           cmd_buffer: &'a mut editor::CommandBuffer)
           -> Self {
        let env_genie = EnvGenie::new(env);
        Self { env,
               controller,
               env_genie,
               cmd_buffer }
    }

    fn validate_and_fix_all_code(&mut self) {
        let env_genie = env_genie::EnvGenie::new(self.env);
        let problem_finder = FixableProblemFinder::new(&env_genie);

        let mut problems = vec![];
        for (location, block) in all_code(&env_genie, self.controller) {
            // TODO: need to get rid of this clone and change this back into an iterator
            let code_node = lang::CodeNode::Block(block.clone());
            for problem in problem_finder.find_problems(location, block, &code_node) {
                problems.push(problem);
            }
        }

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
            FixableProblem::FieldsMissingInStructLiteral { location,
                                                           block,
                                                           struct_literal_id,
                                                           extra_field_ids,
                                                           missing_fields, } => {
                let mut code = lang::CodeNode::Block(block);
                let mut strukt_literal = code.find_node(struct_literal_id)
                                             .unwrap()
                                             .as_struct_literal()
                                             .unwrap()
                                             .clone();
                strukt_literal.fields
                              .drain_filter(|field| extra_field_ids.contains(&field.id()));
                for missing_field in missing_fields {
                    strukt_literal.fields.push(code_generation::new_struct_literal_field_placeholder(&missing_field));
                }

                code.replace_with(strukt_literal.id,
                                  lang::CodeNode::StructLiteral(strukt_literal));
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
        } else if required_return_type.matches_spec(&*lang::MAP_TYPESPEC) {
            let value_typ = required_return_type.params.pop().unwrap();
            let key_typ = required_return_type.params.pop().unwrap();
            code_generation::new_map_literal(key_typ, value_typ)
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
            CodeLocation::Script(script_id) => {
                self.cmd_buffer.add_controller_command(move |controller| {
                                   let mut script =
                                       controller.find_script(script_id).unwrap().clone();
                                   script.code = block;
                                   controller.load_script(script);
                               })
            }
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
                     block: &'a lang::Block,
                     code_node: &'a lang::CodeNode)
                     -> impl Iterator<Item = FixableProblem> + 'a {
        GenIter(move || {
            for prob in self.find_variable_reference_problem(location, block) {
                yield prob;
            }
            if let Some(prob) = self.find_return_type_problem(location, block) {
                yield prob;
            }
            for prob in self.find_missing_struct_literal_field(location, code_node) {
                yield prob;
            }
        })
    }

    // this would've happened if changing a struct
    fn find_missing_struct_literal_field(&'a self,
                                         location: CodeLocation,
                                         code_node: &'a lang::CodeNode)
                                         -> impl Iterator<Item = FixableProblem> + 'a {
        GenIter(move || {
            // TODO: need to impl Children / CodeIteration / CodeNode for Block and not have to clone
            for strukt_literal in code_node.all_children_dfs_iter()
                                           .filter_map(|cn| cn.as_struct_literal().ok())
            {
                let field_ids_in_code_literal = strukt_literal.fields()
                                                              .map(|field| field.struct_field_id)
                                                              .collect::<HashSet<_>>();
                let strukt = self.env_genie
                                 .find_struct(strukt_literal.struct_id)
                                 .unwrap();
                let fields_defined_in_struct = strukt.fields
                                                     .iter()
                                                     .map(|field| (field.id, field))
                                                     .collect::<HashMap<_, _>>();
                let mut fields_not_represented_in_literal =
                    fields_defined_in_struct.iter()
                                            .filter_map(|(field_id, field)| {
                                                if !field_ids_in_code_literal.contains(field_id) {
                                                    Some(*field)
                                                } else {
                                                    None
                                                }
                                            })
                                            .peekable();

                if fields_not_represented_in_literal.peek().is_some() {
                    let extra_field_ids_in_literal = field_ids_in_code_literal.iter().filter(|field_id| {
                        !fields_defined_in_struct.contains_key(field_id)
                    }).cloned();
                    yield FixableProblem::FieldsMissingInStructLiteral { location,
                                                                         block:
                                                                             code_node.as_block()
                                                                                      .unwrap()
                                                                                      .clone(),
                                                                         struct_literal_id:
                                                                             strukt_literal.id,
                                                                         missing_fields: fields_not_represented_in_literal.cloned().collect(),
                                                                         extra_field_ids: extra_field_ids_in_literal.collect() }
                }
            }
        })
    }

    fn find_return_type_problem(&self,
                                location: CodeLocation,
                                block: &lang::Block)
                                -> Option<FixableProblem> {
        let required_return_type = required_return_type(location, self.env_genie)?;
        if !self.env_genie
                .types_match(&required_return_type, &self.returned_type(block))
        {
            println!("code was: {:?}", block);
            println!("found required return type problem, got {:?}, expected {:?}",
                     self.returned_type(block),
                     required_return_type);
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
                    if find_antecedent_for_variable_reference(var_ref,
                                                              false,
                                                              &code_genie,
                                                              self.env_genie).is_none()
                    {
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
                })
                .collect()
    }

    fn returned_type(&self, block: &lang::Block) -> lang::Type {
        let code_genie = CodeGenie::new(lang::CodeNode::Block(block.clone()));
        code_genie.guess_type(code_genie.root(), self.env_genie)
                  .unwrap()
    }
}
