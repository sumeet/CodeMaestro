use std::cell::RefCell;
//use debug_cell::RefCell;
use std::collections::HashMap;
use std::iter;
use std::rc::Rc;

use http;
use itertools::Itertools;

use super::async_executor;
use super::code_editor;
use super::code_editor_renderer::CodeEditorRenderer;
use super::code_generation;
use super::edit_types;
use super::json2;
use super::json_http_client_builder::JSONHTTPClientBuilder;
use super::save_state;
use super::ui_toolkit::{SelectableItem, UiToolkit};
use super::window_positions::{
    WindowPositions, CHAT_TEST_WINDOW_ID, QUICK_START_GUIDE_WINDOW_ID, THEME_EDITOR_WINDOW_ID,
};
use crate::chat::example_chat_program;
use crate::chat_test_window::ChatTestWindow;
use crate::colorscheme;
use crate::draw_all_iter;
use crate::opener::MenuItem;
use crate::opener::Opener;
use crate::send_to_server_overlay::{SendToServerOverlay, SendToServerOverlayStatus};
use crate::theme_editor_renderer::ThemeEditorRenderer;
use crate::ui_toolkit::DrawFnRef;
use crate::window_positions::Window;
use cs::builtins;
use cs::chat_program::{flush_reply_buffer, message_received, ChatProgram};
use cs::code_function;
use cs::code_loading;
use cs::code_loading::TheWorld;
use cs::config;
use cs::enums;
use cs::env;
use cs::env::Interpreter;
use cs::env_genie;
use cs::external_func;
use cs::function;
use cs::http_client;
use cs::json_http_client::JSONHTTPClient;
use cs::jsstuff;
use cs::lang;
use cs::lang::{CodeNode, Function, Value, ID};
use cs::pystuff;
use cs::scripts;
use cs::structs;
use cs::tests;
use cs::{await_eval_result, EnvGenie};

#[derive(Debug, Copy, Clone)]
pub struct Keypress {
    pub key: Key,
    pub ctrl: bool,
    pub shift: bool,
}

impl Keypress {
    pub fn new(key: Key, ctrl: bool, shift: bool) -> Keypress {
        Keypress { key, ctrl, shift }
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum Key {
    A,
    B,
    C,
    D,
    H,
    J,
    K,
    L,
    W,
    X,
    R,
    O,
    U,
    V,
    Tab,
    Escape,
    UpArrow,
    DownArrow,
    LeftArrow,
    RightArrow,
}

#[derive(Debug)]
pub struct TestResult {
    value: Value,
}

impl TestResult {
    pub fn new(value: Value) -> Self {
        Self { value }
    }
}

pub struct Controller {
    // these actually would need to get persisted to the filesystem
    script_by_id: HashMap<ID, scripts::Script>,
    test_by_id: HashMap<ID, tests::Test>,
    // this is purely ephemeral GUI state for display only:
    selected_test_id_by_subject: HashMap<tests::TestSubject, ID>,
    code_editor_by_id: HashMap<ID, code_editor::CodeEditor>,
    test_result_by_func_id: HashMap<ID, TestResult>,
    json_client_builder_by_func_id: HashMap<ID, JSONHTTPClientBuilder>,
    // a record of the builtins kept here so we know which things are builtins
    // and which things aren't. we don't want to let people modify builtins
    builtins: builtins::Builtins,

    pub opener: Option<Opener>,
    window_positions: WindowPositions,
    pub send_to_server_overlay: Rc<RefCell<SendToServerOverlay>>,
    chat_test_window: Rc<RefCell<ChatTestWindow>>,
}

impl<'a> Controller {
    pub fn new(builtins: builtins::Builtins) -> Controller {
        Controller { test_result_by_func_id: HashMap::new(),
                     code_editor_by_id: HashMap::new(),
                     script_by_id: HashMap::new(),
                     test_by_id: HashMap::new(),
                     selected_test_id_by_subject: HashMap::new(),
                     json_client_builder_by_func_id: HashMap::new(),
                     builtins,
                     opener: None,
                     window_positions: WindowPositions::default(),
                     send_to_server_overlay: Rc::new(RefCell::new(SendToServerOverlay::new())),
                     chat_test_window: Rc::new(RefCell::new(ChatTestWindow::new())) }
    }

    pub fn load_serialized_window_positions(&mut self, window_positions: WindowPositions) {
        self.window_positions = window_positions;
    }

    pub fn set_window_position(&mut self,
                               window_id: lang::ID,
                               pos: (isize, isize),
                               size: (usize, usize)) {
        use stdweb::console;
        console!(log, format!("setting position to {:?}", pos));
        console!(log, format!("setting size to {:?}", size));
        self.window_positions.set_window(window_id, pos, size);
        self.save_state();
    }

    pub fn open_window(&mut self, id: lang::ID) {
        self.window_positions.add_window(id);
        self.save_state();
    }

    pub fn close_window(&mut self, id: lang::ID) {
        self.window_positions.close_window(id);
        self.save_state();
    }

    fn save_state(&self) {
        let open_code_editors = self.code_editor_by_id
                                    .values()
                                    .map(|editor| editor.location)
                                    .collect_vec();
        save_state::save(&self.window_positions, &open_code_editors)
    }

    pub fn handle_global_keypress(&mut self, keypress: Keypress) {
        match keypress {
            Keypress { key: Key::O,
                       ctrl: true,
                       shift: false, } => self.open_opener(),
            _ => (),
        }
    }

    pub fn is_builtin(&self, id: lang::ID) -> bool {
        self.builtins.is_builtin(id)
    }

    pub fn open_opener(&mut self) {
        self.opener = Some(Opener::new());
    }

    pub fn close_opener(&mut self) {
        self.opener = None;
    }

    pub fn set_opener_input(&mut self, input_str: String) {
        self.opener
            .as_mut()
            .map(move |opener| opener.set_input_str(input_str));
    }

    pub fn opener_select_next(&mut self) {
        self.opener.as_mut().map(move |opener| opener.select_next());
    }

    pub fn opener_select_prev(&mut self) {
        self.opener.as_mut().map(move |opener| opener.select_prev());
    }

    pub fn list_tests(&self, subject: tests::TestSubject) -> impl Iterator<Item = &tests::Test> {
        self.test_by_id
            .values()
            .filter(move |t| t.subject == subject)
    }

    pub fn list_json_http_client_builders(
        &self)
        -> impl Iterator<Item = (&JSONHTTPClientBuilder, Window)> {
        self.window_positions
            .get_open_windows(self.json_client_builder_by_func_id.keys())
            .map(move |window| {
                (self.json_client_builder_by_func_id.get(&window.id).unwrap(), window)
            })
    }

    pub fn get_test(&self, test_id: lang::ID) -> Option<&tests::Test> {
        self.test_by_id.get(&test_id)
    }

    pub fn get_editor_mut(&mut self, id: lang::ID) -> Option<&mut code_editor::CodeEditor> {
        self.code_editor_by_id.get_mut(&id)
    }

    pub fn get_editor(&self, id: lang::ID) -> Option<&code_editor::CodeEditor> {
        self.code_editor_by_id.get(&id)
    }

    fn get_test_result(&self, func: &dyn lang::Function) -> String {
        let test_result = self.test_result_by_func_id.get(&func.id());
        if let Some(test_result) = test_result {
            format!("{:?}", test_result.value)
        } else {
            "Test not run yet".to_string()
        }
    }

    pub fn load_test(&mut self, test: tests::Test) {
        self.load_code(test.code(), code_editor::CodeLocation::Test(test.id));
        self.test_by_id.insert(test.id, test);
    }

    pub fn load_script(&mut self, script: scripts::Script) {
        let id = script.id();
        self.load_code(script.code(), code_editor::CodeLocation::Script(id));
        self.script_by_id.insert(id, script);
    }

    pub fn load_code(&mut self, code_node: CodeNode, location: code_editor::CodeLocation) {
        let id = code_node.id();
        if !self.code_editor_by_id.contains_key(&id) {
            self.code_editor_by_id
                .insert(id, code_editor::CodeEditor::new(code_node, location));
        } else {
            let code_editor = self.code_editor_by_id.get_mut(&id).unwrap();
            code_editor.replace_code(code_node);
        }
    }

    pub fn load_json_http_client_builder(&mut self, builder: JSONHTTPClientBuilder) {
        self.open_window(builder.json_http_client_id);
        self.json_client_builder_by_func_id
            .insert(builder.json_http_client_id, builder);
    }

    pub fn get_json_http_client_builder(&self, id: lang::ID) -> Option<&JSONHTTPClientBuilder> {
        self.json_client_builder_by_func_id.get(&id)
    }

    pub fn remove_json_http_client_builder(&mut self, id: lang::ID) {
        self.json_client_builder_by_func_id.remove(&id).unwrap();
    }

    // test section stuff
    fn selected_test_id(&self, subject: tests::TestSubject) -> Option<ID> {
        self.selected_test_id_by_subject.get(&subject).cloned()
    }

    fn mark_test_selected(&mut self, test_subject: tests::TestSubject, test_id: ID) {
        self.selected_test_id_by_subject
            .insert(test_subject, test_id);
    }
    // end of test section stuff

    //    // should run the loaded code node
    //    pub fn run(&mut self, _code_node: &CodeNode) {
    //        // TODO: ugh this doesn't work
    //    }
}

// TODO: to simplify things for now, this thing just holds onto closures and
// applies them onto the controller. in the future we could save the actual
// contents into an enum and match on it.... and change things other than the
// controller. for now this is just easier to move us forward
pub struct CommandBuffer {
    // this is kind of messy, but i just need this to get saving to work
    integrating_commands: Vec<Box<dyn FnOnce(&mut Controller,
                                             &mut env::Interpreter,
                                             &mut async_executor::AsyncExecutor,
                                             &mut Self)>>,
    controller_commands: Vec<Box<dyn FnOnce(&mut Controller)>>,
    interpreter_commands: Vec<Box<dyn FnOnce(&mut env::Interpreter)>>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self { integrating_commands: vec![],
               controller_commands: vec![],
               interpreter_commands: vec![] }
    }

    pub fn has_queued_commands(&self) -> bool {
        !self.integrating_commands.is_empty()
        || !self.controller_commands.is_empty()
        || !self.interpreter_commands.is_empty()
    }

    pub fn save_to_net(&mut self) {
        self.add_integrating_command(move |controller, interpreter, async_executor, _| {
                let theworld = save_world(controller, &interpreter.env().borrow());
                let overlay = Rc::clone(&controller.send_to_server_overlay);

                async_executor.exec(async move {
                                  overlay.borrow_mut().mark_as_submitting();
                                  let resp = postthecode(&theworld).await;
                                  match resp {
                                      Err(e) => overlay.borrow_mut()
                                                       .mark_error(e.description().to_owned()),
                                      Ok(resp) => {
                                          let status = resp.status();
                                          if status == 200 {
                                              overlay.borrow_mut().mark_as_success();
                                          } else {
                                              overlay.borrow_mut()
                                       .mark_error(format!("Invalid status code: {}", status));
                                          }
                                      }
                                  }
                                  Ok::<(), ()>(())
                              })
            })
    }

    #[allow(unused)] // unused in wasm
    pub fn save(&mut self) {
        self.add_integrating_command(move |controller, interpreter, _, _| {
                let theworld = save_world(controller, &interpreter.env().borrow());
                code_loading::save("codesample.json", &theworld).unwrap();
            })
    }

    pub fn load_code_func(&mut self, code_func: code_function::CodeFunction) {
        self.add_integrating_command(move |controller, interpreter, _, _| {
                let mut env = interpreter.env.borrow_mut();
                let code = code_func.code();
                let func_id = code_func.id();
                env.add_function(code_func);
                controller.open_window(func_id);
                controller.load_code(code, code_editor::CodeLocation::Function(func_id));
            })
    }

    pub fn change_chat_program(&mut self,
                               chat_program_id: lang::ID,
                               change: impl Fn(&mut ChatProgram) + 'static) {
        self.add_integrating_command(move |_controller, interpreter, _, _| {
                let mut env = interpreter.env.borrow_mut();
                let env_genie = env_genie::EnvGenie::new(&env);
                let mut chat_program = env_genie.get_chat_program(chat_program_id).unwrap().clone();
                change(&mut chat_program);
                env.add_function(chat_program);
            })
    }

    pub fn load_chat_program(&mut self, chat_program: ChatProgram) {
        self.add_integrating_command(move |controller, interpreter, _, _| {
                let mut env = interpreter.env.borrow_mut();
                controller.load_code(lang::CodeNode::Block(chat_program.code.clone()),
                                     code_editor::CodeLocation::ChatProgram(chat_program.id()));
                // TODO: move some of this impl into opener to cut down on code dupe???
                controller.open_window(chat_program.id());
                env.add_function(chat_program);
            })
    }

    pub fn load_json_http_client(&mut self, json_http_client: JSONHTTPClient) {
        self.add_integrating_command(move |controller, interpreter, _, _| {
                let mut env = interpreter.env.borrow_mut();
                controller
                .load_json_http_client_builder(JSONHTTPClientBuilder::new(json_http_client.id()));
                let generate_url_params_code =
                    lang::CodeNode::Block(json_http_client.gen_url_params.clone());
                controller.load_code(
                generate_url_params_code,
                code_editor::CodeLocation::JSONHTTPClientURLParams(json_http_client.id()),
            );
                let gen_url_code = lang::CodeNode::Block(json_http_client.gen_url.clone());
                controller.load_code(
                gen_url_code,
                code_editor::CodeLocation::JSONHTTPClientURL(json_http_client.id()),
            );
                env.add_function(json_http_client);
            })
    }

    pub fn remove_function(&mut self, id: lang::ID) {
        self.add_integrating_command(move |controller, interpreter, _, _| {
                let mut env = interpreter.env.borrow_mut();

                let func = env.find_function(id).unwrap();
                if let Some(json_http_client) = func.downcast_ref::<JSONHTTPClient>() {
                    controller.remove_json_http_client_builder(json_http_client.id())
                }
                env.delete_function(id)
            });
    }

    pub fn load_function(&mut self, func: impl lang::Function + 'static) {
        self.add_environment_command(move |env| env.add_function(func))
    }

    pub fn load_typespec(&mut self, ts: impl lang::TypeSpec + 'static) {
        let ts_id = ts.id();
        self.add_controller_command(move |controller| controller.open_window(ts_id));
        self.add_environment_command(move |env| env.add_typespec(ts))
    }

    // environment actions
    pub fn run(&mut self, code: &lang::CodeNode, callback: impl FnOnce(lang::Value) + 'static) {
        let code = code.clone();
        self.add_integrating_command(move |_controller, interpreter, async_executor, _| {
                run(interpreter, async_executor, &code, callback);
            })
    }

    pub fn add_integrating_command<F: FnOnce(&mut Controller,
                                                  &mut env::Interpreter,
                                                  &mut async_executor::AsyncExecutor,
                                                  &mut Self)
                                           + 'static>(
        &mut self,
        f: F) {
        self.integrating_commands.push(Box::new(f));
    }

    pub fn add_controller_command<F: FnOnce(&mut Controller) + 'static>(&mut self, f: F) {
        self.controller_commands.push(Box::new(f));
    }

    pub fn flush_to_controller(&mut self, controller: &mut Controller) {
        for command in self.controller_commands.drain(..) {
            command(controller)
        }
    }

    pub fn add_interpreter_command<F: FnOnce(&mut env::Interpreter) + 'static>(&mut self, f: F) {
        self.interpreter_commands.push(Box::new(f));
    }

    pub fn add_environment_command<F: FnOnce(&mut env::ExecutionEnvironment) + 'static>(&mut self,
                                                                                        f: F) {
        self.add_interpreter_command(|interpreter| f(&mut interpreter.env().borrow_mut()))
    }

    pub fn flush_to_interpreter(&mut self, interpreter: &mut env::Interpreter) {
        for command in self.interpreter_commands.drain(..) {
            command(interpreter)
        }
    }

    pub fn flush_integrating(&mut self,
                             controller: &mut Controller,
                             interpreter: &mut env::Interpreter,
                             async_executor: &mut async_executor::AsyncExecutor) {
        while let Some(command) = self.integrating_commands.pop() {
            command(controller, interpreter, async_executor, self)
        }
    }
}

async fn postthecode(theworld: &TheWorld)
                     -> Result<http::Response<String>, Box<dyn std::error::Error>> {
    let postcodetoken = config::get_or_err("SERVER_POST_TOKEN")?;
    let post_url = config::post_code_url(postcodetoken)?;
    Ok(http_client::post_json(post_url.as_str(), theworld).await?)
}

pub struct Renderer<'a, T> {
    ui_toolkit: &'a mut T,
    // TODO: take this through the constructor, but now we'll let ppl peek in here
    command_buffer: Rc<RefCell<CommandBuffer>>,
    controller: &'a Controller,
    env_genie: &'a env_genie::EnvGenie<'a>,
}

impl<'a, T: UiToolkit> Renderer<'a, T> {
    pub fn new(ui_toolkit: &'a mut T,
               controller: &'a Controller,
               command_buffer: Rc<RefCell<CommandBuffer>>,
               env_genie: &'a env_genie::EnvGenie)
               -> Renderer<'a, T> {
        Self { ui_toolkit,
               controller,
               command_buffer,
               env_genie }
    }

    pub fn list_open_functions(&self) -> impl Iterator<Item = (&dyn lang::Function, Window)> {
        self.env_genie.all_functions().filter_map(move |func| {
                                          let wp = self.controller
                                                       .window_positions
                                                       .get_open_window(&func.id())?;
                                          Some((func.as_ref(), wp))
                                      })
    }

    pub fn list_open_typespecs(&self) -> impl Iterator<Item = (&dyn lang::TypeSpec, Window)> {
        self.env_genie.typespecs().filter_map(move |ts| {
                                      let wp = self.controller
                                                   .window_positions
                                                   .get_open_window(&ts.id())?;
                                      Some((ts.as_ref(), wp))
                                  })
    }

    pub fn render_app(&self) -> T::DrawResult {
        let cmd_buffer = Rc::clone(&self.command_buffer);
        self.ui_toolkit.handle_global_keypress(move |keypress| {
                           cmd_buffer.borrow_mut()
                                     .add_controller_command(move |controller| {
                                         controller.handle_global_keypress(keypress)
                                     })
                       });
        self.ui_toolkit.draw_all(&[&|| self.render_main_menu_bar(),
                                   &|| self.render_quick_start_guide(),
                                   &|| self.render_colortheme_editor(),
                                   &|| self.render_chat_test_window(),
                                   &|| self.render_scripts(),
                                   //&|| self.render_console_window(),
                                   &|| self.render_edit_code_funcs(),
                                   &|| self.render_edit_pyfuncs(),
                                   &|| self.render_edit_jsfuncs(),
                                   &|| self.render_edit_structs(),
                                   &|| self.render_edit_enums(),
                                   &|| self.render_json_http_client_builders(),
                                   &|| self.render_chat_programs(),
                                   &|| self.render_status_bar(),
                                   &|| self.render_opener(),
                                   &|| self.render_send_to_server_overlay()])
    }

    fn render_colortheme_editor(&self) -> T::DrawResult {
        let open_window = self.controller
                              .window_positions
                              .get_open_window(&*THEME_EDITOR_WINDOW_ID);
        if open_window.is_none() {
            return self.ui_toolkit.draw_all(&[]);
        }
        let open_window = open_window.unwrap();
        self.draw_managed_window(&open_window,
                                 "Theme editor",
                                 &|| ThemeEditorRenderer::new(self.ui_toolkit).render(),
                                 None::<fn(Keypress)>)
    }

    fn render_send_to_server_overlay(&self) -> T::DrawResult {
        self.ui_toolkit.draw_top_right_overlay(&|| {
                           let cmd_buffer = Rc::clone(&self.command_buffer);

                           self.ui_toolkit.draw_all(&[
                &move || {
                    let cmd_buffer = Rc::clone(&cmd_buffer);
                    self.ui_toolkit.draw_button("Upload to \u{f544}",
                                                colorscheme!(action_color),
                                                move || cmd_buffer.borrow_mut().save_to_net())
                },
                &|| match &self.controller.send_to_server_overlay.borrow().status {
                    SendToServerOverlayStatus::Ready => self.ui_toolkit.draw_text("Status: Ready"),
                    SendToServerOverlayStatus::Error(e) => {
                        self.ui_toolkit
                            .draw_text(&format!("Status:  There was an error \n{}", e))
                    }
                    SendToServerOverlayStatus::Submitting => {
                        self.ui_toolkit.draw_all_on_same_line(&[
                            &|| {
                                self.ui_toolkit
                                    .draw_text("Status: Uploading to \u{f544}...")
                            },
                            &|| self.ui_toolkit.draw_spinner(),
                        ])
                    }
                    SendToServerOverlayStatus::Success => {
                        self.ui_toolkit.draw_text("Status:  Sent to server")
                    }
                },
            ])
                       })
    }

    fn render_main_menu_bar(&self) -> T::DrawResult {
        self.ui_toolkit.draw_main_menu_bar(&|| {
                           self.ui_toolkit.draw_menu("File", &|| {
                                              self.ui_toolkit.draw_all(&[
                    #[cfg(not(target_arch = "wasm32"))]
                    &|| {
                        let cmd_buffer = Rc::clone(&self.command_buffer);
                        self.ui_toolkit.draw_menu_item("Save", move || {
                                           cmd_buffer.borrow_mut().save();
                                       })
                    },
                    &|| {
                        let cmd_buffer = Rc::clone(&self.command_buffer);
                        self.ui_toolkit
                            .draw_menu_item("Add new chat program", move || {
                                cmd_buffer.borrow_mut()
                                          .load_chat_program(example_chat_program());
                            })
                    },
                    &|| {
                        let cmd_buffer = Rc::clone(&self.command_buffer);
                        self.ui_toolkit
                            .draw_menu_item("Add new JSON HTTP client", move || {
                                cmd_buffer.borrow_mut()
                                          .load_json_http_client(JSONHTTPClient::new());
                            })
                    },
                    // disable scripts for now while we sprint to chat bot functionality
                    //                        self.ui_toolkit.draw_menu_item("Add new script", move || {
                    //                            cmdb6.borrow_mut().add_controller_command(|controller| {
                    //                                controller.load_script(scripts::Script::new());
                    //                            });
                    //                        }),
                    &|| {
                        let cmd_buffer = Rc::clone(&self.command_buffer);
                        self.ui_toolkit.draw_menu_item("Add new function", move || {
                                           cmd_buffer
                                .borrow_mut()
                                .load_code_func(code_function::CodeFunction::new());
                                       })
                    },
                    &|| {
                        let cmd_buffer = Rc::clone(&self.command_buffer);
                        self.ui_toolkit.draw_menu_item("Add Struct", move || {
                                           cmd_buffer.borrow_mut()
                                                     .load_typespec(structs::Struct::new());
                                       })
                    },
                    &|| {
                        let cmd_buffer = Rc::clone(&self.command_buffer);
                        self.ui_toolkit
                            .draw_menu_item("Add Enum", move || {
                                cmd_buffer.borrow_mut().load_typespec(enums::Enum::new());
                            })
                    },
                    #[cfg(not(target_arch = "wasm32"))]
                    &|| {
                        self.ui_toolkit.draw_menu_item("Exit", || {
                                           std::process::exit(0);
                                       })
                    },
                ])
                                          })
                       })
    }

    fn render_quick_start_guide(&self) -> T::DrawResult {
        let open_window = self.controller
                              .window_positions
                              .get_open_window(&*QUICK_START_GUIDE_WINDOW_ID);
        if let Some(open_window) = open_window {
            self.draw_managed_window(&open_window, "Quick start guide", &|| {
                self.ui_toolkit.draw_all(&[
                    &|| self.ui_toolkit.draw_wrapped_text(colorscheme!(text_color), "Hi, my name is \u{f544}. I'm here to serve your chat room, and anyone can add programs to me."),
                    &|| self.ui_toolkit.draw_wrapped_text(colorscheme!(text_color), ""),
                    &|| {
                        let cmd_buffer = Rc::clone(&self.command_buffer);
                        self.ui_toolkit.draw_button("Make a new chat program", colorscheme!(action_color), move || {
                            cmd_buffer.borrow_mut().load_chat_program(example_chat_program());
                        })
                    },
                    &|| self.ui_toolkit.draw_wrapped_text(colorscheme!(text_color), ""),
                    &|| self.ui_toolkit.draw_wrapped_text(colorscheme!(text_color), "Type your command in the chat test area to try your program before deploying it."),
                    &|| self.ui_toolkit.draw_wrapped_text(colorscheme!(text_color), ""),
                    &|| self.ui_toolkit.draw_wrapped_text(colorscheme!(text_color), "When you're done, press the button on the right to upload it so everyone can use it."),
                ])},
                                        None::<fn(Keypress)>,
            )
        } else {
            self.ui_toolkit.draw_all(&[])
        }
    }

    fn render_chat_test_window(&self) -> T::DrawResult {
        let open_window = self.controller
                              .window_positions
                              .get_open_window(&*CHAT_TEST_WINDOW_ID);
        if let Some(open_window) = open_window {
            self.draw_managed_window(&open_window,
                                     "Chat test area",
                                     &|| {
                                         self.ui_toolkit.draw_layout_with_bottom_bar(
                                             &|| self.ui_toolkit.draw_text_box(&self.controller.chat_test_window.borrow().view()),
                                             &|| {
                                                 let cmd_buffer = Rc::clone(&self.command_buffer);
                                                 self.ui_toolkit.draw_whole_line_console_text_input(move |entered_text| {
                                                     if entered_text.is_empty() {
                                                         return
                                                     }

                                                     let entered_text = entered_text.to_string();
                                                     let mut cmd_buffer = cmd_buffer.borrow_mut();
                                                     cmd_buffer.add_integrating_command(move |cont, interp, async_executor, _cmd_buffer| {
                                                        cont.chat_test_window.borrow_mut().add_message("\u{f406}".to_string(), entered_text.clone()) ;

                                                         let interp = interp.dup();
                                                         let chat_test_window = Rc::clone(&cont.chat_test_window);
                                                         async_executor.exec(async move {
                                                             message_received(&interp, "\u{f406}".to_string(), entered_text).await;

                                                             // TODO: probably not the best place for this, but it'll work. if there's any unflushed
                                                             // bot output , stick it in the chat test window
                                                             let env = interp.env.borrow();
                                                             let env_genie = EnvGenie::new(&env);
                                                             let mut chat_test_window = chat_test_window.borrow_mut();
                                                             for reply in flush_reply_buffer(&env_genie) {
                                                                 chat_test_window.add_message("\u{f544}".to_string(), reply);
                                                             }

                                                             let ok: Result<(), ()> = Ok(());
                                                             ok
                                                         });
                                                     });
                                                 })
                                             }
                                         )
                                     },
                                     None::<fn(Keypress)>)
        } else {
            self.ui_toolkit.draw_all(&[])
        }
    }

    fn render_opener(&self) -> T::DrawResult {
        if self.controller.opener.is_none() {
            return self.ui_toolkit.draw_all(&[]);
        }

        let cmd_buffer4 = Rc::clone(&self.command_buffer);

        let opener = self.controller.opener.as_ref().unwrap();
        self.ui_toolkit.draw_centered_popup(
                                            &|| {
                                                self.ui_toolkit.draw_all(&[
                &|| {
                    self.ui_toolkit.focused(&|| {
                                       let cmd_buffer1 = Rc::clone(&self.command_buffer);
                                       let cmd_buffer2 = Rc::clone(&self.command_buffer);

                                       self.ui_toolkit.draw_text_input(
                            &opener.input_str,
                            move |newvalue: &str| {
                                let newvalue = newvalue.to_string();
                                cmd_buffer1
                                    .borrow_mut()
                                    .add_controller_command(move |controller| {
                                        controller.set_opener_input(newvalue.to_string())
                                    })
                            },
                            move || {
                                cmd_buffer2.borrow_mut().add_integrating_command(
                                    move |controller, interp, _, cmd_buffer| {
                                        if let Some(opener) = &controller.opener {
                                            let env = interp.env.borrow();
                                            let env_genie = env_genie::EnvGenie::new(&env);
                                            let lister =
                                                opener.list_options(controller, &env_genie);
                                            lister.selected_option().map(move |menu_item| {
                                                if let MenuItem::Selectable {
                                                    when_selected, ..
                                                } = menu_item
                                                {
                                                    cmd_buffer.add_controller_command(
                                                        |controller| {
                                                            controller.close_opener();
                                                        },
                                                    );
                                                    when_selected(cmd_buffer)
                                                }
                                            });
                                        }
                                    },
                                )
                            },
                        )
                                   })
                },
                &move || {
                    let options_lister = opener.list_options(self.controller, self.env_genie);
                    let menu_items = options_lister.list().collect_vec();

                    let mut selectable_items = vec![];
                    for menu_item in menu_items {
                        selectable_items.push(match menu_item {
                                                  MenuItem::Heading(heading) => {
                                                      SelectableItem::GroupHeader(heading)
                                                  }
                                                  MenuItem::Selectable { ref label,
                                                                         is_selected,
                                                                         .. } => {
                                                      let label = label.clone();
                                                      SelectableItem::Selectable { item:
                                                                                       menu_item,
                                                                                   label,
                                                                                   is_selected }
                                                  }
                                              });
                    }
                    let cmd_buffer3 = Rc::clone(&self.command_buffer);
                    self.ui_toolkit
                        .draw_selectables2(selectable_items, move |menu_item| {
                            if let MenuItem::Selectable { when_selected, .. } = menu_item {
                                cmd_buffer3.borrow_mut()
                                           .add_controller_command(|controller| {
                                               controller.close_opener();
                                           });
                                when_selected(&mut cmd_buffer3.borrow_mut())
                            }
                        })
                },
            ])
                                            },
                                            Some(move |keypress| {
                                                cmd_buffer4
                    .borrow_mut()
                    .add_controller_command(move |controller| match keypress {
                        Keypress { key: Key::DownArrow, .. } |
                        Keypress {
                            key: Key::Tab,
                            ctrl: false,
                            shift: false,
                        } => controller.opener_select_next(),
                        Keypress { key: Key::UpArrow, .. } |
                        Keypress {
                            key: Key::Tab,
                            ctrl: false,
                            shift: true,
                        } => controller.opener_select_prev(),
                        Keypress {
                            key: Key::Escape, ..
                        } => controller.close_opener(),
                        _ => (),
                    })
                                            }),
        )
    }

    fn render_edit_code_funcs(&self) -> T::DrawResult {
        let draw_fns = self.list_open_functions()
                           .filter_map(|(func, window)| {
                               Some((func.downcast_ref::<code_function::CodeFunction>()?, window))
                           })
                           .map(move |(f, window)| move || self.render_edit_code_func(f, &window));
        draw_all_iter!(T::self.ui_toolkit, draw_fns)
    }

    fn render_scripts(&self) -> T::DrawResult {
        let open_scripts =
            self.controller
                .window_positions
                .get_open_windows(self.controller.script_by_id.keys())
                .map(move |window| (self.controller.script_by_id.get(&window.id).unwrap(), window));

        draw_all_iter!(T::self.ui_toolkit,
                       open_scripts.map(|(script, window)| move || self.render_script(script,
                                                                                      &window)))
    }

    fn render_script(&self, script: &scripts::Script, window: &Window) -> T::DrawResult {
        self.draw_managed_window(
                                 window,
                                 &format!("Script {}", script.id()),
                                 &|| {
                                     self.ui_toolkit
                    .draw_layout_with_bottom_bar(&|| self.render_code(script.id()), &|| {
                        self.render_run_button(script.code())
                    })
                                 },
                                 None::<fn(Keypress)>,
        )
    }

    fn render_run_button(&self, code_node: CodeNode) -> T::DrawResult {
        let cmd_buffer = self.command_buffer.clone();
        self.ui_toolkit
            .draw_button("Run", colorscheme!(action_color), move || {
                let mut cmd_buffer = cmd_buffer.borrow_mut();
                cmd_buffer.run(&code_node, |_val| {
                              //println!("{:?}", val)
                          });
            })
    }

    fn render_edit_code_func(&self,
                             code_func: &code_function::CodeFunction,
                             window: &Window)
                             -> T::DrawResult {
        self.draw_managed_window(
                                 window,
                                 &format!("Edit function: {}", code_func.id()),
                                 &|| {
                                     self.ui_toolkit.draw_all(&[
                &|| {
                    let cont1 = Rc::clone(&self.command_buffer);
                    let code_func1 = code_func.clone();
                    self.ui_toolkit.draw_text_input_with_label("Function name",
                                                               code_func.name(),
                                                               move |newvalue| {
                                                                   let mut code_func1 =
                                                                       code_func1.clone();
                                                                   code_func1.name =
                                                                       newvalue.to_string();
                                                                   cont1.borrow_mut()
                                                                        .load_function(code_func1);
                                                               },
                                                               || {})
                },
                &|| self.render_arguments_selector(code_func),
                &|| self.render_code(code_func.code().id()),
                &|| self.render_return_type_selector(code_func),
                &|| self.ui_toolkit.draw_separator(),
                &|| self.render_general_function_menu(code_func),
            ])
                                 },
                                 None::<fn(Keypress)>,
        )
    }

    fn render_edit_pyfuncs(&self) -> T::DrawResult {
        let pyfuncs =
            self.list_open_functions().filter_map(|(func, window)| {
                                          Some((func.downcast_ref::<pystuff::PyFunc>()?, window))
                                      });
        draw_all_iter!(T::self.ui_toolkit,
                       pyfuncs.map(|(f, window)| move || self.render_edit_pyfunc(f, &window)))
    }

    fn render_edit_pyfunc(&self, pyfunc: &pystuff::PyFunc, window: &Window) -> T::DrawResult {
        self.draw_managed_window(
                                 window,
                                 &format!("Edit PyFunc: {}", pyfunc.id),
                                 &|| {
                                     self.ui_toolkit.draw_all(&[
                &|| {
                    let cont1 = Rc::clone(&self.command_buffer);
                    let pyfunc1 = pyfunc.clone();
                    self.ui_toolkit.draw_text_input_with_label("Function name",
                                                               pyfunc.name(),
                                                               move |newvalue| {
                                                                   let mut pyfunc1 =
                                                                       pyfunc1.clone();
                                                                   pyfunc1.name =
                                                                       newvalue.to_string();
                                                                   cont1.borrow_mut()
                                                                        .load_function(pyfunc1);
                                                               },
                                                               || {})
                },
                &|| self.render_arguments_selector(pyfunc),
                &|| {
                    let cont2 = Rc::clone(&self.command_buffer);
                    let pyfunc2 = pyfunc.clone();
                    self.ui_toolkit.draw_multiline_text_input_with_label(
                                                                         // TODO: add help text here
                                                                         "Prelude",
                                                                         &pyfunc.prelude,
                                                                         move |newvalue| {
                                                                             let mut pyfunc2 =
                                                                                 pyfunc2.clone();
                                                                             pyfunc2.prelude =
                                                                      newvalue.to_string();
                                                                             cont2.borrow_mut()
                                                                       .load_function(pyfunc2);
                                                                         },
                    )
                },
                &|| {
                    let cont3 = Rc::clone(&self.command_buffer);
                    let pyfunc3 = pyfunc.clone();
                    self.ui_toolkit.draw_multiline_text_input_with_label(
                            "Code",
                            &pyfunc.eval,
                            move |newvalue| {
                                let mut pyfunc3 = pyfunc3.clone();
                                pyfunc3.eval = newvalue.to_string();
                                cont3.borrow_mut().load_function(pyfunc3);
                            },
                        )
                },
                &|| self.render_return_type_selector(pyfunc),
                &|| self.ui_toolkit.draw_separator(),
                &|| self.render_old_test_section(pyfunc.clone()),
                &|| self.ui_toolkit.draw_separator(),
                &|| self.render_general_function_menu(pyfunc),
            ])
                                 },
                                 None::<fn(Keypress)>,
        )
    }

    fn render_edit_jsfuncs(&self) -> T::DrawResult {
        let jsfuncs =
            self.list_open_functions().filter_map(|(func, window)| {
                                          Some((func.downcast_ref::<jsstuff::JSFunc>()?, window))
                                      });
        draw_all_iter!(T::self.ui_toolkit,
                       jsfuncs.map(|(f, window)| move || self.render_edit_jsfunc(f, &window)))
    }

    fn render_edit_jsfunc(&self, jsfunc: &jsstuff::JSFunc, window: &Window) -> T::DrawResult {
        self.draw_managed_window(
                                 window,
                                 &format!("Edit JSFunc: {}", jsfunc.id),
                                 &|| {
                                     self.ui_toolkit.draw_all(&[
                &|| {
                    let cont1 = Rc::clone(&self.command_buffer);
                    let jsfunc1 = jsfunc.clone();
                    self.ui_toolkit.draw_text_input_with_label("Function name",
                                                               jsfunc.name(),
                                                               move |newvalue| {
                                                                   let mut jsfunc1 =
                                                                       jsfunc1.clone();
                                                                   jsfunc1.name =
                                                                       newvalue.to_string();
                                                                   cont1.borrow_mut()
                                                                        .load_function(jsfunc1);
                                                               },
                                                               || {})
                },
                &|| self.render_arguments_selector(jsfunc),
                &|| {
                    let cont3 = Rc::clone(&self.command_buffer);
                    let jsfunc3 = jsfunc.clone();
                    self.ui_toolkit.draw_multiline_text_input_with_label(
                            "Code",
                            &jsfunc.eval,
                            move |newvalue| {
                                let mut jsfunc3 = jsfunc3.clone();
                                jsfunc3.eval = newvalue.to_string();
                                cont3.borrow_mut().load_function(jsfunc3);
                            },
                        )
                },
                &|| self.render_return_type_selector(jsfunc),
                &|| self.ui_toolkit.draw_separator(),
                &|| self.render_old_test_section(jsfunc.clone()),
                &|| self.ui_toolkit.draw_separator(),
                &|| self.render_general_function_menu(jsfunc),
            ])
                                 },
                                 None::<fn(Keypress)>,
        )
    }

    fn render_edit_structs(&self) -> T::DrawResult {
        let structs =
            self.list_open_typespecs()
                .filter_map(|(ts, window)| Some((ts.downcast_ref::<structs::Struct>()?, window)));
        draw_all_iter!(T::self.ui_toolkit,
                       structs.map(|(s, window)| move || self.render_edit_struct(s, &window)))
    }

    fn render_edit_enums(&self) -> T::DrawResult {
        let enums =
            self.list_open_typespecs()
                .filter_map(|(ts, window)| Some((ts.downcast_ref::<enums::Enum>()?, window)));
        draw_all_iter!(T::self.ui_toolkit,
                       enums.map(|(e, window)| move || self.render_edit_enum(e, &window)))
    }

    fn render_chat_programs(&self) -> T::DrawResult {
        let programs =
            self.list_open_functions()
                .filter_map(|(func, window)| Some((func.downcast_ref::<ChatProgram>()?, window)));
        draw_all_iter!(T::self.ui_toolkit,
                       programs.map(|(trigger, window)| move || self.render_chat_program(trigger,
                                                                                         &window)))
    }

    // TODO: should window_name go inside of Window?
    fn draw_managed_window(&self,
                           window: &Window,
                           window_name: &str,
                           draw_fn: DrawFnRef<T>,
                           handle_keypress: Option<impl Fn(Keypress) + 'static>)
                           -> T::DrawResult {
        let cmd_buffer = Rc::clone(&self.command_buffer);
        let window_id = window.id;
        self.ui_toolkit.draw_window(window_name,
                                    window.size,
                                    window.pos(),
                                    draw_fn,
                                    handle_keypress,
                                    Some(move || {
                                        cmd_buffer.borrow_mut()
                                                  .add_controller_command(move |controller| {
                                                      controller.close_window(window_id);
                                                  })
                                    }),
                                    onwindowchange(Rc::clone(&self.command_buffer), window.id))
    }

    fn render_chat_program(&self, chat_program: &ChatProgram, window: &Window) -> T::DrawResult {
        let chat_program_id = chat_program.id;
        self.draw_managed_window(
                                 window,
                                 &format!("Edit chat program: {}", chat_program.id()),
                                 &|| {
                                     self.ui_toolkit.draw_all(&[
                &|| {
                    let cmd_buffer2 = Rc::clone(&self.command_buffer);
                    self.ui_toolkit.draw_text_input_with_label(
                                                               "Bot command",
                                                               &chat_program.prefix,
                                                               move |newvalue| {
                                                                   let newvalue =
                                                                       newvalue.to_string();
                                                                   cmd_buffer2
                                    .borrow_mut()
                                    .change_chat_program(chat_program_id, move |mut ct| {
                                        ct.prefix = newvalue.to_string()
                                    })
                                                               },
                                                               &|| {},
                    )
                },
                &|| self.render_code(chat_program.code.id),
            ])
                                 },
                                 None::<fn(Keypress)>,
        )
    }

    fn render_json_http_client_builders(&self) -> T::DrawResult {
        let builders = self.controller.list_json_http_client_builders();
        draw_all_iter!(T::self.ui_toolkit,
                       builders.map(|(builder, window)| move || {
                                   self.render_json_http_client_builder(builder, &window)
                               }))
    }

    fn render_json_http_client_builder(&self,
                                       builder: &JSONHTTPClientBuilder,
                                       window: &Window)
                                       -> T::DrawResult {
        self.draw_managed_window(
                                 window,
                                 &format!("Edit JSON HTTP Client: {}", builder.json_http_client_id),
                                 &|| {
                                     let client =
                                         self.env_genie
                                             .get_json_http_client(builder.json_http_client_id)
                                             .unwrap();
                                     let client_id = client.id();

                                     let current_return_type_name =
                                         self.env_genie
                                             .get_name_for_type(&client.returns())
                                             .unwrap();

                                     self.ui_toolkit.draw_all(&[
                &|| {
                    let cmd_buffer1 = Rc::clone(&self.command_buffer);
                    let client1 = client.clone();
                    self.ui_toolkit.draw_text_input_with_label(
                            "Name",
                            client.name(),
                            move |newvalue| {
                                let mut client = client1.clone();
                                client.name = newvalue.to_string();
                                cmd_buffer1.borrow_mut().load_function(client);
                            },
                            || {},
                        )
                },
                &|| self.ui_toolkit.draw_text("Base URL:"),
                &|| self.render_code(client.gen_url.id),
                &|| self.ui_toolkit.draw_text("URL params:"),
                &|| self.render_code(client.gen_url_params.id),
                &|| self.render_arguments_selector(client),
                &|| {
                    self.ui_toolkit
                        .draw_text(&format!("Current return type: {}", current_return_type_name))
                },
                &|| self.ui_toolkit.draw_separator(),
                &|| {
                    self.ui_toolkit
                        .draw_text("Return type generator and test section")
                },
                &|| {
                    let cmd_buffer3 = Rc::clone(&self.command_buffer);
                    let builder1 = builder.clone();
                    self.ui_toolkit.draw_text_input_with_label(
                                                               "Test URL",
                                                               &builder.test_url,
                                                               move |newvalue| {
                                                                   let builder1 = builder1.clone();
                                                                   let newvalue =
                                                                       newvalue.to_string();
                                                                   cmd_buffer3
                                    .borrow_mut()
                                    .add_controller_command(move |cont| {
                                        let mut builder = builder1.clone();
                                        builder.test_url = newvalue;
                                        cont.load_json_http_client_builder(builder);
                                    })
                                                               },
                                                               || {},
                    )
                },
                &|| {
                    let cmd_buffer4 = Rc::clone(&self.command_buffer);
                    let cmd_buffer5 = Rc::clone(&self.command_buffer);
                    self.ui_toolkit
                        .draw_button("Run test", colorscheme!(action_color), move || {
                            let cmd_buffer5 = Rc::clone(&cmd_buffer5);
                            cmd_buffer4.borrow_mut().add_integrating_command(
                                    move |cont, _interp, async_executor, _| {
                                        let builder =
                                            cont.get_json_http_client_builder(client_id).unwrap();
                                        builder.run_test(async_executor, move |newbuilder| {
                                            cmd_buffer5.borrow_mut().add_controller_command(
                                                move |cont| {
                                                    cont.load_json_http_client_builder(newbuilder);
                                                },
                                            )
                                        })
                                    },
                                )
                        })
                },
                &|| self.render_test_request_results(builder),
                &|| self.render_json_return_type_selector(builder),
                &|| self.ui_toolkit.draw_separator(),
                &|| self.render_general_function_menu(client),
            ])
                                 },
                                 None::<fn(Keypress)>,
        )
    }

    fn render_json_return_type_selector(&self, builder: &JSONHTTPClientBuilder) -> T::DrawResult {
        if let Some(return_type_candidate) = &builder.return_type_candidate {
            let type_description = if let Some(type_name) =
                self.env_genie.get_name_for_type(&return_type_candidate.typ)
            {
                type_name
            } else {
                let new_struct_for_return_type =
                    return_type_candidate.structs_to_be_added
                                         .iter()
                                         .find(|s| s.id == return_type_candidate.typ.typespec_id)
                                         .unwrap();
                new_struct_for_return_type.name.clone()
            };
            let client_id = builder.json_http_client_id;
            self.ui_toolkit.draw_all(&[
                &|| {
                    self.ui_toolkit
                        .draw_text(&format!("Return type: {}", type_description))
                },
                &|| {
                    let cmd_buffer = Rc::clone(&self.command_buffer);
                    self.ui_toolkit.draw_button(
                        "Apply return type",
                        colorscheme!(action_color),
                        move || {
                                                    cmd_buffer.borrow_mut()
                                      .add_integrating_command(move |cont, interp, _, _| {
                                          let mut env = interp.env.borrow_mut();
                                          let mut json_http_client = {
                                              let env_genie = env_genie::EnvGenie::new(&env);
                                              env_genie.get_json_http_client(client_id)
                                                       .unwrap()
                                                       .clone()
                                          };

                                          let builder =
                                              cont.get_json_http_client_builder(client_id).unwrap();
                                          let return_type_candidate =
                                              builder.return_type_candidate.clone().unwrap();
                                          for strukt in return_type_candidate.structs_to_be_added {
                                              env.add_typespec(strukt);
                                          }
                                          json_http_client.return_type = return_type_candidate.typ;
                                          env.add_function(json_http_client);
                                      })
                                                },
                    )
                },
                &|| self.ui_toolkit.draw_text("Structs to be added:"),
                &|| {
                    self.ui_toolkit
                        .draw_text(&format!("{:#?}", return_type_candidate.structs_to_be_added))
                },
            ])
        } else {
            self.ui_toolkit.draw_text("")
        }
    }

    fn render_test_request_results(&self, builder: &JSONHTTPClientBuilder) -> T::DrawResult {
        match builder.test_run_result {
            Some(Ok(_)) => {
                self.render_parsed_doc(builder, builder.test_run_parsed_doc.as_ref().unwrap())
            }
            Some(Err(ref e)) => self.ui_toolkit.draw_text(e),
            None => self.ui_toolkit.draw_all(&[]),
        }
    }

    fn render_parsed_doc(&self,
                         builder: &JSONHTTPClientBuilder,
                         parsed_json: &json2::ParsedDocument)
                         -> T::DrawResult {
        use json2::ParsedDocument::*;
        let nesting = parsed_json.nesting();
        match parsed_json {
            Bool { value, .. } => {
                self.render_parsed_doc_value(builder, &value.to_string(), nesting)
            }
            String { value, .. } => self.render_parsed_doc_value(builder, &value, nesting),
            Number { value, .. } => {
                self.render_parsed_doc_value(builder, &value.to_string(), nesting)
            }
            Null { .. } => self.ui_toolkit.draw_text("Null"),
            List { value, .. } => {
                self.ui_toolkit
                    .draw_all(&[&|| self.ui_toolkit.draw_text("["),
                                &|| {
                                    self.ui_toolkit.indent(10, &|| {
                                                       // TODO: show the rest under a collapsible thingie
                                                       self.render_parsed_doc(builder, &value[0])
                                                   })
                                },
                                &|| self.ui_toolkit.draw_text("]")])
            }
            Map { value: map, .. } => self.ui_toolkit.draw_all(&[
                &|| self.ui_toolkit.draw_text("{"),
                &|| {
                    self.ui_toolkit.indent(10, &|| {
                                       draw_all_iter!(
                                                      T::self.ui_toolkit,
                                                      map.iter().map(|(key, value)| {
                                                          move || {
                                                              self.ui_toolkit.align(
                                        &|| self.ui_toolkit.draw_text(&format!("{}:", key)),
                                        &[&|| self.render_parsed_doc(builder, value)],
                                    )
                                                          }
                                                      })
                        )
                                   })
                },
                &|| self.ui_toolkit.draw_text("}"),
            ]),
            // TODO: maybe one day we'll want to try and put these kinds of things into json::Value
            // containers, and just let you grab at it like in a normal programming language. for
            // now though, just don't let you do anything with them.
            EmptyCantInfer { .. } => self.ui_toolkit.draw_text("Empty list, cannot infer type"),
            NonHomogeneousCantParse { .. } => {
                self.ui_toolkit
                    .draw_text("Non homogeneous list, cannot infer type")
            }
        }
    }

    fn render_parsed_doc_value(&self,
                               builder: &JSONHTTPClientBuilder,
                               value: &str,
                               nesting: &json2::Nesting)
                               -> T::DrawResult {
        let selected_field = builder.get_selected_field(nesting);
        let cmd_buffer = Rc::clone(&self.command_buffer);
        let client_id = builder.json_http_client_id;
        let nesting = nesting.clone();
        self.ui_toolkit.draw_all_on_same_line(&[
            &|| self.ui_toolkit.draw_text(value),
            &move || {
                let cmd_buffer = cmd_buffer.clone();
                let nesting = nesting.clone();
                self.ui_toolkit.draw_checkbox_with_label(
                    "",
                    selected_field.is_some(),
                    move |newvalue| {
                        let nesting = nesting.clone();
                        cmd_buffer.borrow_mut().add_integrating_command(
                            move |cont, interp, _executor, _| {
                                let env = interp.env.borrow();
                                let env_genie = env_genie::EnvGenie::new(&env);
                                let mut builder = cont
                                    .get_json_http_client_builder(client_id)
                                    .unwrap()
                                    .clone();
                                if newvalue {
                                    builder.add_selected_field(nesting, &env_genie)
                                } else {
                                    builder.remove_selected_field(nesting, &env_genie)
                                }
                                cont.load_json_http_client_builder(builder)
                            },
                        )
                    },
                )
            },
            &|| {
                if let Some(selected_field) = selected_field {
                    let ts = self.env_genie
                                 .find_typespec(selected_field.typespec_id)
                                 .unwrap();
                    self.ui_toolkit
                        .draw_text(&format!("{} {}", selected_field.name, ts.readable_name()))
                } else {
                    self.ui_toolkit.draw_text("")
                }
            },
        ])
    }

    fn render_edit_struct(&self, strukt: &structs::Struct, window: &Window) -> T::DrawResult {
        self.draw_managed_window(
                                 window,
                                 &format!("Edit Struct: {}", strukt.id),
                                 &|| {
                                     self.ui_toolkit.draw_all(&[
                &|| {
                    let cont1 = Rc::clone(&self.command_buffer);
                    let strukt1 = strukt.clone();
                    self.ui_toolkit.draw_text_input_with_label("Structure name",
                                                               &strukt.name,
                                                               move |newvalue| {
                                                                   let mut strukt = strukt1.clone();
                                                                   strukt.name =
                                                                       newvalue.to_string();
                                                                   cont1.borrow_mut()
                                                                        .load_typespec(strukt);
                                                               },
                                                               &|| {})
                },
                &|| {
                    let cont2 = Rc::clone(&self.command_buffer);
                    let strukt2 = strukt.clone();
                    self.ui_toolkit.draw_text_input_with_label("Symbol",
                                                               &strukt.symbol,
                                                               move |newvalue| {
                                                                   let mut strukt = strukt2.clone();
                                                                   strukt.symbol =
                                                                       newvalue.to_string();
                                                                   cont2.borrow_mut()
                                                                        .load_typespec(strukt);
                                                               },
                                                               &|| {})
                },
                &|| self.render_struct_fields_selector(strukt),
                &|| self.render_general_struct_menu(strukt),
            ])
                                 },
                                 None::<fn(Keypress)>,
        )
    }

    // TODO: this is super dupe of render_arguments_selector, whatever for now but we'll
    // clean this up
    // TODO: fix this it looks like shit
    fn render_struct_fields_selector(&self, strukt: &structs::Struct) -> T::DrawResult {
        let fields = &strukt.fields;

        self.ui_toolkit.draw_all(&[
            &|| {
                self.ui_toolkit
                    .draw_text_with_label(&format!("Has {} field(s)", fields.len()), "Fields")
            },
            &|| {
                draw_all_iter!(
                               T::self.ui_toolkit,
                               fields.iter()
                                     .enumerate()
                                     .map(|(current_field_index, field)| {
                                         move || {
                                             let strukt1 = strukt.clone();
                                             let cont1 = Rc::clone(&self.command_buffer);
                                             self.ui_toolkit.draw_text_input_with_label(
                          "Name",
                          &field.name,
                          move |newvalue| {
                              let mut newstrukt = strukt1.clone();
                              let mut newfield = &mut newstrukt.fields[current_field_index];
                              newfield.name = newvalue.to_string();
                              cont1.borrow_mut().load_typespec(newstrukt)
                          },
                          &|| {},
                       )
                                         }
                                     })
                )
            },
            &|| {
                let strukt1 = strukt.clone();
                let cont1 = Rc::clone(&self.command_buffer);
                self.ui_toolkit.draw_button(
                    "Add another field",
                    colorscheme!(action_color),
                    move || {
                                                let mut newstrukt = strukt1.clone();
                                                newstrukt.fields.push(structs::StructField::new(
                           format!("field{}", newstrukt.fields.len()),
                           "".into(),
                           lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                       ));
                                                cont1.borrow_mut().load_typespec(newstrukt);
                                            },
                )
            },
        ])
    }

    // TODO: a way to delete the struct :)
    fn render_general_struct_menu(&self, _strukt: &structs::Struct) -> T::DrawResult {
        self.ui_toolkit.draw_all(&[])
    }

    fn render_edit_enum(&self, eneom: &enums::Enum, window: &Window) -> T::DrawResult {
        self.draw_managed_window(
                                 window,
                                 &format!("Edit Enum: {}", eneom.id),
                                 &|| {
                                     self.ui_toolkit.draw_all(&[
                &|| {
                    let cont1 = Rc::clone(&self.command_buffer);
                    let eneom1 = eneom.clone();
                    self.ui_toolkit.draw_text_input_with_label("Enum name",
                                                               &eneom.name,
                                                               move |newvalue| {
                                                                   let mut eneom = eneom1.clone();
                                                                   eneom.name =
                                                                       newvalue.to_string();
                                                                   cont1.borrow_mut()
                                                                        .load_typespec(eneom);
                                                               },
                                                               &|| {})
                },
                &|| {
                    let cont2 = Rc::clone(&self.command_buffer);
                    let eneom2 = eneom.clone();
                    self.ui_toolkit.draw_text_input_with_label("Symbol",
                                                               &eneom.symbol,
                                                               move |newvalue| {
                                                                   let mut eneom = eneom2.clone();
                                                                   eneom.symbol =
                                                                       newvalue.to_string();
                                                                   cont2.borrow_mut()
                                                                        .load_typespec(eneom);
                                                               },
                                                               &|| {})
                },
                &|| self.render_enum_variants_selector(eneom),
                // TODO: why is this commented out? lol
                //                    self.render_general_struct_menu(eneom),
            ])
                                 },
                                 None::<fn(Keypress)>,
        )
    }

    fn render_enum_variants_selector(&self, eneom: &enums::Enum) -> T::DrawResult {
        let variants = &eneom.variants;

        self.ui_toolkit.draw_all(&[
            &|| self
                .ui_toolkit
                .draw_text_with_label(&format!("Has {} variant(s)", variants.len()), "Variants"),
            &|| {
                draw_all_iter!(T::self.ui_toolkit,
                   variants.iter().enumerate().map(|(current_variant_index, variant)| {
                       move || self.ui_toolkit.draw_all(&[
                            &|| {
                                let eneom1 = eneom.clone();
                                let cont1 = Rc::clone(&self.command_buffer);
                                self.ui_toolkit.draw_text_input_with_label(
                                    "Name",
                                    &variant.name,
                                    move |newvalue| {
                                        let mut neweneom = eneom1.clone();
                                        let mut newvariant = &mut neweneom.variants[current_variant_index];
                                        newvariant.name = newvalue.to_string();
                                        cont1.borrow_mut().load_typespec(neweneom)
                                    },
                                    &|| {},
                                )
                            },
                            // TODO: add this checkbox logic to other types?
                            &|| {
                                let eneom1 = eneom.clone();
                                let cont1 = Rc::clone(&self.command_buffer);
                                self.ui_toolkit.draw_checkbox_with_label(
                                    "Parameterized type?",
                                    variant.is_parameterized(),
                                    move |is_parameterized| {
                                        let mut neweneom = eneom1.clone();
                                        let mut newvariant = &mut neweneom.variants[current_variant_index];
                                        if is_parameterized {
                                            newvariant.variant_type = None;
                                        } else {
                                            newvariant.variant_type =
                                                Some(lang::Type::from_spec(&*lang::STRING_TYPESPEC));
                                        }
                                        cont1.borrow_mut().load_typespec(neweneom)
                                    },
                                )
                            },
                            &|| {
                                if !variant.is_parameterized() {
                                    self.ui_toolkit.draw_all(&[])
                                } else {
                                    let eneom1 = eneom.clone();
                                    let cont1 = Rc::clone(&self.command_buffer);
                                    self.render_type_change_combo(
                                        "Type",
                                        variant.variant_type.as_ref().unwrap(),
                                        move |newtype| {
                                            let mut neweneom = eneom1.clone();
                                            let mut newvariant = &mut neweneom.variants[current_variant_index];
                                            newvariant.variant_type = Some(newtype);
                                            cont1.borrow_mut().load_typespec(neweneom)
                                        },
                                    )
                                }
                            },
                            &|| {
                                let eneom1 = eneom.clone();
                                let cont1 = Rc::clone(&self.command_buffer);
                                self.ui_toolkit.draw_button("Delete", colorscheme!(danger_color), move || {
                                    let mut neweneom = eneom1.clone();
                                    neweneom.variants.remove(current_variant_index);
                                    cont1.borrow_mut().load_typespec(neweneom)
                                })
                            },
                            &|| {
                                let eneom1 = eneom.clone();
                                let cont1 = Rc::clone(&self.command_buffer);
                                self.ui_toolkit
                                    .draw_button("Add another variant", colorscheme!(action_color), move || {
                                        let mut neweneom = eneom1.clone();
                                        neweneom.variants.push(enums::EnumVariant::new(
                                            format!("variant{}", neweneom.variants.len()),
                                            None,
                                        ));
                                        cont1.borrow_mut().load_typespec(neweneom);
                                    })
                            }
                       ])
                   })
                )
            }
        ])
    }

    fn render_general_function_menu<F: lang::Function>(&self, func: &F) -> T::DrawResult {
        self.ui_toolkit.draw_all(&[&|| {
                                       let cont1 = Rc::clone(&self.command_buffer);
                                       let func_id = func.id();
                                       self.ui_toolkit
                                           .draw_button("Delete", colorscheme!(danger_color), move || {
                                               cont1.borrow_mut().remove_function(func_id);
                                           })
                                   },
                                   &|| self.render_test_section(func)])
    }

    fn render_test_section<F: lang::Function>(&self, func: &F) -> T::DrawResult {
        let subject = tests::TestSubject::Function(func.id());
        let tests = self.controller.list_tests(subject).collect_vec();
        let selected_test_id = self.controller.selected_test_id(subject);

        self.ui_toolkit.draw_all(&[
            &|| self.ui_toolkit.draw_text("Tests:"),
            &|| {
                let cmd_buffer2 = Rc::clone(&self.command_buffer);
                self.ui_toolkit.draw_selectables(
                                                 move |item| Some(item.id) == selected_test_id,
                                                 |t| &t.name,
                                                 &tests,
                                                 move |test| {
                                                     let id = test.id;
                                                     cmd_buffer2
                            .borrow_mut()
                            .add_controller_command(move |cont| {
                                cont.mark_test_selected(subject, id);
                            })
                                                 },
                )
            },
            &|| {
                let cmd_buffer = Rc::clone(&self.command_buffer);
                self.ui_toolkit
                    .draw_button("Add a test", colorscheme!(action_color), move || {
                        cmd_buffer.borrow_mut().add_controller_command(move |cont| {
                                                   cont.load_test(tests::Test::new(subject))
                                               })
                    })
            },
            &|| {
                if let Some(selected_test_id) = selected_test_id {
                    self.render_test_details(selected_test_id)
                } else {
                    self.ui_toolkit.draw_all(&[])
                }
            },
        ])
    }

    fn render_test_details(&self, test_id: lang::ID) -> T::DrawResult {
        // just assume it exists because if someone called this, that test had to have existed
        let test = self.controller.get_test(test_id).unwrap();

        self.ui_toolkit.draw_all(&[
            &|| {
                let cmd_buffer1 = Rc::clone(&self.command_buffer);
                let test1 = test.clone();
                self.ui_toolkit.draw_text_input_with_label(
                                                           "Test name",
                                                           &test.name,
                                                           move |newname| {
                                                               let mut test = test1.clone();
                                                               test.name = newname.to_string();
                                                               cmd_buffer1
                            .borrow_mut()
                            .add_controller_command(move |cont| cont.load_test(test))
                                                           },
                                                           || {},
                )
            },
            &|| self.render_code(test.code_id()),
        ])
    }

    fn render_arguments_selector<F: function::SettableArgs + std::clone::Clone>(
        &self,
        func: &F)
        -> T::DrawResult {
        let args = func.takes_args();
        self.ui_toolkit.draw_all(&[
            &|| {
                self.ui_toolkit
                    .draw_text_with_label(&format!("Takes {} argument(s)", args.len()), "Arguments")
            },
            &|| {
                draw_all_iter!(
                               T::self.ui_toolkit,
                               args.iter().enumerate().map(|(current_arg_index, arg)| {
                                   move || {
                                       self.ui_toolkit.draw_all(&[
                            &|| {
                                let func1 = func.clone();
                                let args1 = func.takes_args();
                                let cont1 = Rc::clone(&self.command_buffer);
                                self.ui_toolkit.draw_text_input_with_label(
                                "Name",
                                &arg.short_name,
                                move |newvalue| {
                                    let mut newfunc = func1.clone();
                                    let mut newargs = args1.clone();
                                    let mut newarg = &mut newargs[current_arg_index];
                                    newarg.short_name = newvalue.to_string();
                                    newfunc.set_args(newargs);
                                    cont1.borrow_mut().load_function(newfunc)
                                },
                                &|| {},
                            )
                            },
                            &|| {
                                let func1 = func.clone();
                                let args1 = func.takes_args();
                                let cont1 = Rc::clone(&self.command_buffer);
                                self.render_type_change_combo("Type",
                                                              &arg.arg_type,
                                                              move |newtype| {
                                                                  let mut newfunc = func1.clone();
                                                                  let mut newargs = args1.clone();
                                                                  let mut newarg = &mut newargs
                                                                      [current_arg_index];
                                                                  newarg.arg_type = newtype;
                                                                  newfunc.set_args(newargs);
                                                                  cont1.borrow_mut()
                                                                       .load_function(newfunc)
                                                              })
                            },
                            &|| {
                                let func1 = func.clone();
                                let args1 = func.takes_args();
                                let cont1 = Rc::clone(&self.command_buffer);
                                self.ui_toolkit.draw_button("Delete",
                                                            colorscheme!(danger_color),
                                                            move || {
                                                                let mut newfunc = func1.clone();
                                                                let mut newargs = args1.clone();
                                                                newargs.remove(current_arg_index);
                                                                newfunc.set_args(newargs);
                                                                cont1.borrow_mut()
                                                                     .load_function(newfunc)
                                                            })
                            },
                        ])
                                   }
                               })
                )
            },
            &|| {
                let func1 = func.clone();
                let args1 = args.clone();
                let cont1 = Rc::clone(&self.command_buffer);
                self.ui_toolkit.draw_button(
                    "Add another argument",
                    colorscheme!(action_color),
                    move || {
                                                let mut args = args1.clone();
                                                let mut func = func1.clone();
                                                args.push(lang::ArgumentDefinition::new(
                            lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                            format!("arg{}", args.len()),
                        ));
                                                func.set_args(args);
                                                cont1.borrow_mut().load_function(func);
                                            },
                )
            },
        ])
    }

    fn render_typespec_selector_with_label<F>(&self,
                                              label: &str,
                                              selected_ts_id: ID,
                                              nesting_level: Option<&[usize]>,
                                              onchange: F)
                                              -> T::DrawResult
        where F: Fn(&Box<dyn lang::TypeSpec>) + 'static
    {
        // TODO: pretty sure we can get rid of the clone and let the borrow live until the end
        // but i don't want to mess around with it right now
        let selected_ts = self.env_genie
                              .find_typespec(selected_ts_id)
                              .unwrap()
                              .clone();
        let typespecs = self.env_genie
                            .typespecs()
                            .into_iter()
                            .map(|ts| ts.clone())
                            .collect_vec();
        self.ui_toolkit.draw_combo_box_with_label(label,
                                                  |ts| ts.matches(selected_ts.id()),
                                                  |ts| format_typespec_select(ts, nesting_level),
                                                  &typespecs.iter().collect_vec(),
                                                  move |newts| onchange(newts))
    }

    fn render_type_change_combo<F>(&self,
                                   label: &str,
                                   typ: &lang::Type,
                                   onchange: F)
                                   -> T::DrawResult
        where F: Fn(lang::Type) + 'static
    {
        let onchange = Rc::new(onchange);
        self.ui_toolkit.draw_all(&[&|| {
                                       let onchange = Rc::clone(&onchange);
                                       let type1 = typ.clone();
                                       self.render_typespec_selector_with_label(label,
                                                         typ.typespec_id,
                                                         None,
                                                         move |new_ts| {
                                                             let mut newtype = type1.clone();
                                                             edit_types::set_typespec(&mut newtype,
                                                                                      new_ts,
                                                                                      &[]);
                                                             onchange(newtype)
                                                         })
                                   },
                                   &|| {
                                       let onchange2 = Rc::clone(&onchange);
                                       self.render_type_params_change_combo(typ, onchange2, &[])
                                   }])
    }

    fn render_type_params_change_combo<F>(&self,
                                          root_type: &lang::Type,
                                          onchange: Rc<F>,
                                          nesting_level: &[usize])
                                          -> T::DrawResult
        where F: Fn(lang::Type) + 'static
    {
        let mut type_to_change = root_type.clone();
        let mut type_to_change = &mut type_to_change;
        for param_index in nesting_level.into_iter() {
            type_to_change = &mut type_to_change.params[*param_index]
        }

        draw_all_iter!(
                       T::self.ui_toolkit,
                       type_to_change.params.iter().enumerate().map(|(i, param)| {
                           let onchange = Rc::clone(&onchange);
                           move || {
                               let mut new_nesting_level = nesting_level.to_owned();
                               new_nesting_level.push(i);
                               self.ui_toolkit.draw_all(&[
                    &|| {
                        let onchange = Rc::clone(&onchange);
                        let nnl = new_nesting_level.clone();
                        let root_type1 = root_type.clone();
                        self.render_typespec_selector_with_label(
                                "",
                                param.typespec_id,
                                Some(nesting_level),
                                move |new_ts| {
                                    let mut newtype = root_type1.clone();
                                    edit_types::set_typespec(&mut newtype, new_ts, &nnl);
                                    onchange(newtype)
                                })
                    },
                    &|| {
                        let onchange2 = Rc::clone(&onchange);
                        self.render_type_params_change_combo(root_type,
                                                             onchange2,
                                                             &new_nesting_level)
                    },
                ])
                           }
                       })
        )
    }

    fn render_return_type_selector<F: external_func::ModifyableFunc + std::clone::Clone>(
        &self,
        func: &F)
        -> T::DrawResult {
        // TODO: why doesn't this return a reference???
        let return_type = func.returns();

        let cont = Rc::clone(&self.command_buffer);
        let pyfunc2 = func.clone();

        self.render_type_change_combo("Return type", &return_type, move |newtype| {
                let mut newfunc = pyfunc2.clone();
                newfunc.set_return_type(newtype);
                cont.borrow_mut().load_function(newfunc)
            })
    }

    fn render_old_test_section<F: lang::Function>(&self, func: F) -> T::DrawResult {
        let test_result = self.controller.get_test_result(&func);
        let func = Rc::new(func);
        self.ui_toolkit.draw_all(&[&|| {
                                       self.ui_toolkit
                                           .draw_text(&format!("Test result: {}", test_result))
                                   },
                                   &|| {
                                       let func = func.clone();
                                       let cont = Rc::clone(&self.command_buffer);
                                       self.ui_toolkit.draw_button("Run",
                                                                   colorscheme!(action_color),
                                                                   move || {
                                                                       run_test(&cont,
                                                                                func.as_ref());
                                                                   })
                                   }])
    }

    // TODO: gotta redo this... it needs to know what's focused and stuff :/
    fn render_status_bar(&self) -> T::DrawResult {
        self.ui_toolkit.draw_statusbar(&|| {
                           self.ui_toolkit.draw_text("")
                           //            if let Some(node) = self.controller.get_selected_node() {
                           //                self.ui_toolkit.draw_text(
                           //                    &format!("SELECTED: {}", node.description())
                           //                )
                           //            } else {
                           //                self.ui_toolkit.draw_all(vec![])
                           //            }
                       })
    }

    // TODO: reimplement the console. i disabled it because it needs an ID for the window placement
    // scheme
    //    fn render_console_window(&self) -> T::DrawResult {
    //        let console = self.env_genie.read_console();
    //        if console.is_empty() {
    //            return self.ui_toolkit.draw_all(vec![])
    //        }
    //        self.ui_toolkit.draw_window("Console", &|| {
    //            self.ui_toolkit.draw_text_box(console)
    //        },
    //        None::<fn(Keypress)>,
    //        None::<fn()>)
    //    }

    fn render_code(&self, code_id: lang::ID) -> T::DrawResult {
        let code_editor = self.controller.get_editor(code_id).unwrap();
        CodeEditorRenderer::new(self.ui_toolkit,
                                code_editor,
                                Rc::clone(&self.command_buffer),
                                self.env_genie).render()
    }
}

fn onwindowchange(cmd_buffer: Rc<RefCell<CommandBuffer>>,
                  window_id: lang::ID)
                  -> impl Fn((isize, isize), (usize, usize)) + 'static {
    move |pos, size| {
        cmd_buffer.borrow_mut()
                  .add_controller_command(move |controller| {
                      controller.set_window_position(window_id, pos, size)
                  })
    }
}

fn format_typespec_select(ts: &Box<dyn lang::TypeSpec>, nesting_level: Option<&[usize]>) -> String {
    let indent = match nesting_level {
        Some(nesting_level) => iter::repeat("\t").take(nesting_level.len() + 1).join(""),
        None => "".to_owned(),
    };
    format!("{}{} {}", indent, ts.symbol(), ts.readable_name())
}

fn run_test<F: lang::Function>(command_buffer: &Rc<RefCell<CommandBuffer>>, func: &F) {
    let fc = code_generation::new_function_call_with_placeholder_args(func);
    let id = func.id();
    let command_buffer2 = Rc::clone(command_buffer);
    command_buffer.borrow_mut().run(&fc, move |value| {
                                   let mut command_buffer = command_buffer2.borrow_mut();
                                   command_buffer.add_controller_command(move |controller| {
                                                     controller.test_result_by_func_id
                                                               .insert(id, TestResult::new(value));
                                                 });
                               });
}

fn save_world(cont: &Controller, env: &env::ExecutionEnvironment) -> code_loading::TheWorld {
    code_loading::TheWorld { scripts: cont.script_by_id.values().cloned().collect(),
                             tests: cont.test_by_id.values().cloned().collect(),
                             // save all non-builtin functions and typespecs
                             functions: env.functions
                                           .values()
                                           .filter(|f| !cont.builtins.is_builtin(f.id()))
                                           .cloned()
                                           .collect(),
                             typespecs: env.typespecs
                                           .values()
                                           .filter(|ts| !cont.builtins.is_builtin(ts.id()))
                                           .cloned()
                                           .collect() }
}

pub fn run<F: FnOnce(lang::Value) + 'static>(interpreter: &mut Interpreter,
                                             async_executor: &mut async_executor::AsyncExecutor,
                                             code_node: &lang::CodeNode,
                                             callback: F) {
    let fut = interpreter.evaluate(code_node);
    async_executor.exec(async move {
                      callback(await_eval_result!(fut));
                      let ok: Result<(), ()> = Ok(());
                      ok
                  })
}
