use super::env;
use super::lang;
use super::editor;

pub struct FunctionCallView<'a> {
    function_call: &'a lang::FunctionCall,
    env: &'a env::ExecutionEnvironment,
    function: Option<&'a lang::Function>
}

impl<'a> FunctionCallView<'a> {
    pub fn new(function_call: &'a lang::FunctionCall, env: &'a env::ExecutionEnvironment) -> Self {
        let function = env.find_function(function_call.function_reference().function_id)
            .map(|func| &**func);
        FunctionCallView { function_call, env, function }
    }

    pub fn color(&self) -> editor::Color {
        if self.function.is_some() {
            editor::BLUE_COLOR
        } else {
            editor::RED_COLOR
        }
    }

    pub fn func_name(&self) -> String {
        match self.function {
            Some(func) => func.name().to_string(),
            None => format!("Error: function ID {} not found", self.function_id())
        }
    }

    fn function_id(&self) -> lang::ID {
        self.function_call.function_reference().function_id
    }
}