use std::cell::RefCell;
use std::rc::Rc;

use cs::builtins;
use cs::code_loading;
use cs::env;
use cs::env::ExecutionEnvironment;
use cs::env::Interpreter;
use cs::env_genie;
use cs::init_interpreter;

use super::async_executor;
use super::code_editor::CodeLocation;
use super::code_validation;
use super::editor;
use super::editor::CommandBuffer;
use super::editor::Controller;
use super::save_state;
use super::ui_toolkit::UiToolkit;

// TODO: this is a mess, but not as bad as it was before (the part about the builtins)
fn init_controller(_interpreter: &env::Interpreter) -> Controller {
    let builtins = builtins::Builtins::load().unwrap();

    // this is commented out because we don't need to save builtins right now. uncomment it when we
    // need to save new builtins
    //_save_builtins(&env).unwrap();
    Controller::new(builtins)
}

pub struct App {
    pub interpreter: Interpreter,
    command_buffer: Rc<RefCell<editor::CommandBuffer>>,
    controller: Controller,
}

impl App {
    pub fn new() -> Self {
        let interpreter = init_interpreter();
        let mut command_buffer = editor::CommandBuffer::new();
        // let controller = init_controller(&interpreter);

        // this is usually commented out
        let mut controller = init_controller(&interpreter);
        _load_saved_code_from_disk(&mut controller, &mut interpreter.env.borrow_mut());

        init_save_state(&mut command_buffer, &mut interpreter.env.borrow_mut());

        let command_buffer = Rc::new(RefCell::new(command_buffer));
        Self { interpreter,
               command_buffer,
               controller }
    }

    pub fn new_rc() -> Rc<RefCell<App>> {
        Rc::new(RefCell::new(Self::new()))
    }

    pub fn draw<T: UiToolkit>(&mut self, ui_toolkit: &mut T) -> T::DrawResult {
        let command_buffer = Rc::clone(&self.command_buffer);
        let env = self.interpreter.env();
        let env = env.borrow();
        let env_genie = env_genie::EnvGenie::new(&env);
        let renderer = editor::Renderer::new(ui_toolkit,
                                             &self.controller,
                                             Rc::clone(&command_buffer),
                                             &env_genie);
        renderer.render_app()
    }

    pub fn flush_commands(&mut self, mut async_executor: &mut async_executor::AsyncExecutor) {
        let mut command_buffer = self.command_buffer.borrow_mut();
        while command_buffer.has_queued_commands() {
            //println!("some queued commands, flushing");
            command_buffer.flush_to_controller(&mut self.controller);
            command_buffer.flush_to_interpreter(&mut self.interpreter);
            command_buffer.flush_integrating(&mut self.controller,
                                             &mut self.interpreter,
                                             &mut async_executor);
            code_validation::validate_and_fix(&mut self.interpreter.env().borrow_mut(),
                                              &self.controller,
                                              &mut command_buffer);
        }
    }
}

fn init_save_state(command_buffer: &mut CommandBuffer, env: &mut env::ExecutionEnvironment) {
    let loaded_state = save_state::load();
    let env_genie = env_genie::EnvGenie::new(env);
    for code_location in loaded_state.open_code_editors.iter() {
        match code_location {
            CodeLocation::Function(id) => {
                env_genie.get_code_func(*id)
                         .map(|code_func| command_buffer.load_code_func(code_func.clone()));
            }

            CodeLocation::Script(_id) => {
                // lazy, no support for scripts yet
            }
            CodeLocation::Test(_id) => {
                // lazy, no support for tests yet
            }
            CodeLocation::JSONHTTPClientURLParams(id)
            | CodeLocation::JSONHTTPClientURL(id)
            | CodeLocation::JSONHTTPClientTestSection(id)
            | CodeLocation::JSONHTTPClientTransform(id) => {
                env_genie.get_json_http_client(*id)
                         .map(|client| command_buffer.load_json_http_client(client.clone()));
            }
            CodeLocation::ChatProgram(id) => {
                env_genie
                    .get_chat_program(*id)
                    .map(|chat_program| command_buffer.load_chat_program(chat_program.clone()));
            }
        }
    }

    let window_positions = loaded_state.window_positions;
    command_buffer.add_controller_command(move |controller| {
                      controller.load_serialized_window_positions(window_positions);
                  })
}

pub fn _load_saved_code_from_disk(controller: &mut Controller, env: &mut ExecutionEnvironment) {
    let codestring = include_str!("../../codesample.json");
    let the_world: code_loading::TheWorld = code_loading::deserialize(codestring).unwrap();
    for script in the_world.scripts {
        controller.load_script(script)
    }
    for test in the_world.tests {
        controller.load_test(test);
    }

    // TODO: this is duped in irctest.rs
    for function in the_world.functions {
        env.add_function_box(function);
    }
    for typespec in the_world.typespecs {
        env.add_typespec_box(typespec);
    }
}
