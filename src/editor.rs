use std::cell::RefCell;
//use debug_cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use std::iter;
use std::boxed::FnBox;

use objekt::{clone_trait_object,__internal_clone_trait_object};
use super::env;
use super::lang;
use super::code_loading;
use super::code_generation;
use super::lang::{
    Value,CodeNode,Function,FunctionCall,FunctionReference,StringLiteral,ID,Assignment,Block,
    VariableReference};
use itertools::Itertools;
use super::pystuff;
use super::jsstuff;
use super::external_func;
use super::edit_types;
use super::enums;
use super::structs;
use super::function;
use super::code_function;
use super::code_editor;


pub const SELECTION_COLOR: Color = [1., 1., 1., 0.3];
pub const BLUE_COLOR: Color = [100.0 / 255.0, 149.0 / 255.0, 237.0 / 255.0, 1.0];
pub const YELLOW_COLOR: Color = [253.0 / 255.0, 159.0 / 255.0, 19.0 / 255.0, 1.0];
pub const BLACK_COLOR: Color = [0.0, 0.0, 0.0, 1.0];
pub const RED_COLOR: Color = [0.858, 0.180, 0.180, 1.0];
pub const GREY_COLOR: Color = [0.521, 0.521, 0.521, 1.0];
pub const PURPLE_COLOR: Color = [0.486, 0.353, 0.952, 1.0];
pub const CLEAR_COLOR: Color = [0.0, 0.0, 0.0, 0.0];
pub const SALMON_COLOR: Color = [0.996, 0.286, 0.322, 1.0];

pub const PLACEHOLDER_ICON: &str = "\u{F071}";
pub const PX_PER_INDENTATION_LEVEL : i16 = 20;

pub type Color = [f32; 4];

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

pub struct CodeGenie<'a> {
    code: &'a CodeNode,
    env: &'a env::ExecutionEnvironment,
}

impl<'a> CodeGenie<'a> {
    fn new(code_node: &'a CodeNode, env: &'a env::ExecutionEnvironment) -> Self {
        Self { code: code_node, env }
    }

    // TODO: bug??? for when we add conditionals, it's possible this won't detect assignments made
    // inside of conditionals... ugh scoping is tough
    //
    // update: yeah... for conditionals, we'll have to make another recursive call and keep searching
    // up parent blocks. i think we can do this! just have to find assignments that come before the
    // conditional itself
    fn find_assignments_that_come_before_code(&self, node_id: ID) -> Vec<&Assignment> {
        let block_expression_id = self.find_expression_inside_block_that_contains(node_id);
        if block_expression_id.is_none() {
            return vec![]
        }
        let block_expression_id = block_expression_id.unwrap();
        match self.find_parent(block_expression_id) {
            Some(CodeNode::Block(block)) => {
                // if this dies, it means we found a block that's a parent of a block expression,
                // but then when we looked inside the block it didn't contain that expression. this
                // really shouldn't happen
                let position_in_block = self.find_position_in_block(&block, block_expression_id).unwrap();
                block.expressions.iter()
                    // position in the block is 0 indexed, so this will take every node up TO it
                    .take(position_in_block)
                    .map(|code| code.into_assignment())
                    .filter(|opt| opt.is_some())
                    .map(|opt| opt.unwrap())
                    .collect()
            },
            _ => vec![]
        }
    }

    fn find_position_in_block(&self, block: &lang::Block, block_expression_id: ID) -> Option<usize> {
        block.expressions.iter().position(|code| code.id() == block_expression_id)
    }

//    fn find_expression_inside_block_that_contains(&self, node_id: ID) -> Option<ID> {
//        let parent = self.code.find_parent(node_id);
//        match parent {
//            Some(CodeNode::Block(_)) => Some(node_id),
//            Some(parent_node) => self.find_expression_inside_block_that_contains(
//                parent_node.id()),
//            None => None
//        }
//    }

    fn get_arg_definition(&self, argument_definition_id: ID) -> Option<lang::ArgumentDefinition> {
        // shouldn't all_functions not clone them????
        for function in self.all_functions() {
            for arg_def in function.takes_args() {
                if arg_def.id == argument_definition_id {
                    return Some(arg_def)
                }
            }
        }
        None
    }

    fn get_type_for_arg(&self, argument_definition_id: ID) -> Option<lang::Type> {
        for function in self.all_functions() {
            for arg_def in function.takes_args() {
                if arg_def.id == argument_definition_id {
                    return Some(arg_def.arg_type)
                }
            }
        }
        None
    }

    // this whole machinery cannot handle parameterized types yet :/
    fn find_types_matching(&'a self, str: &'a str) -> impl Iterator<Item = lang::Type> + 'a {
        self.env.list_typespecs()
            .filter(|ts| ts.num_params() == 0)
            .filter(move |ts| ts.readable_name().to_lowercase().contains(str))
            .map(|ts| lang::Type::from_spec_id(ts.id(), vec![]))
    }

    fn get_symbol_for_type(&self, t: &lang::Type) -> String {
        let typespec = self.get_typespec(t.typespec_id).unwrap();
        if typespec.num_params() == 0 {
            return typespec.symbol().to_string()
        }
        let joined_params = t.params.iter()
            .map(|p| self.get_symbol_for_type(p))
            .join(", ");
        format!("{}\u{f053}{}\u{f054}", typespec.symbol(), joined_params)
    }

    fn get_typespec(&self, ts_id: lang::ID) -> Option<&lang::TypeSpec> {
        self.env.find_typespec(ts_id).map(|b| b.as_ref())
    }

    fn root(&self) -> &CodeNode {
        self.code
    }

//    fn find_node(&self, id: ID) -> Option<&CodeNode> {
//        self.code.find_node(id)
//    }

//    fn find_parent(&self, id: ID) -> Option<&CodeNode> {
//        self.code.find_parent(id)
//    }

    fn find_function(&self, id: ID) -> Option<&Box<Function>> {
        self.env.find_function(id)
    }

    fn all_functions(&self) -> impl Iterator<Item = &Box<lang::Function>> {
        self.env.list_functions()
    }

    fn find_struct(&self, id: lang::ID) -> Option<&structs::Struct> {
        self.env.find_struct(id)
    }

    pub fn guess_type(&self, code_node: &CodeNode) -> lang::Type {
        match code_node {
            CodeNode::FunctionCall(function_call) => {
                let func_id = function_call.function_reference().function_id;
                match self.find_function(func_id) {
                    Some(ref func) => func.returns().clone(),
                    // TODO: do we really want to just return Null if we couldn't find the function?
                    None => lang::Type::from_spec(&*lang::NULL_TYPESPEC),
                }
            }
            CodeNode::StringLiteral(_) => {
                lang::Type::from_spec(&*lang::STRING_TYPESPEC)
            }
            CodeNode::Assignment(assignment) => {
                self.guess_type(&*assignment.expression)
            }
            CodeNode::Block(block) => {
                if block.expressions.len() > 0 {
                    let last_expression_in_block= &block.expressions[block.expressions.len() - 1];
                    self.guess_type(last_expression_in_block)
                } else {
                    lang::Type::from_spec(&*lang::NULL_TYPESPEC)
                }
            }
            CodeNode::VariableReference(_) => {
                lang::Type::from_spec(&*lang::NULL_TYPESPEC)
            }
            CodeNode::FunctionReference(_) => {
                lang::Type::from_spec(&*lang::NULL_TYPESPEC)
            }
            CodeNode::FunctionDefinition(_) => {
                lang::Type::from_spec(&*lang::NULL_TYPESPEC)
            }
            CodeNode::Argument(arg) => {
                self.get_type_for_arg(arg.argument_definition_id).unwrap()
            }
            CodeNode::Placeholder(_) => {
                lang::Type::from_spec(&*lang::NULL_TYPESPEC)
            }
            CodeNode::NullLiteral => {
                lang::Type::from_spec(&*lang::NULL_TYPESPEC)
            },
            CodeNode::StructLiteral(struct_literal) => {
                let strukt = self.env.find_struct(struct_literal.struct_id).unwrap();
                lang::Type::from_spec(strukt)
            }
            CodeNode::StructLiteralField(struct_literal_field) => {
                let strukt_literal = self.find_parent(struct_literal_field.id)
                    .unwrap().into_struct_literal().unwrap();
                let strukt = self.env.find_struct(strukt_literal.struct_id).unwrap();
                strukt.field_by_id().get(&struct_literal_field.struct_field_id).unwrap()
                    .field_type.clone()
            }
            // this means that both branches of a conditional must be of the same type.we need to
            // add a validation for that
            CodeNode::Conditional(conditional) => {
                self.guess_type(&conditional.true_branch)
            }
            CodeNode::ListLiteral(list_literal) => {
                lang::Type::with_params(&*lang::LIST_TYPESPEC,
                                        vec![list_literal.element_type.clone()])
            }
        }
    }
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
    // TODO: i only need this to be public for hax, so i can make this unpublic later
    execution_environment: Option<env::ExecutionEnvironment>,
    selected_node_id: Option<ID>,
    pub editing: bool,
    insert_code_menu: Option<InsertCodeMenu>,
    // TODO: i only need this to be public for hax, so i can make this unpublic later
    error_console: String,
//    mutation_master: MutationMaster,
    test_result_by_func_id: HashMap<ID, TestResult>,
    // here it is
    code_editor_by_id: HashMap<ID, code_editor::CodeEditor>,
}

impl<'a> Controller {
    pub fn new() -> Controller {
        Controller {
            execution_environment: None,
            selected_node_id: None,
            error_console: String::new(),
            insert_code_menu: None,
            editing: false,
//            mutation_master: MutationMaster::new(),
            test_result_by_func_id: HashMap::new(),
            code_editor_by_id: HashMap::new(),
        }
    }

    pub fn borrow_env<R, F: FnMut(&mut Self) -> R>(&mut self,
                                       env: &mut env::ExecutionEnvironment,
                                       mut borrows_self: F) -> R {
        take_mut::take(env, |env| {
            self.execution_environment = Some(env);
            let ret = borrows_self(self);
            (self.execution_environment.take().unwrap(), ret)
        })
    }

    // TODO: delete this to see what kinda things need to be moved into the CommandBuffer
    fn execution_environment_mut(&mut self) -> &mut env::ExecutionEnvironment {
        self.execution_environment.as_mut().unwrap()
    }

    fn execution_environment(&self) -> &env::ExecutionEnvironment {
        self.execution_environment.as_ref().unwrap()
    }

    fn typespecs(&self) -> impl Iterator<Item = &Box<lang::TypeSpec>> {
        self.execution_environment().list_typespecs()
    }

    fn list_structs(&self) -> impl Iterator<Item = &structs::Struct> {
        self.typespecs()
            .filter_map(|ts| ts.as_ref().downcast_ref::<structs::Struct>())
    }

    fn list_enums(&self) -> impl Iterator<Item = &enums::Enum> {
        self.typespecs()
            .filter_map(|ts| ts.as_ref().downcast_ref::<enums::Enum>())
    }

    fn get_typespec(&self, id: lang::ID) -> Option<&Box<lang::TypeSpec>> {
        self.execution_environment().find_typespec(id)
    }

    fn save(&self) {
        let theworld = code_loading::TheWorld {
            // this needs to be a list of funcs or smth
            main_code: self.code_editor_by_id.values().next().unwrap().get_code().clone(),
            pyfuncs: self.list_pyfuncs().cloned().collect(),
            jsfuncs: self.list_jsfuncs().cloned().collect(),
            structs: self.list_structs().cloned().collect(),
            enums: self.list_enums().cloned().collect(),
        };
        code_loading::save("codesample.json", &theworld).unwrap();
    }

    fn list_jsfuncs(&self) -> impl Iterator<Item = &jsstuff::JSFunc> {
        self.execution_environment().list_functions()
            .filter_map(|f| f.downcast_ref::<jsstuff::JSFunc>())
    }

    fn list_code_funcs(&self) -> impl Iterator<Item = &code_function::CodeFunction> {
        self.execution_environment().list_functions()
            .filter_map(|f| f.downcast_ref::<code_function::CodeFunction>())
    }

    fn list_pyfuncs(&self) -> impl Iterator<Item = &pystuff::PyFunc> {
        self.execution_environment().list_functions()
            .filter_map(|f| f.downcast_ref::<pystuff::PyFunc>())
    }

    // TODO: return a result instead of returning nothing? it seems like there might be places this
    // thing can error
    fn insert_code(&mut self, code_node: CodeNode, insertion_point: InsertionPoint) {
        let genie = self.code_genie();
        let new_code = self.mutation_master.insert_code(&code_node, insertion_point,
                                                        genie.as_ref().unwrap());
        self.loaded_code.as_mut().unwrap().replace(&new_code);
        let genie = self.code_genie();
        match post_insertion_cursor(&code_node, genie.as_ref().unwrap()) {
            PostInsertionAction::SelectNode(id) => { self.set_selected_node_id(Some(id)); }
            PostInsertionAction::MarkAsEditing(insertion_point) => { self.mark_as_editing(insertion_point); }
        }
    }

    fn get_test_result(&self, func: &lang::Function) -> String {
        let test_result = self.test_result_by_func_id.get(&func.id());
        if let Some(test_result) = test_result {
            format!("{:?}", test_result.value)
        } else {
            "Test not run yet".to_string()
        }
    }

    fn redo(&mut self) {
        let loaded_code = self.loaded_code.as_ref().unwrap();
        if let Some(next_root) = self.mutation_master.redo(loaded_code, self.selected_node_id) {
            self.loaded_code.as_mut().unwrap().replace(&next_root.root);
            self.set_selected_node_id(next_root.cursor_position);
        }
    }

    fn delete_selected_code(&mut self) -> Option<()> {
        let deletion_result = self.mutation_master.delete_code(
            self.selected_node_id?, &self.code_genie()?, self.selected_node_id);
        // TODO: these save current state calls can go inside of the mutation master
        self.save_current_state_to_undo_history();
        self.loaded_code.as_mut().unwrap().replace(&deletion_result.new_root);
        // TODO: intelligently select a nearby node to select after deleting
        self.set_selected_node_id(deletion_result.new_cursor_position);
        Some(())
    }

    fn select_current_line(&mut self) {
        let genie = self.code_genie();
        if genie.is_none() || self.selected_node_id.is_none() {
            return
        }
        let genie = genie.unwrap();
        let selected_id = self.selected_node_id.unwrap();
        if let Some(code_id) = genie.find_expression_inside_block_that_contains(selected_id) {
            self.set_selected_node_id(Some(code_id))
        }
    }

//    pub fn hide_insert_code_menu(&mut self) {
//        self.insert_code_menu = None;
//        self.editing = false
//    }

    pub fn insertion_point(&self) -> Option<InsertionPoint> {
        match self.insert_code_menu.as_ref() {
            None => None,
            Some(menu) => Some(menu.insertion_point),
        }
    }

    // TODO: hax passing in the command buffer, we only need it to schedule things to run from the
    // controller :/
    pub fn handle_keypress_in_code_editor(&mut self, id: lang::ID, keypress: Keypress) {
        self.code_editor_by_id.get_mut(&id)
            .map(|ce| ce.handle_keypress(keypress));
    }

//    fn mark_as_editing(&mut self, insertion_point: InsertionPoint) -> Option<()> {
//        self.insert_code_menu = InsertCodeMenu::for_insertion_point(insertion_point,
//                                                                    &self.code_genie()?);
//        self.save_current_state_to_undo_history();
//        self.selected_node_id = insertion_point.selected_node_id();
//        self.editing = true;
//        Some(())
//    }

    fn code_genie(&'a self) -> Option<CodeGenie> {
        Some(CodeGenie::new(
            self.loaded_code.as_ref()?,
            self.execution_environment(),
        ))
    }

    pub fn load_typespec<T: lang::TypeSpec + 'static>(&mut self, typespec: T) {
        self.execution_environment_mut().add_typespec(typespec)
    }

    pub fn load_function<F: Function>(&mut self, function: F) {
        self.execution_environment_mut().add_function(Box::new(function))
    }

    pub fn remove_function(&mut self, id: lang::ID) {
        self.execution_environment_mut().delete_function(id)
    }

    pub fn find_function(&self, id: ID) -> Option<&Box<Function>> {
        self.execution_environment().find_function(id)
    }

    pub fn load_code(&mut self, code_node: &CodeNode) {
        // OLD
        self.loaded_code = Some(code_node.clone());

        // NEW
        self.code_editor_by_id.insert(code_node.id(),
                                      code_editor::CodeEditor::new(code_node));
    }

    // should run the loaded code node
    pub fn run(&mut self, _code_node: &CodeNode) {
        // TODO: ugh this doesn't work
    }

    pub fn read_console(&self) -> &str {
        &self.execution_environment().console
    }

    pub fn read_error_console(&self) -> &str {
        &self.error_console
    }

    pub fn set_selected_node_id(&mut self, code_node_id: Option<ID>) {
        self.selected_node_id = code_node_id;
    }

    pub fn get_selected_node_id(&self) -> &Option<ID> {
        &self.selected_node_id
    }

    pub fn get_selected_node(&self) -> Option<&CodeNode> {
        self.loaded_code.as_ref()?.find_node(self.selected_node_id?)
    }

    pub fn save_current_state_to_undo_history(&mut self) {
        if let Some(ref loaded_code) = self.loaded_code {
            self.mutation_master.log_new_mutation(loaded_code, self.selected_node_id)
        }
    }
}

pub trait UiToolkit {
    type DrawResult;

    fn draw_all(&self, draw_results: Vec<Self::DrawResult>) -> Self::DrawResult;
    fn draw_window<F: Fn(Keypress) + 'static>(&self, window_name: &str, draw_fn: &Fn() -> Self::DrawResult, handle_keypress: Option<F>) -> Self::DrawResult;
    fn draw_layout_with_bottom_bar(&self, draw_content_fn: &Fn() -> Self::DrawResult, draw_bottom_bar_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_empty_line(&self) -> Self::DrawResult;
    fn draw_separator(&self) -> Self::DrawResult;
    fn draw_text(&self, text: &str) -> Self::DrawResult;
    fn draw_text_with_label(&self, text: &str, label: &str) -> Self::DrawResult;
    fn draw_button<F: Fn() + 'static>(&self, label: &str, color: Color, onclick: F) -> Self::DrawResult;
    fn draw_small_button<F: Fn() + 'static>(&self, label: &str, color: Color, onclick: F) -> Self::DrawResult;
    fn draw_text_box(&self, text: &str) -> Self::DrawResult;
    fn draw_text_input<F: Fn(&str) + 'static, D: Fn() + 'static>(&self, existing_value: &str, onchange: F, ondone: D) -> Self::DrawResult;
    fn draw_text_input_with_label<F: Fn(&str) + 'static, D: Fn() + 'static>(&self, label: &str, existing_value: &str, onchange: F, ondone: D) -> Self::DrawResult;
    fn draw_multiline_text_input_with_label<F: Fn(&str) -> () + 'static>(&self, label: &str, existing_value: &str, onchange: F) -> Self::DrawResult;
    fn draw_combo_box_with_label<F, G, H, T>(&self, label: &str, is_item_selected: G, format_item: H, items: &[&T], onchange: F) -> Self::DrawResult
        where T: Clone + 'static,
              F: Fn(&T) -> () + 'static,
              G: Fn(&T) -> bool,
              H: Fn(&T) -> String;
    fn draw_checkbox_with_label<F: Fn(bool) + 'static>(&self, label: &str, value: bool, onchange: F) -> Self::DrawResult;
    fn draw_all_on_same_line(&self, draw_fns: &[&Fn() -> Self::DrawResult]) -> Self::DrawResult;
    fn draw_box_around(&self, color: [f32; 4], draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_top_border_inside(&self, color: [f32; 4], thickness: u8,
                              draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_right_border_inside(&self, color: [f32; 4], thickness: u8,
                                draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_left_border_inside(&self, color: [f32; 4], thickness: u8,
                               draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_bottom_border_inside(&self, color: [f32; 4], thickness: u8,
                                 draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_statusbar(&self, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_main_menu_bar(&self, draw_menus: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_menu(&self, label: &str, draw_menu_items: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_menu_item<F: Fn() + 'static>(&self, label: &str, onselect: F) -> Self::DrawResult;
    fn focused(&self, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn indent(&self, px: i16, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn align(&self, lhs: &Fn() -> Self::DrawResult, rhs: &[&Fn() -> Self::DrawResult]) -> Self::DrawResult;
}

// TODO: to simplify things for now, this thing just holds onto closures and
// applies them onto the controller. in the future we could save the actual
// contents into an enum and match on it.... and change things other than the
// controller. for now this is just easier to move us forward
pub struct CommandBuffer {
    controller_commands: Vec<Box<FnBox(&mut Controller)>>,
    interpreter_commands: Vec<Box<FnBox(&mut env::Interpreter)>>,
}

impl CommandBuffer {
    pub fn new() -> Self {
        Self {
            controller_commands: vec![],
            interpreter_commands: vec![],
        }
    }

    pub fn save(&mut self) {
        self.add_controller_command(move |controller| {
            controller.save()
        })
    }

    pub fn load_function<F: lang::Function>(&mut self, func: F) {
        self.add_controller_command(move |controller| {
            controller.load_function(func)
        })
    }

    pub fn hide_insert_code_menu(&mut self) {
        self.add_controller_command(move |controller| {
            controller.hide_insert_code_menu()
        })
    }

    pub fn insert_code(&mut self, code: lang::CodeNode,
                       insertion_point: InsertionPoint) {
        self.add_controller_command(move |controller| {
            controller.insert_code(code, insertion_point)
        })
    }

    pub fn set_search_str_on_insert_code_menu(&mut self, input: &str) {
        let input = input.to_owned();
        self.add_controller_command(move |controller| {
            controller.insert_code_menu.as_mut()
                .map(|m| {m.set_search_str(&input)});
        })
    }

    pub fn replace_code(&mut self, code: lang::CodeNode) {
        self.add_controller_command(move |controller| {
            controller.loaded_code.as_mut().unwrap()
                .replace(&code)
        })
    }

    pub fn mark_as_not_editing(&mut self) {
        self.add_controller_command(move |mut controller| {
            controller.editing = false
        })
    }

    pub fn remove_function(&mut self, func_id: lang::ID) {
        self.add_controller_command(move |controller| {
            controller.remove_function(func_id)
        })
    }

    pub fn load_typespec<T: lang::TypeSpec>(&mut self, ts: T) {
        self.add_controller_command(move |controller| {
            controller.load_typespec(ts)
        })
    }

    pub fn undo(&mut self) {
        self.add_controller_command(move |controller| {
            controller.undo()
        })
    }

    // environment actions
    pub fn run(&mut self, code: &lang::CodeNode, callback: impl FnOnce(lang::Value) + 'static) {
        let code = code.clone();
        self.add_interpreter_command(move |interpreter| {
            interpreter.run(&code, callback);
        })
    }

    pub fn add_controller_command<F: FnOnce(&mut Controller) + 'static>(&mut self, f: F) {
        self.controller_commands.push(Box::new(f));
    }

    pub fn flush_to_controller(&mut self, controller: &mut Controller) {
        for command in self.controller_commands.drain(..) {
            command.call_box((controller,))
        }
    }

    pub fn add_interpreter_command<F: FnOnce(&mut env::Interpreter) + 'static>(&mut self, f: F) {
        self.interpreter_commands.push(Box::new(f));
    }

    pub fn flush_to_interpreter(&mut self, interpreter: &mut env::Interpreter) {
        for command in self.interpreter_commands.drain(..) {
            command.call_box((interpreter,))
        }
    }
}

pub struct Renderer<'a, T> {
    arg_nesting_level: RefCell<u32>,
    indentation_level: RefCell<u8>,
    ui_toolkit: &'a mut T,
    // TODO: take this through the constructor, but now we'll let ppl peek in here
    command_buffer: Rc<RefCell<CommandBuffer>>,
    controller: &'a Controller,
}

impl<'a, T: UiToolkit> Renderer<'a, T> {
    pub fn new(ui_toolkit: &'a mut T, controller: &'a Controller,
               command_buffer: Rc<RefCell<CommandBuffer>>) -> Renderer<'a, T> {
        Self {
            arg_nesting_level: RefCell::new(0),
            indentation_level: RefCell::new(0),
            ui_toolkit,
            controller,
            command_buffer,
        }
    }

    pub fn render_app(&self) -> T::DrawResult {
        self.ui_toolkit.draw_all(vec![
            self.render_main_menu_bar(),
            self.render_code_editors(),
            self.render_console_window(),
            self.render_error_window(),
            self.render_edit_code_funcs(),
            self.render_edit_pyfuncs(),
            self.render_edit_jsfuncs(),
            self.render_edit_structs(),
            self.render_edit_enums(),
            self.render_status_bar()
        ])
    }

    fn render_main_menu_bar(&self) -> T::DrawResult {
        self.ui_toolkit.draw_main_menu_bar(&|| {
            self.ui_toolkit.draw_menu(
                "File",
                &|| {
                    let cont1 = Rc::clone(&self.command_buffer);
                    let cont2 = Rc::clone(&self.command_buffer);
                    let cont3 = Rc::clone(&self.command_buffer);
                    let cont4 = Rc::clone(&self.command_buffer);
                    let cont5 = Rc::clone(&self.command_buffer);
                    self.ui_toolkit.draw_all(vec![
                        self.ui_toolkit.draw_menu_item("Save", move || {
                            cont1.borrow_mut().save();
                        }),
                        self.ui_toolkit.draw_menu_item("Add new function", move || {
                            cont5.borrow_mut().load_function(code_function::CodeFunction::new());
                        }),
                        #[cfg(feature = "default")]
                        self.ui_toolkit.draw_menu_item("Add Python function", move || {
                            cont2.borrow_mut().load_function(pystuff::PyFunc::new());
                        }),
                        #[cfg(feature = "javascript")]
                        self.ui_toolkit.draw_menu_item("Add JavaScript function", move || {
                            cont2.borrow_mut().load_function(jsstuff::JSFunc::new());
                        }),
                        self.ui_toolkit.draw_menu_item("Add Struct", move || {
                            cont3.borrow_mut().load_typespec(structs::Struct::new());
                        }),
                        self.ui_toolkit.draw_menu_item("Add Enum", move || {
                            cont4.borrow_mut().load_typespec(enums::Enum::new());
                        }),
                        self.ui_toolkit.draw_menu_item("Exit", || {
                            std::process::exit(0);
                        }),
                    ])
                }
            )
        })
    }

    fn render_edit_code_funcs(&self) -> T::DrawResult {
        let code_funcs = self.controller.list_code_funcs();
        self.ui_toolkit.draw_all(code_funcs.map(|f| self.render_edit_code_func(f)).collect())
    }

    fn render_edit_code_func(&self, code_func: &code_function::CodeFunction) -> T::DrawResult {
        self.ui_toolkit.draw_all(vec![])
    }

    fn render_edit_pyfuncs(&self) -> T::DrawResult {
        let pyfuncs = self.controller.list_pyfuncs();
        self.ui_toolkit.draw_all(pyfuncs.map(|f| self.render_edit_pyfunc(f)).collect())
    }

    fn render_edit_pyfunc(&self, pyfunc: &pystuff::PyFunc) -> T::DrawResult {
        self.ui_toolkit.draw_window(&format!("Edit PyFunc: {}", pyfunc.id), &|| {
            let cont1 = Rc::clone(&self.command_buffer);
            let pyfunc1 = pyfunc.clone();
            let cont2 = Rc::clone(&self.command_buffer);
            let pyfunc2 = pyfunc.clone();
            let cont3 = Rc::clone(&self.command_buffer);
            let pyfunc3 = pyfunc.clone();

            self.ui_toolkit.draw_all(vec![
                self.ui_toolkit.draw_text_input_with_label(
                    "Function name",
                    pyfunc.name(),
                    move |newvalue| {
                        let mut pyfunc1 = pyfunc1.clone();
                        pyfunc1.name = newvalue.to_string();
                        cont1.borrow_mut().load_function(pyfunc1);
                    },
                    || {},
                ),
                self.render_arguments_selector(pyfunc.clone()),
                self.ui_toolkit.draw_multiline_text_input_with_label(
                    // TODO: add help text here
                    "Prelude",
                    &pyfunc.prelude,
                    move |newvalue| {
                        let mut pyfunc2 = pyfunc2.clone();
                        pyfunc2.prelude = newvalue.to_string();
                        cont2.borrow_mut().load_function(pyfunc2);
                    },
                ),
                self.ui_toolkit.draw_multiline_text_input_with_label(
                    "Code",
                    &pyfunc.eval,
                    move |newvalue| {
                        let mut pyfunc3 = pyfunc3.clone();
                        pyfunc3.eval = newvalue.to_string();
                        cont3.borrow_mut().load_function(pyfunc3);
                    },
                ),
                self.render_return_type_selector(pyfunc),
                self.ui_toolkit.draw_separator(),
                self.render_test_section(pyfunc.clone()),
                self.ui_toolkit.draw_separator(),
                self.render_general_function_menu(pyfunc),
            ])
        },
        None::<fn(Keypress)>)
    }

    fn render_edit_jsfuncs(&self) -> T::DrawResult {
        let jsfuncs = self.controller.list_jsfuncs();
        self.ui_toolkit.draw_all(jsfuncs.map(|f| self.render_edit_jsfunc(f)).collect())
    }

    fn render_edit_jsfunc(&self, jsfunc: &jsstuff::JSFunc) -> T::DrawResult {
        self.ui_toolkit.draw_window(&format!("Edit JSFunc: {}", jsfunc.id), &|| {
            let cont1 = Rc::clone(&self.command_buffer);
            let jsfunc1 = jsfunc.clone();
            let cont3 = Rc::clone(&self.command_buffer);
            let jsfunc3 = jsfunc.clone();

            self.ui_toolkit.draw_all(vec![
                self.ui_toolkit.draw_text_input_with_label(
                    "Function name",
                    jsfunc.name(),
                    move |newvalue| {
                        let mut jsfunc1 = jsfunc1.clone();
                        jsfunc1.name = newvalue.to_string();
                        cont1.borrow_mut().load_function(jsfunc1);
                    },
                    || {},
                ),
                self.render_arguments_selector(jsfunc.clone()),
                self.ui_toolkit.draw_multiline_text_input_with_label(
                    "Code",
                    &jsfunc.eval,
                    move |newvalue| {
                        let mut jsfunc3 = jsfunc3.clone();
                        jsfunc3.eval = newvalue.to_string();
                        cont3.borrow_mut().load_function(jsfunc3);
                    },
                ),
                self.render_return_type_selector(jsfunc),
                self.ui_toolkit.draw_separator(),
                self.render_test_section(jsfunc.clone()),
                self.ui_toolkit.draw_separator(),
                self.render_general_function_menu(jsfunc),
            ])
        },
        None::<fn(Keypress)>)
    }

    fn get_struct(&self, struct_id: lang::ID) -> Option<structs::Struct> {
        let typespec = self.controller.get_typespec(struct_id).cloned()?;
        typespec.downcast::<structs::Struct>().map(|bawx| *bawx).ok()
    }

    fn render_edit_structs(&self) -> T::DrawResult {
        let structs = self.controller.list_structs();
        self.ui_toolkit.draw_all(structs.map(|s| self.render_edit_struct(s)).collect())
    }

    fn render_edit_enums(&self) -> T::DrawResult {
        let structs = self.controller.list_enums();
        self.ui_toolkit.draw_all(structs.map(|e| self.render_edit_enum(e)).collect())
    }

    fn render_edit_struct(&self, strukt: &structs::Struct) -> T::DrawResult {
        self.ui_toolkit.draw_window(
            &format!("Edit Struct: {}", strukt.id),
            &|| {
                let cont1 = Rc::clone(&self.command_buffer);
                let strukt1 = strukt.clone();
                let cont2 = Rc::clone(&self.command_buffer);
                let strukt2 = strukt.clone();

                self.ui_toolkit.draw_all(vec![
                    self.ui_toolkit.draw_text_input_with_label(
                        "Structure name",
                        &strukt.name,
                        move |newvalue| {
                            let mut strukt = strukt1.clone();
                            strukt.name = newvalue.to_string();
                            cont1.borrow_mut().load_typespec(strukt);
                        },
                        &|| {}
                    ),
                    self.ui_toolkit.draw_text_input_with_label(
                        "Symbol",
                        &strukt.symbol,
                        move |newvalue| {
                            let mut strukt = strukt2.clone();
                            strukt.symbol = newvalue.to_string();
                            cont2.borrow_mut().load_typespec(strukt);
                        },
                        &|| {},
                    ),
                    self.render_struct_fields_selector(strukt),
                    self.render_general_struct_menu(strukt),
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

        let mut to_draw = vec![
            self.ui_toolkit.draw_text_with_label(&format!("Has {} field(s)", fields.len()),
                                                 "Fields"),
        ];

        for (current_field_index, field) in fields.iter().enumerate() {
            let strukt1 = strukt.clone();
            let cont1 = Rc::clone(&self.command_buffer);
            to_draw.push(self.ui_toolkit.draw_text_input_with_label(
                "Name",
                &field.name,
                move |newvalue| {
                    let mut newstrukt = strukt1.clone();
                    let mut newfield = &mut newstrukt.fields[current_field_index];
                    newfield.name = newvalue.to_string();
                    cont1.borrow_mut().load_typespec(newstrukt)
                },
                &||{}));

            let strukt1 = strukt.clone();
            let cont1 = Rc::clone(&self.command_buffer);
            to_draw.push(self.render_type_change_combo(
                "Type",
                &field.field_type,
                move |newtype| {
                    let mut newstrukt = strukt1.clone();
                    let mut newfield = &mut newstrukt.fields[current_field_index];
                    newfield.field_type = newtype;
                    cont1.borrow_mut().load_typespec(newstrukt)
                }
            ));

           let strukt1 = strukt.clone();
           let cont1 = Rc::clone(&self.command_buffer);
           to_draw.push(self.ui_toolkit.draw_button(
               "Delete",
               RED_COLOR,
               move || {
                   let mut newstrukt = strukt1.clone();
                   newstrukt.fields.remove(current_field_index);
                   cont1.borrow_mut().load_typespec(newstrukt)
               }
           ));
        }

        let strukt1 = strukt.clone();
        let cont1 = Rc::clone(&self.command_buffer);
        to_draw.push(self.ui_toolkit.draw_button("Add another field", GREY_COLOR, move || {
            let mut newstrukt = strukt1.clone();
            newstrukt.fields.push(structs::StructField::new(
                format!("field{}", newstrukt.fields.len()),
                lang::Type::from_spec(&*lang::STRING_TYPESPEC),
            ));
            cont1.borrow_mut().load_typespec(newstrukt);
        }));

        self.ui_toolkit.draw_all(to_draw)
    }

    // TODO: a way to delete the struct :)
    fn render_general_struct_menu(&self, _strukt: &structs::Struct) -> T::DrawResult {
        self.ui_toolkit.draw_all(vec![
        ])
    }

    fn render_edit_enum(&self, eneom: &enums::Enum) -> T::DrawResult {
        self.ui_toolkit.draw_window(
            &format!("Edit Enum: {}", eneom.id),
            &|| {
                let cont1 = Rc::clone(&self.command_buffer);
                let eneom1 = eneom.clone();
                let cont2 = Rc::clone(&self.command_buffer);
                let eneom2 = eneom.clone();

                self.ui_toolkit.draw_all(vec![
                    self.ui_toolkit.draw_text_input_with_label(
                        "Enum name",
                        &eneom.name,
                        move |newvalue| {
                            let mut eneom = eneom1.clone();
                            eneom.name = newvalue.to_string();
                            cont1.borrow_mut().load_typespec(eneom);
                        },
                        &|| {}
                    ),
                    self.ui_toolkit.draw_text_input_with_label(
                        "Symbol",
                        &eneom.symbol,
                        move |newvalue| {
                            let mut eneom = eneom2.clone();
                            eneom.symbol = newvalue.to_string();
                            cont2.borrow_mut().load_typespec(eneom);
                        },
                        &|| {},
                    ),
                    self.render_enum_variants_selector(eneom),
//                    self.render_general_struct_menu(eneom),
                ])
            },
            None::<fn(Keypress)>,
        )
    }

    fn render_enum_variants_selector(&self, eneom: &enums::Enum) -> T::DrawResult {
        let variants = &eneom.variants;

        let mut to_draw = vec![
            self.ui_toolkit.draw_text_with_label(&format!("Has {} variant(s)", variants.len()),
                                                 "Variants"),
        ];

        for (current_variant_index, variant) in variants.iter().enumerate() {
            let eneom1 = eneom.clone();
            let cont1 = Rc::clone(&self.command_buffer);
            to_draw.push(self.ui_toolkit.draw_text_input_with_label(
                "Name",
                &variant.name,
                move |newvalue| {
                    let mut neweneom = eneom1.clone();
                    let mut newvariant = &mut neweneom.variants[current_variant_index];
                    newvariant.name = newvalue.to_string();
                    cont1.borrow_mut().load_typespec(neweneom)
                },
                &||{}));

            // TODO: add this checkbox logic to other types?
            let eneom1 = eneom.clone();
            let cont1 = Rc::clone(&self.command_buffer);
            to_draw.push(self.ui_toolkit.draw_checkbox_with_label(
                "Parameterized type?",
                variant.is_parameterized(),
                move |is_parameterized| {
                    let mut neweneom = eneom1.clone();
                    let mut newvariant = &mut neweneom.variants[current_variant_index];
                    if is_parameterized {
                        newvariant.variant_type = None;
                    } else {
                        newvariant.variant_type = Some(lang::Type::from_spec(&*lang::STRING_TYPESPEC));
                    }
                    cont1.borrow_mut().load_typespec(neweneom)
                }
            ));
            if !variant.is_parameterized() {
                let eneom1 = eneom.clone();
                let cont1 = Rc::clone(&self.command_buffer);
                to_draw.push(self.render_type_change_combo(
                    "Type",
                    variant.variant_type.as_ref().unwrap(),
                    move |newtype| {
                        let mut neweneom = eneom1.clone();
                        let mut newvariant = &mut neweneom.variants[current_variant_index];
                        newvariant.variant_type = Some(newtype);
                        cont1.borrow_mut().load_typespec(neweneom)
                    }
                ));
            }

            let eneom1 = eneom.clone();
            let cont1 = Rc::clone(&self.command_buffer);
            to_draw.push(self.ui_toolkit.draw_button(
                "Delete",
                RED_COLOR,
                move || {
                    let mut neweneom = eneom1.clone();
                    neweneom.variants.remove(current_variant_index);
                    cont1.borrow_mut().load_typespec(neweneom)
                }
            ));
        }

        let eneom1 = eneom.clone();
        let cont1 = Rc::clone(&self.command_buffer);
        to_draw.push(self.ui_toolkit.draw_button("Add another variant", GREY_COLOR, move || {
            let mut neweneom = eneom1.clone();
            neweneom.variants.push(enums::EnumVariant::new(
                format!("variant{}", neweneom.variants.len()),
                None,
            ));
            cont1.borrow_mut().load_typespec(neweneom);
        }));

        self.ui_toolkit.draw_all(to_draw)
    }

    fn render_general_function_menu<F: lang::Function>(&self, func: &F) -> T::DrawResult {
        let cont1 = Rc::clone(&self.command_buffer);
        let func_id = func.id();
        self.ui_toolkit.draw_all(vec![
            self.ui_toolkit.draw_button("Delete", RED_COLOR, move || {
                cont1.borrow_mut().remove_function(func_id);
            })
        ])
    }

    fn render_arguments_selector<F: function::SettableArgs + std::clone::Clone>(&self, func: F) -> T::DrawResult {
        let args = func.takes_args();

        let mut to_draw = vec![
            self.ui_toolkit.draw_text_with_label(&format!("Takes {} argument(s)", args.len()),
                                                 "Arguments"),
        ];

        for (current_arg_index, arg) in args.iter().enumerate() {
            let func1 = func.clone();
            let args1 = args.clone();
            let cont1 = Rc::clone(&self.command_buffer);
            to_draw.push(self.ui_toolkit.draw_text_input_with_label(
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
                &||{}));

            let func1 = func.clone();
            let args1 = args.clone();
            let cont1 = Rc::clone(&self.command_buffer);
            to_draw.push(self.render_type_change_combo(
                "Type",
                &arg.arg_type,
                move |newtype| {
                    let mut newfunc = func1.clone();
                    let mut newargs = args1.clone();
                    let mut newarg = &mut newargs[current_arg_index];
                    newarg.arg_type = newtype;
                    newfunc.set_args(newargs);
                    cont1.borrow_mut().load_function(newfunc)
                }
            ));

            let func1 = func.clone();
            let args1 = args.clone();
            let cont1 = Rc::clone(&self.command_buffer);
            to_draw.push(self.ui_toolkit.draw_button(
                "Delete",
                RED_COLOR,
                move || {
                    let mut newfunc = func1.clone();
                    let mut newargs = args1.clone();
                    newargs.remove(current_arg_index);
                    newfunc.set_args(newargs);
                    cont1.borrow_mut().load_function(newfunc)
                }
            ));
        }

        let func1 = func.clone();
        let args1 = args.clone();
        let cont1 = Rc::clone(&self.command_buffer);
        to_draw.push(self.ui_toolkit.draw_button("Add another argument", GREY_COLOR, move || {
            let mut args = args1.clone();
            let mut func = func1.clone();
            args.push(lang::ArgumentDefinition::new(
                lang::Type::from_spec(&*lang::STRING_TYPESPEC),
                format!("arg{}", args.len()),
            ));
            func.set_args(args);
            cont1.borrow_mut().load_function(func);
        }));

        self.ui_toolkit.draw_all(to_draw)
    }

    fn render_typespec_selector_with_label<F>(&self, label: &str, selected_ts_id: ID,
                                              nesting_level: Option<&[usize]>, onchange: F) -> T::DrawResult
        where F: Fn(&Box<lang::TypeSpec>) + 'static
    {
        // TODO: pretty sure we can get rid of the clone and let the borrow live until the end
        // but i don't want to mess around with it right now
        let selected_ts = self.controller.get_typespec(selected_ts_id).unwrap().clone();
        let typespecs = self.controller.typespecs().into_iter()
            .map(|ts| ts.clone()).collect_vec();
        self.ui_toolkit.draw_combo_box_with_label(
            label,
            |ts| ts.matches(selected_ts.id()),
            |ts| format_typespec_select(ts, nesting_level),
            &typespecs.iter().collect_vec(),
            move |newts| { onchange(newts) }
        )
    }

    fn render_type_change_combo<F>(&self, label: &str, typ: &lang::Type, onchange: F) -> T::DrawResult
        where F: Fn(lang::Type) + 'static {
        let type1 = typ.clone();
        let onchange = Rc::new(onchange);
        let onchange2 = Rc::clone(&onchange);
        self.ui_toolkit.draw_all(vec![
            self.render_typespec_selector_with_label(
                label,
                typ.typespec_id,
                None,
                move |new_ts| {
                    let mut newtype = type1.clone();
                    edit_types::set_typespec(&mut newtype, new_ts, &[]);
                    onchange(newtype)
                }
            ),
            self.render_type_params_change_combo(typ, onchange2, &[])
        ])
    }

    fn render_type_params_change_combo<F>(&self, root_type: &lang::Type, onchange: Rc<F>,
                                          nesting_level: &[usize]) -> T::DrawResult
        where F: Fn(lang::Type) + 'static
    {
        let mut type_to_change = root_type.clone();
        let mut type_to_change = &mut type_to_change;
        for param_index in nesting_level.into_iter() {
            type_to_change = &mut type_to_change.params[*param_index]
        }

        let mut drawn = vec![];
        for (i, param) in type_to_change.params.iter().enumerate() {
            let mut new_nesting_level = nesting_level.to_owned();
            new_nesting_level.push(i);

            let onchange = Rc::clone(&onchange);
            let onchange2 = Rc::clone(&onchange);
            let nnl = new_nesting_level.clone();
            let root_type1 = root_type.clone();
            drawn.push(
                self.render_typespec_selector_with_label(
                    "",
                    param.typespec_id,
                    Some(nesting_level),
                    move |new_ts| {
                        let mut newtype = root_type1.clone();
                        edit_types::set_typespec(&mut newtype, new_ts, &nnl);
                        onchange(newtype)
                    }
                ),
            );
            drawn.push(self.render_type_params_change_combo(root_type, onchange2, &new_nesting_level));
        }
        self.ui_toolkit.draw_all(drawn)
    }

    fn render_return_type_selector<F: external_func::ModifyableFunc + std::clone::Clone>(&self, func: &F) -> T::DrawResult {
        // TODO: why doesn't this return a reference???
        let return_type = func.returns();

        let cont = Rc::clone(&self.command_buffer);
        let pyfunc2 = func.clone();

        self.ui_toolkit.draw_all(vec![
            self.render_type_change_combo(
                "Return type",
                &return_type,
                move |newtype| {
                    let mut newfunc = pyfunc2.clone();
                    newfunc.set_return_type(newtype);
                    cont.borrow_mut().load_function(newfunc)
                }
            ),
        ])
    }

    fn render_test_section<F: lang::Function>(&self, func: F) -> T::DrawResult {
        let test_result = self.controller.get_test_result(&func);
        let cont = Rc::clone(&self.command_buffer);
        self.ui_toolkit.draw_all(vec![
            self.ui_toolkit.draw_text(&format!("Test result: {}", test_result)),
            self.ui_toolkit.draw_button("Run", GREY_COLOR, move || {
                run_test(&cont, &func);
            })
        ])
    }

    fn render_status_bar(&self) -> T::DrawResult {
        self.ui_toolkit.draw_statusbar(&|| {
            if let Some(node) = self.controller.get_selected_node() {
                self.ui_toolkit.draw_text(
                    &format!("SELECTED: {}", node.description())
                )
            } else {
                self.ui_toolkit.draw_all(vec![])
            }
        })
    }

    fn render_console_window(&self) -> T::DrawResult {
        let console = self.controller.read_console();
        self.ui_toolkit.draw_window("Console", &|| {
            self.ui_toolkit.draw_text_box(console)
        },
        None::<fn(Keypress)>)
    }

    fn render_error_window(&self) -> T::DrawResult {
        let error_console = self.controller.read_error_console();
        self.ui_toolkit.draw_window("Errors", &|| {
            self.ui_toolkit.draw_text_box(error_console)
        },
        None::<fn(Keypress)>)
    }

    fn render_code_editors(&self) -> T::DrawResult {
        self.ui_toolkit.draw_all(
            self.controller.code_editor_by_id.values().map(|ce| self.render_code_editor(ce)).collect()
        )
    }

    fn render_code_editor(&self, code_editor: &code_editor::CodeEditor) -> T::DrawResult {
        let code = code_editor.get_code();
        let cmd_buffer = Rc::clone(&self.command_buffer);
        self.ui_toolkit.draw_window(&code.description(), &|| {
            self.ui_toolkit.draw_layout_with_bottom_bar(
                &||{ self.render_code(code) },
                &||{ self.render_run_button(code) }
            )},
            Some(move |keypress| {
                cmd_buffer.borrow_mut().add_controller_command(|controller| {
                    controller.handle_keypress_in_code_editor(code_editor.id(), keypress)
                })
            })
        )
    }


    fn render_code(&self, code_node: &CodeNode) -> T::DrawResult {
        if self.is_editing(code_node.id()) {
            return self.draw_inline_editor(code_node)
        }
        let draw = ||{
            match code_node {
                CodeNode::FunctionCall(function_call) => {
                    self.render_function_call(&function_call)
                }
                CodeNode::StringLiteral(string_literal) => {
                    self.render_string_literal(&string_literal)
                }
                CodeNode::Assignment(assignment) => {
                    self.render_assignment(&assignment)
                }
                CodeNode::Block(block) => {
                    self.render_block(&block)
                }
                CodeNode::VariableReference(variable_reference) => {
                    self.render_variable_reference(&variable_reference)
                }
                CodeNode::FunctionDefinition(_function_definition) => {
                    self.ui_toolkit.draw_button(
                        &"Function defs are unimplemented",
                        RED_COLOR,
                        ||{}
                    )
                }
                CodeNode::FunctionReference(function_reference) => {
                    self.render_function_reference(&function_reference)
                }
                CodeNode::Argument(argument) => {
                    self.render_function_call_argument(&argument)
                }
                CodeNode::Placeholder(placeholder) => {
                    self.render_placeholder(&placeholder)
                }
                CodeNode::NullLiteral => {
                    self.ui_toolkit.draw_text(&format!(" {} ", lang::NULL_TYPESPEC.symbol))
                },
                CodeNode::StructLiteral(struct_literal) => {
                    self.render_struct_literal(&struct_literal)
                },
                CodeNode::StructLiteralField(_field) => {
                    self.ui_toolkit.draw_all(vec![])
                    // we would, except render_struct_literal_field isn't called from here...
                    //self.render_struct_literal_field(&field)
                },
                CodeNode::Conditional(conditional) => {
                    self.render_conditional(&conditional)
                }
                CodeNode::ListLiteral(list_literal) => {
                    self.render_list_literal(&list_literal, code_node)
                }
            }
        };

        if self.is_selected(code_node.id()) {
            self.draw_selected(&draw)
        } else {
            self.draw_code_node_and_insertion_point_if_before_or_after(code_node, &draw)
        }
    }

    fn draw_selected(&self, draw: &Fn() -> T::DrawResult) -> T::DrawResult {
        self.ui_toolkit.draw_box_around(SELECTION_COLOR, draw)
    }

    fn draw_code_node_and_insertion_point_if_before_or_after(&self, code_node: &CodeNode, draw: &Fn() -> T::DrawResult) -> T::DrawResult {
        let mut drawn: Vec<T::DrawResult> = vec![];
        if self.is_insertion_pointer_immediately_before(code_node.id()) {
            drawn.push(self.render_insert_code_node())
        }
        drawn.push(draw());
        if self.is_insertion_pointer_immediately_after(code_node.id()) {
            drawn.push(self.render_insert_code_node())
        }
        self.ui_toolkit.draw_all(drawn)
    }

    fn is_insertion_pointer_immediately_before(&self, id: ID) -> bool {
        let insertion_point = self.controller.insertion_point();
        match insertion_point {
            Some(InsertionPoint::Before(code_node_id)) if code_node_id == id => {
                true
            }
            _ => false
        }
    }

    fn is_insertion_pointer_immediately_after(&self, id: ID) -> bool {
        let insertion_point = self.controller.insertion_point();
        match insertion_point {
            Some(InsertionPoint::After(code_node_id)) if code_node_id == id => {
                true
            }
            _ => false
        }
    }

    fn render_insert_code_node(&self) -> T::DrawResult {
        let menu = self.controller.insert_code_menu.as_ref().unwrap().clone();

        self.ui_toolkit.draw_all(vec![
            self.ui_toolkit.focused(&||{
                let controller_1 = Rc::clone(&self.command_buffer);
                let controller_2 = Rc::clone(&self.command_buffer);
                let insertion_point = menu.insertion_point.clone();
                let new_code_node = menu.selected_option_code(&self.controller.code_genie().unwrap());

                self.ui_toolkit.draw_text_input(
                    menu.search_str(),
                    move |input|{
                        controller_1.borrow_mut()
                            .set_search_str_on_insert_code_menu(input);
                    },
                    move ||{
                        let mut controller = controller_2.borrow_mut();
                        if let Some(ref new_code_node) = new_code_node {
                            controller.hide_insert_code_menu();
                            controller.insert_code(new_code_node.clone(), insertion_point);
                        } else {
                            controller.undo();
                            controller.hide_insert_code_menu();
                        }
                    })
            }),
            self.render_insertion_options(&menu)
        ])
    }

    fn render_insertion_options(&self, menu: &InsertCodeMenu) -> <T as UiToolkit>::DrawResult {
        let options = menu.list_options(&self.controller.code_genie().unwrap());
        let render_insertion_options : Vec<Box<Fn() -> T::DrawResult>> = options.iter()
            .map(|option| {
                let c : Box<Fn() -> T::DrawResult> = Box::new(move || {
                    self.render_insertion_option(option, menu.insertion_point)
                });
                c
            })
            .collect();
        self.ui_toolkit.draw_all_on_same_line(
            &render_insertion_options.iter()
                .map(|c| c.as_ref()).collect_vec()
        )
    }

    fn render_insertion_option(
        &self, option: &'a InsertCodeMenuOption, insertion_point: InsertionPoint) -> T::DrawResult {
        let is_selected = option.is_selected;
        let button_color = if is_selected { RED_COLOR } else { BLACK_COLOR };
        let controller = Rc::clone(&self.command_buffer);
        let new_code_node = Rc::new(option.new_node.clone());
        let draw = move|| {
            let cont = controller.clone();
            let ncn = new_code_node.clone();
            self.ui_toolkit.draw_small_button(&option.label, button_color, move|| {
                let mut cont2 = cont.borrow_mut();
                cont2.hide_insert_code_menu();
                cont2.insert_code((*ncn).clone(), insertion_point);
            })
        };
        if is_selected {
            self.draw_selected(&draw)
        } else {
            draw()
        }
    }

    fn render_assignment(&self, assignment: &Assignment) -> T::DrawResult {
        self.ui_toolkit.draw_all_on_same_line(&[
            &|| {
                self.render_inline_editable_button(
                    &assignment.name,
                    PURPLE_COLOR,
                    assignment.id
                )
            },
            &|| { self.ui_toolkit.draw_text(" = ") },
            &|| { self.render_code(assignment.expression.as_ref()) }
        ])
    }

    fn render_list_literal(&self, list_literal: &lang::ListLiteral,
                           code_node: &CodeNode) -> T::DrawResult {
        let t = self.controller.code_genie().unwrap().guess_type(code_node);

        // TODO: we can use smth better to express the nesting than ascii art, like our nesting scheme
        //       with the black lines (can actually make that generic so we can swap it with something
        //       else
        let type_symbol = self.get_symbol_for_type(&t);
        let lhs = &|| self.ui_toolkit.draw_button(&type_symbol, BLUE_COLOR, &|| {});

        let insert_pos = match self.controller.insert_code_menu {
            Some(InsertCodeMenu {
                     insertion_point: InsertionPoint::ListLiteralElement { list_literal_id, pos }, ..
                 }) if list_literal_id == list_literal.id => Some(pos),
            _ => None,
        };

        let mut rhs : Vec<Box<Fn() -> T::DrawResult>> = vec![];
        let mut position_label = 0;
        let mut i = 0;
        while i <= list_literal.elements.len() {
            if insert_pos.map_or(false, |insert_pos| insert_pos == i) {
                let position_string = position_label.to_string();
                rhs.push(Box::new(move || {
                    self.ui_toolkit.draw_all_on_same_line(&[
                        &|| {
                            self.ui_toolkit.draw_button(&position_string, BLACK_COLOR, &||{})
                        },
                        &|| self.render_nested(&|| self.render_insert_code_node()),
                    ])
                }));
                position_label += 1;
            }

            list_literal.elements.get(i).map(|el| {
                rhs.push(Box::new(move || {
                    self.ui_toolkit.draw_all_on_same_line(&[
                        &|| {
                            self.ui_toolkit.draw_button(&position_label.to_string(), BLACK_COLOR, &|| {})
                        },
                        &|| self.render_nested(&|| self.render_code(el)),
                    ])
                }));
                position_label += 1;
            });
            i += 1;
        }

        self.ui_toolkit.align(lhs,
                &rhs.iter()
                    .map(|c| c.as_ref())
                    .collect_vec()
        )
    }

    fn render_variable_reference(&self, variable_reference: &VariableReference) -> T::DrawResult {
        let loaded_code = self.controller.loaded_code.as_ref().unwrap();
        let assignment = loaded_code.find_node(variable_reference.assignment_id);
        if let Some(CodeNode::Assignment(assignment)) = assignment {
            self.ui_toolkit.draw_button(&assignment.name, PURPLE_COLOR, &|| {})
        } else {
            self.ui_toolkit.draw_button("Variable reference not found", RED_COLOR, &|| {})
        }
    }

    fn render_block(&self, block: &Block) -> T::DrawResult {
        self.ui_toolkit.draw_all(
            block.expressions.iter().map(|code| self.render_code(code)).collect())
    }

    fn render_function_call(&self, function_call: &FunctionCall) -> T::DrawResult {
        // XXX: we've gotta have this conditional because of a quirk with the way the imgui
        // toolkit works. if render_function_call_arguments doesn't actually draw anything, it
        // will cause the next drawn thing to appear on the same line. weird i know, maybe we can
        // one day fix this jumbledness
        if function_call.args.is_empty() {
            return self.render_code(&function_call.function_reference)
        }

        let rhs = self.render_function_call_arguments(
                function_call.function_reference().function_id,
                function_call.args());
        let rhs : Vec<Box<Fn() -> T::DrawResult>> = rhs
            .iter()
            .map(|cl| {
                let b : Box<Fn() -> T::DrawResult> = Box::new(move || cl(&self));
                b
            })
            .collect_vec();

        self.ui_toolkit.align(
            &|| { self.render_code(&function_call.function_reference) },
            &rhs.iter().map(|b| b.as_ref()).collect_vec()
        )
    }

    fn render_function_reference(&self, function_reference: &FunctionReference) -> T::DrawResult {
        let function_id = function_reference.function_id;

        // TODO: don't do validation in here. this is just so i can see what this error looks
        // like visually. for realz, i would probably be better off having a separate validation
        // step. and THEN show the errors in here. or maybe overlay something on the codenode that
        // contains the error
        //
        // UPDATE: so i tried that, but figured i still needed to have this code here. i guess maybe
        // there's gonna be no avoiding doing double validation in some situations, and that's ok
        // i think
        let mut color = RED_COLOR;
        let mut function_name = format!("Error: function ID {} not found", function_id);

        if let Some(function) = self.controller.find_function(function_id) {
            color = GREY_COLOR;
            function_name = function.name().to_string();
        }
        self.ui_toolkit.draw_button(&function_name, color, &|| {})
    }

    fn render_function_call_arguments(&self, function_id: ID, args: Vec<&lang::Argument>) -> Vec<Box<Fn(&Renderer<T>) -> T::DrawResult>> {
            let function = self.controller.find_function(function_id)
                .map(|func| func.clone());
            let args = args.clone();
            match function {
                Some(function) => {
                    return self.render_args_for_found_function(&*function, args)
                },
                None => {
                    return self.render_args_for_missing_function(args)
                }
            }
    }

    fn render_nested(&self, draw_fn: &Fn() -> T::DrawResult) -> T::DrawResult {
        let top_border_thickness = 1;
        let right_border_thickness = 1;
        let left_border_thickness = 1;
        let bottom_border_thickness = 1;

        let nesting_level = self.arg_nesting_level.replace_with(|i| *i + 1);
        let top_border_thickness = top_border_thickness + nesting_level + 1;
        let drawn = self.ui_toolkit.draw_top_border_inside(BLACK_COLOR, top_border_thickness as u8, &|| {
            self.ui_toolkit.draw_right_border_inside(BLACK_COLOR, right_border_thickness, &|| {
                self.ui_toolkit.draw_left_border_inside(BLACK_COLOR, right_border_thickness, &|| {
                    self.ui_toolkit.draw_bottom_border_inside(BLACK_COLOR, bottom_border_thickness, draw_fn)
                })
            })
        });
        self.arg_nesting_level.replace_with(|i| *i - 1);
        drawn
    }

    fn get_symbol_for_type(&self, t: &lang::Type) -> String {
        self.controller.code_genie().unwrap().get_symbol_for_type(t)
    }

    fn render_function_call_argument(&self, argument: &lang::Argument) -> T::DrawResult {
        let arg_display = {
            let genie = self.controller.code_genie().unwrap();
            match genie.get_arg_definition(argument.argument_definition_id) {
                Some(arg_def) => {
                    let type_symbol = self.get_symbol_for_type(&arg_def.arg_type);
                    format!("{} {}", type_symbol, arg_def.short_name)
                },
                None => "\u{f059}".to_string(),
            }
        };


        self.render_nested(&|| {
            self.ui_toolkit.draw_all_on_same_line(&[
                &|| {
                    self.render_inline_editable_button(&arg_display, BLACK_COLOR, argument.id)
                },
                &|| {
                    self.render_code(argument.expr.as_ref())
                },
            ])
        })
    }

    fn render_args_for_found_function(&self, function: &Function, args: Vec<&lang::Argument>) -> Vec<Box<Fn(&Renderer<T>) -> T::DrawResult>> {
        let provided_arg_by_definition_id : HashMap<ID,lang::Argument> = args.into_iter()
            .map(|arg| (arg.argument_definition_id, arg.clone())).collect();
        let expected_args = function.takes_args();

        let mut draw_fns : Vec<Box<Fn(&Renderer<T>) -> T::DrawResult>> = vec![];

        for expected_arg in expected_args.into_iter() {
            if let Some(provided_arg) = provided_arg_by_definition_id.get(&expected_arg.id).clone() {
                let provided_arg = provided_arg.clone();
                draw_fns.push(Box::new(move |s: &Renderer<T>| s.render_code(&CodeNode::Argument(provided_arg.clone()))))
            } else {
                draw_fns.push(Box::new(move |s: &Renderer<T>| s.render_missing_function_argument(&expected_arg)))
            }
        }
        draw_fns
    }

    fn render_missing_function_argument(&self, _arg: &lang::ArgumentDefinition) -> T::DrawResult {
        self.ui_toolkit.draw_button(
            "this shouldn't have happened, you've got a missing function arg somehow",
            RED_COLOR,
            &|| {})
    }

    fn render_args_for_missing_function(&self, _args: Vec<&lang::Argument>) -> Vec<Box<Fn(&Renderer<T>) -> T::DrawResult>> {
        vec![Box::new(|s: &Renderer<T>| s.ui_toolkit.draw_all(vec![]))]
    }

    fn render_struct_literal(&self, struct_literal: &lang::StructLiteral) -> T::DrawResult {
        // XXX: we've gotta have this conditional because of a quirk with the way the imgui
        // toolkit works. if render_function_call_arguments doesn't actually draw anything, it
        // will cause the next drawn thing to appear on the same line. weird i know, maybe we can
        // one day fix this jumbledness
        let strukt = self.get_struct(struct_literal.struct_id).unwrap();

        if struct_literal.fields.is_empty() {
            return self.render_struct_identifier(&strukt, struct_literal)
        }
        let rhs = self.render_struct_literal_fields(&strukt,
                                                  struct_literal.fields());
        let rhs : Vec<Box<Fn() -> T::DrawResult>> = rhs.into_iter()
            .map(|draw_fn| {
                let b : Box<Fn() -> T::DrawResult> = Box::new(move || draw_fn(&self));
                b
            }).collect_vec();
        self.ui_toolkit.align(
            &|| { self.render_struct_identifier(&strukt, struct_literal) },
            &rhs.iter().map(|b| b.as_ref()).collect_vec()
        )
    }

    fn render_struct_identifier(&self, strukt: &structs::Struct,
                                _struct_literal: &lang::StructLiteral) -> T::DrawResult {
        // TODO: handle when the typespec ain't available
        self.ui_toolkit.draw_button(&strukt.name, BLUE_COLOR, &|| {})
    }

    fn render_struct_literal_fields(&self, strukt: &'a structs::Struct,
        fields: impl Iterator<Item = &'a lang::StructLiteralField>) -> Vec<Box<Fn(&Renderer<T>) -> T::DrawResult>> {
        // TODO: should this map just go inside the struct????
        let struct_field_by_id = strukt.field_by_id();

        let mut to_draw : Vec<Box<Fn(&Renderer<T>) -> T::DrawResult>> = vec![];
        for literal_field in fields {
            // this is where the bug is
            let strukt_field = struct_field_by_id.get(&literal_field.struct_field_id).unwrap();
            let strukt_field = (*strukt_field).clone();
            let literal_feeld = literal_field.clone();
            to_draw.push(Box::new(move |s: &Renderer<T>| {
                s.render_struct_literal_field(&strukt_field, &literal_feeld)
            }));
        }
        to_draw
    }

    fn render_struct_literal_field(&self, field: &structs::StructField,
                                   literal: &lang::StructLiteralField) -> T::DrawResult {
        let field_text = format!("{} {}", self.get_symbol_for_type(&field.field_type),
                                        field.name);
        self.ui_toolkit.draw_all_on_same_line(&[
            &|| {
                if self.is_editing(literal.id) {
                    self.render_insert_code_node()
                } else {
                    self.render_inline_editable_button(&field_text, BLACK_COLOR, literal.id)
                }
            },
            &|| self.render_nested(&|| self.render_code(&literal.expr))
        ])
    }

    fn render_conditional(&self, conditional: &lang::Conditional) -> T::DrawResult {
        self.ui_toolkit.draw_all(vec![
            self.ui_toolkit.draw_all_on_same_line(&[
                &|| { self.ui_toolkit.draw_button("If", GREY_COLOR, &||{}) },
                &|| { self.render_code(&conditional.condition) },
            ]),
            self.render_indented(&|| { self.render_code(&conditional.true_branch) }),
        ])
    }

    fn render_indented(&self, draw_fn: &Fn() -> T::DrawResult) -> T::DrawResult {
        self.ui_toolkit.indent(PX_PER_INDENTATION_LEVEL, draw_fn)
    }

    fn render_placeholder(&self, placeholder: &lang::Placeholder) -> T::DrawResult {
        let mut r = YELLOW_COLOR;
        // LOL: mess around w/ some transparency
        r[3] = 0.4;
        // TODO: maybe use the traffic cone instead of the exclamation triangle,
        // which is kinda hard to see
        self.ui_toolkit.draw_button(
            &format!("{} {}", PLACEHOLDER_ICON, placeholder.description),
            r,
            &|| {})
    }

    fn render_string_literal(&self, string_literal: &StringLiteral) -> T::DrawResult {
        self.render_inline_editable_button(
            &format!("\u{F10D} {} \u{F10E}", string_literal.value),
            CLEAR_COLOR,
            string_literal.id)
    }

    fn render_run_button(&self, code_node: &CodeNode) -> T::DrawResult {
        let controller = self.command_buffer.clone();
        let code_node = code_node.clone();
        self.ui_toolkit.draw_button("Run", GREY_COLOR, move ||{
            let mut controller = controller.borrow_mut();
            controller.run(&code_node, |_|{});
        })
    }

    fn render_inline_editable_button(&self, label: &str, color: Color, code_node_id: lang::ID) -> T::DrawResult {
        let command_buffer = Rc::clone(&self.command_buffer);
        self.ui_toolkit.draw_button(label, color, move || {
            let mut command_buffer = command_buffer.borrow_mut();
            command_buffer.add_controller_command(move |controller| {
                controller.mark_as_editing(InsertionPoint::Editing(code_node_id));
            })
        })
    }

    fn is_selected(&self, code_node_id: ID) -> bool {
        Some(code_node_id) == *self.controller.get_selected_node_id()
    }

    fn is_editing(&self, code_node_id: ID) -> bool {
        self.is_selected(code_node_id) && self.controller.editing
    }

    fn draw_inline_editor(&self, code_node: &CodeNode) -> T::DrawResult {
        // this is kind of a mess. render_insert_code_node() does `focus` inside of
        // it. the other parts of the branch need to be wrapped in focus() but not
        // render_insert_code_node()
        match code_node {
            CodeNode::StringLiteral(string_literal) => {
                self.ui_toolkit.focused(&move ||{
                    let new_literal = string_literal.clone();
                    self.draw_inline_text_editor(
                        &string_literal.value,
                        move |new_value| {
                            let mut sl = new_literal.clone();
                            sl.value = new_value.to_string();
                            CodeNode::StringLiteral(sl)
                        })
                })
            },
            CodeNode::Assignment(assignment) => {
                self.ui_toolkit.focused(&|| {
                    let a = assignment.clone();
                    self.draw_inline_text_editor(
                        &assignment.name,
                        move |new_value| {
                            let mut new_assignment = a.clone();
                            new_assignment.name = new_value.to_string();
                            CodeNode::Assignment(new_assignment)
                        })
                })
            },
            CodeNode::Argument(_) | CodeNode::StructLiteralField(_) => {
                self.render_insert_code_node()
            }
            // the list literal renders its own editor inline
            CodeNode::ListLiteral(list_literal) => {
                self.render_list_literal(list_literal, code_node)
            }
            _ => {
                // TODO: this is super hacks. the editor just reaches in and makes something not
                // editing while rendering lol
                self.command_buffer.borrow_mut().mark_as_not_editing();
                self.ui_toolkit.draw_button(&format!("Not possible to edit {:?}", code_node), RED_COLOR, &||{})
            }
        }
    }

    fn draw_inline_text_editor<F: Fn(&str) -> CodeNode + 'static>(&self, initial_value: &str, new_node_fn: F) -> T::DrawResult {
        let controller = Rc::clone(&self.command_buffer);
        let controller2 = Rc::clone(&self.command_buffer);
        self.ui_toolkit.draw_text_input(
            initial_value,
            move |new_value| {
                controller.borrow_mut().replace_code(new_node_fn(new_value));
            },
            move || {
                let mut controller = controller2.borrow_mut();
                controller.mark_as_not_editing();
            },
            // TODO: i think we need another callback for what happens when you CANCEL
        )
    }
}

enum PostInsertionAction {
    SelectNode(ID),
    MarkAsEditing(InsertionPoint),
}

fn post_insertion_cursor(code_node: &CodeNode, code_genie: &CodeGenie) -> PostInsertionAction {
    if let CodeNode::FunctionCall(function_call) = code_node {
        // if we just inserted a function call, then go to the first arg if there is one
        if function_call.args.len() > 0 {
            let id = function_call.args[0].id();
            return PostInsertionAction::MarkAsEditing(InsertionPoint::Argument(id))
        } else {
            return PostInsertionAction::SelectNode(function_call.id)
        }
    }

    if let CodeNode::StructLiteral(struct_literal) = code_node {
        // if we just inserted a function call, then go to the first arg if there is one
        if struct_literal.fields.len() > 0 {
            let id = struct_literal.fields[0].id();
            return PostInsertionAction::MarkAsEditing(InsertionPoint::StructLiteralField(id))
        } else {
            return PostInsertionAction::SelectNode(struct_literal.id)
        }
    }

    let parent = code_genie.find_parent(code_node.id());
    if let Some(CodeNode::Argument(argument)) = parent {
        // if we just finished inserting into a function call argument, and the next argument is
        // a placeholder, then let's insert into that arg!!!!
        if let Some(CodeNode::FunctionCall(function_call)) = code_genie.find_parent(argument.id) {
            let just_inserted_argument_position = function_call.args.iter()
                .position(|arg| arg.id() == argument.id).unwrap();
            let maybe_next_arg = function_call.args.get(just_inserted_argument_position + 1);
            if let Some(CodeNode::Argument(lang::Argument{ expr: box CodeNode::Placeholder(_), id, .. })) = maybe_next_arg {
                return PostInsertionAction::MarkAsEditing(InsertionPoint::Argument(*id))
            }
        }
    } else if let Some(CodeNode::StructLiteralField(struct_literal_field)) = parent {
        // if we just finished inserting into a function call argument, and the next argument is
        // a placeholder, then let's insert into that arg!!!!
        if let Some(CodeNode::StructLiteral(struct_literal)) = code_genie.find_parent(struct_literal_field.id) {
            let just_inserted_argument_position = struct_literal.fields.iter()
                .position(|field| field.id() == struct_literal_field.id).unwrap();
            let maybe_next_field = struct_literal.fields.get(just_inserted_argument_position + 1);
            if let Some(CodeNode::StructLiteralField(lang::StructLiteralField{ expr: box CodeNode::Placeholder(_), id, .. })) = maybe_next_field {
                return PostInsertionAction::MarkAsEditing(InsertionPoint::StructLiteralField(*id))
            }
        }
    }

    // nothing that we can think of to do next, just chill at the insertion point
    PostInsertionAction::SelectNode(code_node.id())
}

fn format_typespec_select(ts: &Box<lang::TypeSpec>, nesting_level: Option<&[usize]>) -> String {
    let indent = match nesting_level {
        Some(nesting_level) => {
            iter::repeat("\t").take(nesting_level.len() + 1).join("")
        },
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
            controller.test_result_by_func_id.insert(id, TestResult::new(value));
        });
    });
}

