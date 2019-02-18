use super::env;
use super::editor;
use super::lang;
use super::lang::Function;
use super::env_genie;
use super::code_editor::CodeGenie;
use super::code_function;
use super::code_generation;
use super::json_http_client::JSONHTTPClient;
use super::builtins;

use std::iter;
use lazy_static::lazy_static;

lazy_static! {
    static ref HTTP_FORM_PARAM_TYPE: lang::Type = {
        lang::Type::from_spec_id(*builtins::HTTP_FORM_PARAM_STRUCT_ID, vec![])
    };

    static ref LIST_OF_FORM_PARAMS: lang::Type = {
        lang::Type::with_params(
            &*lang::LIST_TYPESPEC,
            vec![HTTP_FORM_PARAM_TYPE.clone()])
    };
}


pub fn validate_and_fix(env: &mut env::ExecutionEnvironment, cmd_buffer: &mut editor::CommandBuffer) {
    Validator::new(env, cmd_buffer).validate_and_fix_all_code();
}

enum Problem {
    InvalidReturnType { function_id: lang::ID }
}

struct Validator<'a> {
    env: &'a env::ExecutionEnvironment,
    cmd_buffer: &'a mut editor::CommandBuffer,
}

impl<'a> Validator<'a> {
    fn new(env: &'a env::ExecutionEnvironment, cmd_buffer: &'a mut editor::CommandBuffer) -> Self {
        Self { env, cmd_buffer }
    }

    fn validate_and_fix_all_code(&mut self) {
        let env_genie = env_genie::EnvGenie::new(self.env);
        let problem_finder = ProblemFinder::new(&env_genie);

        let problems = env_genie.all_functions()
            .flat_map(|function| {
                problem_finder.find_problems(function.as_ref())
            });

        for problem in problems {
            self.fix(problem)
        }
    }

    fn fix(&mut self, problem: Problem) {
        match problem {
            Problem::InvalidReturnType { function_id } => self.fix_return(function_id)
        };
    }

    // fixes the return statement of code functions
    fn fix_return(&mut self, function_id: lang::ID) -> Option<()> {
        let func = self.env.find_function(function_id)?;
        if let Some(code_func) = func.downcast_ref::<code_function::CodeFunction>() {
            self.fix_code_func_return(code_func);
        // TODO: JSON HTTP client code blocks just return HTTP responses... so they call the regular
        // HTTP client. the only difference between them and a normal function is the struct builder
        } else if let Some(json_http_client) = func.downcast_ref::<JSONHTTPClient>() {
            self.fix_json_http_client_return(json_http_client)
        }
        Some(())
    }

    fn fix_code_func_return(&mut self, code_func: &code_function::CodeFunction) {
        let mut block = code_func.block.clone();
        if is_placeholder_expression(block.expressions.last()) {
            let mut placeholder = block.expressions.last().unwrap().into_placeholder().unwrap().clone();
            placeholder.typ = code_func.returns();
            *(block.expressions.last_mut().unwrap()) = lang::CodeNode::Placeholder(placeholder);
        } else {
            block.expressions.push(code_generation::new_placeholder("Return value".to_string(),
                                                                    code_func.returns()))
        }

        let mut new_cf = code_func.clone();
        new_cf.block = block;
        self.cmd_buffer.load_code_func(new_cf);
    }

    fn fix_json_http_client_return(&mut self, json_http_client: &JSONHTTPClient) {
        let mut new_client = json_http_client.clone();
        new_client.gen_url_params.expressions.push(
            code_generation::new_list_literal(HTTP_FORM_PARAM_TYPE.clone()));
        self.cmd_buffer.load_json_http_client(new_client)
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

    fn find_problems(&self, func: &lang::Function) -> Box<Iterator<Item = Problem>> {
        let type_that_should_be_returned = self.type_that_should_be_returned(func);
        let actual_type_returned = self.actual_type_returned(func);
        if type_that_should_be_returned.as_ref().map(|t| t.id()) != actual_type_returned.as_ref().map(|t| t.id()) {
            // TODO: keep this debug crap in here for now. i want to see it in the console. later,
            // we'll surface this to users and ask them to accept the changes (or undo)
            let t1_symbol = self.env_genie.get_name_for_type(actual_type_returned.as_ref().unwrap()).unwrap();
            let t2_symbol = self.env_genie.get_name_for_type(type_that_should_be_returned.as_ref().unwrap()).unwrap();
            println!("block has type {} but func returns type {}", t1_symbol, t2_symbol);
            Box::new(std::iter::once(Problem::InvalidReturnType { function_id: func.id() }))
        } else {
            Box::new(std::iter::empty())
        }
    }

    fn type_that_should_be_returned(&self, func: &lang::Function) -> Option<lang::Type> {
        if let Some(code_func) = func.downcast_ref::<code_function::CodeFunction>() {
            Some(code_func.returns())
        } else if let Some(_) = func.downcast_ref::<JSONHTTPClient>() {
            Some(LIST_OF_FORM_PARAMS.clone())
        } else {
            None
        }
    }

    fn actual_type_returned(&self, func: &lang::Function) -> Option<lang::Type> {
        if let Some(code_func) = func.downcast_ref::<code_function::CodeFunction>() {
            let code_genie = CodeGenie::new(code_func.code());
            Some(code_genie.guess_type(code_genie.root(), self.env_genie))
        } else if let Some(json_http_client) = func.downcast_ref::<JSONHTTPClient>() {
            let code_genie = CodeGenie::new(lang::CodeNode::Block(json_http_client.gen_url_params.clone()));
            Some(code_genie.guess_type(code_genie.root(), self.env_genie))
        } else {
            None
        }
    }
}

