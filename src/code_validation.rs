use super::env;
use super::editor;
use super::lang;
use super::lang::Function;
use super::env_genie;
use super::code_editor::CodeGenie;
use super::code_function;
use super::code_generation;

enum Fix {
    FixReturn(code_function::CodeFunction)
}

struct Validator<'a> {
    env: &'a env::ExecutionEnvironment,
    cmd_buffer: &'a mut editor::CommandBuffer,
}

impl<'a> Validator<'a> {
    fn new(env: &'a env::ExecutionEnvironment, cmd_buffer: &'a mut editor::CommandBuffer) -> Self {
        Self { env, cmd_buffer }
    }

    fn validate_and_fix(&mut self) {
        let mut fixes = vec![];
        let env_genie = env_genie::EnvGenie::new(&self.env);
        for code_func in env_genie.list_code_funcs() {
            let code_genie = CodeGenie::new(code_func.code());
            if code_genie.guess_type(code_genie.root(), &env_genie) != code_func.returns() {
                let t1_symbol = env_genie.get_name_for_type(&code_genie.guess_type(code_genie.root(), &env_genie));
                let t2_symbol = env_genie.get_name_for_type(&code_func.returns());
                println!("block has type {} but func returns type {}", t1_symbol, t2_symbol);
                fixes.push(Fix::FixReturn(code_func.clone()))
            }
        }

        for fix in fixes {
            self.execute_fix(fix)
        }
    }

    fn execute_fix(&mut self, fix: Fix) {
        match fix {
            Fix::FixReturn(code_func) => self.fix_return(code_func)
        }
    }

    // fixes the return statement of code functions
    fn fix_return(&mut self, code_func: code_function::CodeFunction) {
        let mut block = code_func.block.clone();
        if is_placeholder_expression(block.expressions.last()) {
            let mut placeholder = block.expressions.last().unwrap().into_placeholder().unwrap().clone();
            placeholder.typ = code_func.returns();
            *(block.expressions.last_mut().unwrap()) = lang::CodeNode::Placeholder(placeholder);
        } else {
            block.expressions.push(code_generation::new_placeholder("Returned", code_func.returns()))
        }

        let mut new_cf = code_func.clone();
        new_cf.block = block;
        self.cmd_buffer.load_code_func(new_cf);
    }
}

pub fn validate_and_fix(env: &mut env::ExecutionEnvironment, cmd_buffer: &mut editor::CommandBuffer) {
    Validator::new(env, cmd_buffer).validate_and_fix();
}

fn is_placeholder_expression(code_node: Option<&lang::CodeNode>) -> bool {
    match code_node {
        Some(lang::CodeNode::Placeholder(_)) => true,
        _ => false,
    }
}
