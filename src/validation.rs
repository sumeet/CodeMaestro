use super::env_genie::EnvGenie;
use super::lang;

#[derive(Debug)]
pub enum ProblemPreventingRun {
    HasPlaceholderNode(lang::ID),
    FunctionCannotBeRun(lang::ID),
}

pub fn find_problems_for_code<'a>(code: &'a lang::CodeNode,
                                  env_genie: &'a EnvGenie)
                                  -> impl Iterator<Item = ProblemPreventingRun> + 'a {
    let function_cant_be_runs = find_functions_referred_to_by(&code, env_genie)
        // in the case of the JSON HTTP client, the test code refers to itself. 
        // so we want to skip it here or else we'll get a stack overflow
        .filter(move |func| { !can_be_run(func.as_ref(), env_genie) })
        .map(|func| ProblemPreventingRun::FunctionCannotBeRun(func.id()));

    find_placeholder_nodes(code).map(|id| ProblemPreventingRun::HasPlaceholderNode(id))
                                .chain(function_cant_be_runs)
}

pub fn can_be_run(func: &dyn lang::Function, env_genie: &EnvGenie) -> bool {
    let mut codes_from_func = func.cs_code();

    let any_code_contains_placeholders =
        codes_from_func.any(|code_block| {
                           // TODO: is there any way we can avoid cloning here???
                           let code_from_func = lang::CodeNode::Block(code_block.clone());
                           let has_any_placeholder_nodes =
                               find_placeholder_nodes(&code_from_func).next().is_some();
                           has_any_placeholder_nodes
                       });

    if any_code_contains_placeholders {
        return false;
    }

    func.cs_code().all(|code| {
                      // TODO: find a way not to clone in here
                      let code = lang::CodeNode::Block(code.clone());
                      let all_referred_functions_can_be_run =
                          find_functions_referred_to_by(&code, env_genie)
                              // in the case of the JSON HTTP client, the test code refers to itself. 
                              // so we want to skip it here or else we'll get a stack overflow
                              .filter(|found_func| found_func.id() != func.id())
                              .all(|func| {
                              can_be_run(func.as_ref(), env_genie)
                          });
                      all_referred_functions_can_be_run
                  })
}

fn find_placeholder_nodes(root: &lang::CodeNode) -> impl Iterator<Item = lang::ID> + '_ {
    root.self_with_all_children_dfs()
        .filter_map(|code_node| match code_node {
            lang::CodeNode::Placeholder(ph) => Some(ph.id),
            _ => None,
        })
}

fn find_functions_referred_to_by<'a>(root: &'a lang::CodeNode,
                                     env_genie: &'a EnvGenie)
                                     -> impl Iterator<Item = &'a Box<dyn lang::Function>> + 'a {
    root.self_with_all_children_dfs()
        .filter_map(move |code_node| {
            let function_reference = code_node.as_function_reference().ok()?;
            Some(env_genie.find_function(function_reference.function_id)
                          .unwrap())
        })
}
