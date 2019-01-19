use super::env;
use super::lang;
use super::lang::Function;
use super::env_genie;
use super::code_editor::CodeGenie;
use super::code_function;
use super::code_generation;

enum Fix {
    FixReturn(code_function::CodeFunction)
}

pub fn validate_and_fix(env: &mut env::ExecutionEnvironment) {
    let mut fixes = vec![];
    let env_genie = env_genie::EnvGenie::new(&env);
    for code_func in env_genie.list_code_funcs() {
        let code_genie = CodeGenie::new(code_func.code());
        if code_genie.guess_type(code_genie.root(), &env_genie) != code_func.returns() {
            fixes.push(Fix::FixReturn(code_func.clone()))
        }
    }

    for fix in fixes {
        execute_fix(fix, env)
    }
}

fn execute_fix(fix: Fix, env: &mut env::ExecutionEnvironment) {
    match fix {
        Fix::FixReturn(code_func) => fix_return(code_func, env)
    }
}

fn fix_return(mut code_func: code_function::CodeFunction, env: &mut env::ExecutionEnvironment) {
    let mut block = code_func.block.clone();
    if is_placeholder_expression(block.expressions.last()) {
        let mut placeholder = block.expressions.last().unwrap().into_placeholder().unwrap().clone();
        placeholder.typ = code_func.returns();
        *(block.expressions.last_mut().unwrap()) = lang::CodeNode::Placeholder(placeholder);
    } else {
        block.expressions.push(code_generation::new_placeholder("Returned", code_func.returns()))
    }

    env.add_function(code_func);
}

fn is_placeholder_expression(code_node: Option<&lang::CodeNode>) -> bool {
    match code_node {
        Some(lang::CodeNode::Placeholder(_)) => true,
        _ => false,
    }
}
