use debug_cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;

use failure::{err_msg};
use failure::Error as Error;
use super::code_loading::{serialize};
use super::env::{ExecutionEnvironment};
use super::editor_views::{FunctionCallView};
use super::lang;
use super::code_generation;
use super::lang::{
    Value,CodeNode,Function,FunctionCall,FunctionReference,StringLiteral,ID,Error as LangError,Assignment,Block,
    VariableReference};
use super::itertools::Itertools;


pub const BLUE_COLOR: Color = [0.196, 0.584, 0.721, 1.0];
pub const YELLOW_COLOR: Color = [253.0 / 255.0, 159.0 / 255.0, 19.0 / 255.0, 1.0];
pub const BLACK_COLOR: Color = [0.0, 0.0, 0.0, 1.0];
pub const RED_COLOR: Color = [0.858, 0.180, 0.180, 1.0];
pub const GREY_COLOR: Color = [0.521, 0.521, 0.521, 1.0];
pub const PURPLE_COLOR: Color = [0.486, 0.353, 0.952, 1.0];
pub const CLEAR_COLOR: Color = [0.0, 0.0, 0.0, 0.0];

pub const PLACEHOLDER_ICON: &str = "\u{F071}";

pub type Color = [f32; 4];

// TODO: types of insert code generators
// 1: variable
// 2: function call to capitalize
// 3: new string literal
// 4: placeholder

#[derive(Clone)]
struct InsertCodeMenu {
    option_generators: Vec<Box<InsertCodeMenuOptionGenerator>>,
    selected_option_index: usize,
    search_params: CodeSearchParams,
    insertion_point: InsertionPoint,
}

impl InsertCodeMenu {
    fn new_expression_inside_code_block(insertion_point: InsertionPoint, env: &ExecutionEnvironment) -> Self {
        Self {
            // TODO: should probably be able to insert new assignment expressions as well
            option_generators: vec![Box::new(InsertFunctionOptionGenerator { all_funcs: env.list_functions() })],
            selected_option_index: 0,
            search_params: CodeSearchParams::empty(),
            insertion_point,
        }
    }

    fn fill_in_argument(argument: &lang::Argument, env: &ExecutionEnvironment, root_node: &CodeNode) -> Self {
        let genie = CodeGenie::new(root_node, env);
        let arg_type = genie.get_type_for_arg(argument.argument_definition_id);
        if arg_type.is_none() {
            panic!("h000000what. couldn't find the argument definition for this thing!")
        }
        let arg_type = arg_type.unwrap();

        let assignments_by_type_id = genie.find_assignments_that_come_before_code(argument.id)
            .into_iter()
            .group_by(|assignment| {
                let assignment : Assignment = (**assignment).clone();
                genie.guess_type(&CodeNode::Assignment(assignment)).id
            })
            .into_iter()
            .map(|(id, assignments)| (id, assignments.cloned().collect::<Vec<Assignment>>()))
            .collect();

        Self {
            option_generators: vec![
                Box::new(InsertVariableReferenceOptionGenerator { assignments_by_type_id }),
                Box::new(InsertFunctionOptionGenerator { all_funcs: env.list_functions() }),
                Box::new(InsertLiteralOptionGenerator {}),
            ],
            selected_option_index: 0,
            search_params: CodeSearchParams::with_type(&arg_type),
            insertion_point: InsertionPoint::Argument(argument.id),
        }
    }

    fn selected_option_code(&self) -> Option<CodeNode> {
        Some(self.list_options().get(self.selected_option_index)?.new_node.clone())
    }

    fn select_next(&mut self) {
        if self.selected_option_index < self.list_options().len() - 1 {
            self.selected_option_index += 1;
        } else {
            self.selected_option_index = 0;
        }
    }

    fn set_search_str(&mut self, input_str: &str) {
        if input_str != self.search_params.input_str {
            self.search_params.input_str = input_str.to_string();
            self.selected_option_index = 0;
        }
    }

    // TODO: i think the selected option index can get out of sync with this generated list, leading
    // to a panic, say if someone types something and changes the number of options without changing
    // the selected index.
    fn list_options(&self) -> Vec<InsertCodeMenuOption> {
        let mut all_options : Vec<InsertCodeMenuOption> = self.option_generators
            .iter()
            .flat_map(|generator| generator.options(&self.search_params))
            .collect();
        all_options.get_mut(self.selected_option_index).as_mut()
            .map(|option| option.is_selected = true);
        all_options
    }
}

trait InsertCodeMenuOptionGenerator : objekt::Clone {
    fn options(&self, search_params: &CodeSearchParams) -> Vec<InsertCodeMenuOption>;
}

clone_trait_object!(InsertCodeMenuOptionGenerator);

#[derive(Clone)]
struct CodeSearchParams {
    return_type: Option<lang::Type>,
    input_str: String,
}

impl CodeSearchParams {
    fn empty() -> Self {
        Self { return_type: None, input_str: "".to_string() }
    }

    fn with_type(t: &lang::Type) -> Self {
        Self { return_type: Some(t.clone()), input_str: "".to_string() }
    }
}

#[derive(Clone)]
struct InsertCodeMenuOption {
    label: String,
    new_node: CodeNode,
    is_selected: bool,
}

#[derive(Clone)]
struct InsertFunctionOptionGenerator {
    all_funcs: Vec<Box<Function>>,
}

impl InsertCodeMenuOptionGenerator for InsertFunctionOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams) -> Vec<InsertCodeMenuOption> {
        let mut functions = self.all_funcs.clone();
        let input_str = search_params.input_str.trim().to_lowercase();
        if !input_str.is_empty() {
            functions = functions.into_iter()
                .filter(|f| {
                    f.name().to_lowercase().contains(&input_str)
                }).collect()
        }
        if let(Some(ref return_type)) = search_params.return_type {
            functions = functions.into_iter()
                .filter(|f| f.returns().id == return_type.id).collect()
        }
        functions.into_iter().map(|func| {
            InsertCodeMenuOption {
                label: func.name().to_string(),
                new_node: code_generation::new_function_call_with_placeholder_args(func.as_ref()),
                is_selected: false,
            }
        }).collect()
    }
}

#[derive(Clone)]
struct InsertVariableReferenceOptionGenerator {
    assignments_by_type_id: HashMap<ID, Vec<Assignment>>,
}

impl InsertCodeMenuOptionGenerator for InsertVariableReferenceOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams) -> Vec<InsertCodeMenuOption> {
        let mut assignments = if let(Some(search_type)) = &search_params.return_type {
            self.assignments_by_type_id.get(&search_type.id).map_or_else(
                || vec![],
                |assignments| assignments.iter()
                    .map(|assignment| assignment.clone()).collect()
            )
        } else {
            self.assignments_by_type_id.iter()
                .flat_map(|(_id, assignments)| assignments)
                .map(|assignment| assignment.clone())
                .collect()
        };

        let input_str = search_params.input_str.trim().to_lowercase();
        if !input_str.is_empty() {
            assignments = assignments.into_iter()
                .filter(|assignment| {
                    assignment.name.to_lowercase().contains(&input_str)
                }).collect()
        }

        assignments.into_iter().map(|assignment| {
            InsertCodeMenuOption {
                label: assignment.name.to_string(),
                new_node: code_generation::new_variable_reference(&assignment),
                is_selected: false,
            }
        }).collect()
    }
}

#[derive(Clone)]
struct InsertLiteralOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertLiteralOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams) -> Vec<InsertCodeMenuOption> {
        let mut options = vec![];
        let input_str = &search_params.input_str;
        if let(Some(ref return_type)) = search_params.return_type {
            if return_type.id == lang::STRING_TYPE.id {
                options.push(
                    InsertCodeMenuOption {
                        label: format!("\u{f10d}{}\u{f10e}", input_str),
                        is_selected: false,
                        new_node: code_generation::new_string_literal(input_str)
                    }
                );
                // design decision made here: all placeholders have types. therefore, it is now
                // required for a placeholder node to have a type, meaning we need to know what the
                // type of a placeholder is to create it. under current conditions that's ok, but i
                // think we can make this less restrictive in the future if we need to
                options.push(
                    InsertCodeMenuOption {
                        label: format!("{} {}", PLACEHOLDER_ICON, input_str),
                        is_selected: false,
                        new_node: code_generation::new_placeholder(input_str, return_type.id),
                    }
                );
            }
        }
        options
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InsertionPoint {
    Before(ID),
    After(ID),
    Argument(ID),
}

impl InsertionPoint {
    fn node_id(&self) -> ID {
        match *self {
            InsertionPoint::Before(id) => id,
            InsertionPoint::After(id) => id,
            InsertionPoint::Argument(id) => id,
        }
    }
}

#[derive(Debug)]
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

#[derive(Debug, PartialEq)]
pub enum Key {
    A,
    B,
    C,
    D,
    H,
    L,
    W,
    X,
    R,
    O,
    Tab,
    Escape,
    LeftArrow,
    RightArrow,
}

pub struct CodeGenie<'a> {
    code: &'a CodeNode,
    env: &'a ExecutionEnvironment,
}

impl<'a> CodeGenie<'a> {
    fn new(code_node: &'a CodeNode, env: &'a ExecutionEnvironment) -> Self {
        Self { code: code_node, env }
    }

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
                let position_in_block = block.expressions.iter()
                    .position(|code| code.id() == block_expression_id)
                    .unwrap();

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

    fn find_expression_inside_block_that_contains(&self, node_id: ID) -> Option<ID> {
        let parent = self.code.find_parent(node_id);
        match parent {
            Some(CodeNode::Block(_)) => Some(node_id),
            Some(parent_node) => self.find_expression_inside_block_that_contains(
                parent_node.id()),
            None => None
        }
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

    fn get_functions_returning_type(&self, t: &lang::Type) -> Vec<Box<Function>> {
        self.all_functions().into_iter()
            .filter(|func| func.returns().id == t.id)
            .collect()
    }

    fn find_node(&self, id: ID) -> Option<&CodeNode> {
        self.code.find_node(id)
    }

    fn find_parent(&self, id: ID) -> Option<&CodeNode> {
        self.code.find_parent(id)
    }

    fn find_function(&self, id: ID) -> Option<&Box<Function>> {
        self.env.find_function(id)
    }

    fn all_functions(&self) -> Vec<Box<Function>> {
        self.env.list_functions()
    }

    pub fn guess_type(&self, code_node: &CodeNode) -> lang::Type {
        match code_node {
            CodeNode::FunctionCall(function_call) => {
                let func_id = function_call.function_reference().function_id;
                match self.find_function(func_id) {
                    Some(ref func) => func.returns().clone(),
                    None => lang::NULL_TYPE.clone()
                }
            }
            CodeNode::StringLiteral(string_literal) => {
                lang::STRING_TYPE.clone()
            }
            CodeNode::Assignment(assignment) => {
                self.guess_type(&assignment.expression)
            }
            CodeNode::Block(block) => {
                if block.expressions.len() > 0 {
                    let last_expression_in_block= &block.expressions[block.expressions.len() - 1];
                    self.guess_type(last_expression_in_block)
                } else {
                    lang::NULL_TYPE.clone()
                }
            }
            CodeNode::VariableReference(variable_reference) => {
                lang::NULL_TYPE.clone()
            }
            CodeNode::FunctionReference(function_reference) => {
                lang::NULL_TYPE.clone()
            }
            CodeNode::FunctionDefinition(function_definition) => {
                lang::NULL_TYPE.clone()
            }
            CodeNode::Argument(argument) => {
                lang::NULL_TYPE.clone()
            }
            CodeNode::Placeholder(placeholder) => {
                lang::NULL_TYPE.clone()
            }
        }
    }
}

pub struct Controller {
    execution_environment: ExecutionEnvironment,
    selected_node_id: Option<ID>,
    editing: bool,
    insert_code_menu: Option<InsertCodeMenu>,
    loaded_code: Option<CodeNode>,
    error_console: String,
    type_by_id: HashMap<ID, lang::Type>,
}

impl<'a> Controller {
    pub fn new() -> Controller {
        Controller {
            execution_environment: ExecutionEnvironment::new(),
            selected_node_id: None,
            loaded_code: None,
            error_console: String::new(),
            insert_code_menu: None,
            editing: false,
            type_by_id: Self::build_types()
        }
    }

    fn build_types() -> HashMap<ID, lang::Type> {
        let mut type_by_id : HashMap<ID, lang::Type> = HashMap::new();
        type_by_id.insert(lang::NULL_TYPE.id, lang::NULL_TYPE.clone());
        type_by_id.insert(lang::STRING_TYPE.id, lang::STRING_TYPE.clone());
        type_by_id.insert(lang::RESULT_TYPE.id, lang::RESULT_TYPE.clone());
        type_by_id
    }

    // TODO: return a result instead of returning nothing? it seems like there might be places this
    // thing can error
    fn insert_code(&mut self, code_node: CodeNode, insertion_point: InsertionPoint) {
        let genie = self.code_genie();
        let parent = genie.as_ref()
            .map(|genie| genie.find_parent(insertion_point.node_id()));
        if parent.is_none() || parent.unwrap().is_none() {
            panic!("unable to insert new code, couldn't find parent to insert into")

        }
        let parent = parent.unwrap().unwrap();
        match insertion_point {
            InsertionPoint::Before(_) | InsertionPoint::After(_) => {
                self.insert_new_expression_in_block(code_node, insertion_point, parent.clone())
            }
            InsertionPoint::Argument(argument_id) => {
                self.insert_expression_into_argument(code_node, argument_id)
            }
        }
    }

    // TODO: error handling
    fn insert_expression_into_argument(&mut self, code_node: CodeNode, argument_id: ID) {
        let loaded_code = self.loaded_code.as_mut().unwrap();
        let mut argument = loaded_code.find_node(argument_id).unwrap().into_argument().clone();
        argument.expr = Box::new(code_node);
        loaded_code.replace(&CodeNode::Argument(argument));
    }

    fn insert_new_expression_in_block(&mut self, code_node: CodeNode, insertion_point: InsertionPoint, parent: CodeNode) {
        match parent {
            CodeNode::Block(mut block) => {
                let insertion_point_in_block_exprs = block.expressions.iter()
                    .position(|exp| exp.id() == insertion_point.node_id());
                if insertion_point_in_block_exprs.is_none() { return }
                let insertion_point_in_block_exprs = insertion_point_in_block_exprs.unwrap();

                match insertion_point {
                    InsertionPoint::Before(_) => {
                        block.expressions.insert(insertion_point_in_block_exprs, code_node)
                    },
                    InsertionPoint::After(_) => {
                        block.expressions.insert(insertion_point_in_block_exprs + 1, code_node)
                    },
                    _ => panic!("bad insertion point type for a block: {:?}", insertion_point)
                }

                self.loaded_code.as_mut().unwrap().replace(&CodeNode::Block(block));
            },
            _ => panic!("should be inserting into type parent, got {:?} instead", parent)
        }
    }

    fn hide_insert_code_menu(&mut self) {
        self.insert_code_menu = None;
        self.editing = false
    }

    pub fn insertion_point(&self) -> Option<InsertionPoint> {
        match self.insert_code_menu.as_ref() {
            None => None,
            Some(menu) => Some(menu.insertion_point),
        }
    }

    pub fn handle_keypress(&mut self, keypress: Keypress) {
        if keypress.key == Key::Escape {
            self.handle_cancel();
            return
        }
        // don't perform any commands when in edit mode
        match (self.editing, keypress.key) {
            (false, Key::B) | (false, Key::LeftArrow) | (false, Key::H) => {
                self.try_select_back_one_node()
            },
            (false, Key::W) | (false, Key::RightArrow) | (false, Key::L) => {
                self.try_select_forward_one_node()
            },
            (false, Key::C) => {
                if let(Some(id)) = self.selected_node_id {
                    self.mark_as_editing(id)
                }
            },
            (false, Key::R) => {
                self.run(&self.loaded_code.as_ref().unwrap().clone())
            },
            (false, Key::O) => {
                if keypress.shift {
                    self.set_insertion_point_on_previous_line_in_block()
                } else {
                    self.set_insertion_point_on_next_line_in_block()
                }
            },
            (_, Key::Tab) => {
                self.insert_code_menu.as_mut()
                    .map(|menu| menu.select_next());
            }
            _ => {},
        }
    }

    fn handle_cancel(&mut self) {
        self.editing = false;
        if self.insert_code_menu.is_none() { return }

        match self.insert_code_menu.as_ref().unwrap().insertion_point {
            InsertionPoint::After(id) => self.selected_node_id = Some(id),
            InsertionPoint::Before(id) => self.selected_node_id = Some(id),
            InsertionPoint::Argument(id) => self.selected_node_id = Some(id),
        }
        self.hide_insert_code_menu()
    }

    fn set_insertion_point_on_previous_line_in_block(&mut self) {
        if let(Some(expression_id)) = self.currently_focused_block_expression() {
            self.insert_code_menu = Some(InsertCodeMenu::new_expression_inside_code_block(
                InsertionPoint::Before(expression_id),
                &self.execution_environment,
            ));
            self.editing = true;
            self.selected_node_id = None;
        } else {
            self.hide_insert_code_menu()
        }
    }

    fn set_insertion_point_on_next_line_in_block(&mut self) {
        if let(Some(expression_id)) = self.currently_focused_block_expression() {
            self.insert_code_menu = Some(InsertCodeMenu::new_expression_inside_code_block(
                InsertionPoint::After(expression_id),
                &self.execution_environment,
            ));
            self.editing = true;
            self.selected_node_id = None;
        } else {
            self.hide_insert_code_menu()
        }
    }

    fn mark_as_editing(&mut self, node_id: ID) {
        let genie = self.code_genie();
        if genie.is_none() {
            return
        }
        let genie = genie.unwrap();
        match genie.find_node(node_id) {
            Some(CodeNode::Argument(argument)) => {
                self.insert_code_menu = Some(
                    InsertCodeMenu::fill_in_argument(
                        argument,
                        &self.execution_environment,
                        self.loaded_code.as_ref().unwrap()));
            }
            _ => ()
        }
        self.selected_node_id = Some(node_id);
        self.editing = true;
    }

    fn currently_focused_block_expression(&self) -> Option<ID> {
        self.code_genie()?
            .find_expression_inside_block_that_contains(self.selected_node_id?)
    }

    fn code_genie(&'a self) -> Option<CodeGenie> {
        Some(CodeGenie::new(
            self.loaded_code.as_ref()?,
            &self.execution_environment,
        ))
    }

    pub fn try_select_back_one_node(&mut self) {
        let root_node_was_selected = self.select_loaded_code_if_nothing_selected();
        if root_node_was_selected.is_err() || root_node_was_selected.unwrap() {
            // if nothing was selected, and we selected the root node, then our job is done.
            return
        }

        let selected_node_id = self.get_selected_node_id().unwrap();
        let loaded_code = self.loaded_code.as_ref().unwrap().clone();
        let parent = loaded_code.find_parent(selected_node_id);
        if parent.is_none() {
            return
        }
        let parent = parent.unwrap();

        // first try selecting the previous sibling
        if let(Some(previous_sibling)) = parent.previous_child(selected_node_id) {
            // but since we're going back, if the previous sibling has children, then let's
            // select the last one. that feels more ergonomic while moving backwards
            let children = previous_sibling.all_children_dfs();
            if children.len() > 0 {
                self.set_selected_node_id(Some(children[children.len() - 1].id()))
            } else {
                self.set_selected_node_id(Some(previous_sibling.id()));
            }
            return
        }

        // if there is no previous sibling, select the parent
        self.set_selected_node_id(Some(parent.id()));
    }

    pub fn try_select_forward_one_node(&mut self) {
        let root_node_was_selected = self.select_loaded_code_if_nothing_selected();
        if root_node_was_selected.is_err() || root_node_was_selected.unwrap() {
            // if nothing was selected, and we selected the root node, then our job is done.
            return
        }

        let selected_node_id = self.get_selected_node_id().unwrap();
        let loaded_code = self.loaded_code.as_ref().unwrap().clone();

        let selected_code = loaded_code.find_node(selected_node_id).as_ref().unwrap().clone();
        let children = selected_code.children();
        let first_child = children.get(0);

        if let(Some(first_child)) = first_child {
            self.set_selected_node_id(Some(first_child.id()));
            return
        }

        let mut node_id_to_find_next_sibling_of = selected_node_id;
        while let(Some(parent))= loaded_code.find_parent(node_id_to_find_next_sibling_of) {
            if let(Some(next_sibling)) = parent.next_child(node_id_to_find_next_sibling_of) {
                self.set_selected_node_id(Some(next_sibling.id()));
                return
            }
            // if there is no sibling, then try going to the next sibling of the parent, recursively
            node_id_to_find_next_sibling_of = parent.id()
        }
    }

    pub fn select_loaded_code_if_nothing_selected(&mut self) -> Result<bool,Error> {
        if self.loaded_code.is_none() { return Err(err_msg("No code loaded")) }
        let loaded_code = self.loaded_code.as_ref().unwrap().clone();
        if self.get_selected_node_id().is_none() {
            self.set_selected_node_id(Some(loaded_code.id()));
            return Ok(true)
        }
        Ok(false)
    }

    pub fn load_function(&mut self, function: Box<Function>) {
        self.execution_environment.add_function(function.clone())
    }

    pub fn find_function(&self, id: ID) -> Option<&Box<Function>> {
        self.execution_environment.find_function(id)
    }

    pub fn load_code(&mut self, code_node: &CodeNode) {
        self.loaded_code = Some(code_node.clone())
    }

    // should run the loaded code node
    pub fn run(&mut self, code_node: &CodeNode) {
        match self.execution_environment.evaluate(code_node) {
            Value::Result(Err(e)) => {
                self.error_console.push_str(&format!("{:?}", e));
                self.error_console.push_str("\n");
            }
            _ => { }
        }
    }

    pub fn read_console(&self) -> &str {
        &self.execution_environment.console
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

}

pub trait UiToolkit {
    type DrawResult;

    fn draw_all(&self, draw_results: Vec<Self::DrawResult>) -> Self::DrawResult;
    fn draw_window(&self, window_name: &str, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_layout_with_bottom_bar(&self, draw_content_fn: &Fn() -> Self::DrawResult, draw_bottom_bar_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_empty_line(&self) -> Self::DrawResult;
    fn draw_text(&self, text: &str) -> Self::DrawResult;
    fn draw_button<F: Fn() + 'static>(&self, label: &str, color: Color, f: F) -> Self::DrawResult;
    fn draw_small_button<F: Fn() + 'static>(&self, label: &str, color: Color, f: F) -> Self::DrawResult;
    fn draw_text_box(&self, text: &str) -> Self::DrawResult;
    fn draw_text_input<F: Fn(&str) -> () + 'static, D: Fn() + 'static>(&self, existing_value: &str, onchange: F, ondone: D) -> Self::DrawResult;
    fn draw_all_on_same_line(&self, draw_fns: Vec<&Fn() -> Self::DrawResult>) -> Self::DrawResult;
    fn draw_border_around(&self, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_statusbar(&self, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn focused(&self, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
}

pub struct Renderer<'a, T> {
    ui_toolkit: &'a mut T,
    controller: Rc<RefCell<Controller>>,
}

impl<'a, T: UiToolkit> Renderer<'a, T> {
    pub fn new(ui_toolkit: &'a mut T, controller: Rc<RefCell<Controller>>) -> Renderer<'a, T> {
        Self {
            ui_toolkit: ui_toolkit,
            controller: Rc::clone(&controller)
        }
    }

    pub fn render_app(&self) -> T::DrawResult {
        self.ui_toolkit.draw_all(vec![
            self.render_code_window(),
            self.render_console_window(),
            self.render_error_window(),
            self.render_status_bar()
        ])
    }

    fn render_status_bar(&self) -> T::DrawResult {
        self.ui_toolkit.draw_statusbar(&|| {
            if let(Some(node)) = self.controller.borrow().get_selected_node() {
                self.ui_toolkit.draw_text(
                    &format!("SELECTED: {}", node.description())
                )
            } else {
                self.ui_toolkit.draw_all(vec![])
            }
        })
    }

    fn render_console_window(&self) -> T::DrawResult {
        let controller = self.controller.clone();
        self.ui_toolkit.draw_window("Console", &|| {
            self.ui_toolkit.draw_text_box(controller.borrow().read_console())
        })
    }

    fn render_error_window(&self) -> T::DrawResult {
        let controller = self.controller.clone();
        self.ui_toolkit.draw_window("Errors", &|| {
            self.ui_toolkit.draw_text_box(controller.borrow().read_error_console())
        })
    }

    fn render_code_window(&self) -> T::DrawResult {
        let loaded_code = self.controller.borrow().loaded_code.clone();
        match loaded_code {
            None => {
                self.ui_toolkit.draw_button("No code loaded", CLEAR_COLOR, &||{})
            },
            Some(ref code) => {
                self.ui_toolkit.draw_window(&code.description(), &|| {
                    self.ui_toolkit.draw_layout_with_bottom_bar(
                        &||{ self.render_code(code) },
                        &||{ self.render_run_button(code) }
                    )})
            }
        }
    }

    fn render_code(&self, code_node: &CodeNode) -> T::DrawResult {
        if self.is_editing(code_node) {
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
                CodeNode::FunctionDefinition(function_definition) => {
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
            }
        };

        if self.is_selected(code_node) {
            self.ui_toolkit.draw_border_around(&draw)
        } else {
            self.draw_code_node_and_insertion_point_if_before_or_after(code_node, &draw)
        }
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
        let insertion_point = self.controller.borrow().insertion_point();
        match insertion_point {
            Some(InsertionPoint::Before(code_node_id)) if code_node_id == id => {
                true
            }
            _ => false
        }
    }

    fn is_insertion_pointer_immediately_after(&self, id: ID) -> bool {
        let insertion_point = self.controller.borrow().insertion_point();
        match insertion_point {
            Some(InsertionPoint::After(code_node_id)) if code_node_id == id => {
                true
            }
            _ => false
        }
    }

    fn render_insert_code_node(&self) -> T::DrawResult {
        let menu = self.controller.borrow().insert_code_menu.as_ref().unwrap().clone();

        self.ui_toolkit.draw_all(vec![
            self.ui_toolkit.focused(&||{
                let controller_1 = Rc::clone(&self.controller);
                let controller_2 = Rc::clone(&self.controller);
                let insertion_point = menu.insertion_point.clone();
                let new_code_node = menu.selected_option_code();

                self.ui_toolkit.draw_text_input(
                    "",
                    move |input|{
                        controller_1.borrow_mut().insert_code_menu.as_mut()
                            .map(|m| {
                                m.set_search_str(input)
                            });
                    },
                    move ||{
                        let mut controller = controller_2.borrow_mut();
                        if let(Some(ref new_code_node)) = new_code_node {
                            let id = new_code_node.id();
                            controller.hide_insert_code_menu();
                            controller.insert_code(new_code_node.clone(), insertion_point);
                            controller.set_selected_node_id(Some(id))
                        } else {
                            controller.hide_insert_code_menu();
                        }
                    })
            }),
            self.render_insertion_options(&menu)
        ])
    }

    fn render_insertion_options(&self, menu: &InsertCodeMenu) -> <T as UiToolkit>::DrawResult {
        let mut function_line: Vec<Box<Fn() -> T::DrawResult>> = vec![];
        for option in menu.list_options() {
            let button_color = if option.is_selected { RED_COLOR } else { BLACK_COLOR };
            function_line.push(Box::new(move || {
                let draw = || {
                    self.ui_toolkit.draw_small_button(&option.label, button_color, &|| {})
                };
                if option.is_selected {
                    self.ui_toolkit.draw_border_around(&draw)
                } else {
                    draw()
                }
            }))
        }
        let mut function_line_refs = vec![];
        for func in function_line.iter() {
            function_line_refs.push(func.as_ref())
        }
        self.ui_toolkit.draw_all_on_same_line(function_line_refs)
    }

    fn render_assignment(&self, assignment: &Assignment) -> T::DrawResult {
        self.ui_toolkit.draw_all_on_same_line(vec![
            &|| {
                self.render_inline_editable_button(
                    &assignment.name,
                    PURPLE_COLOR,
                    &CodeNode::Assignment(assignment.clone())
                )
            },
            &|| { self.ui_toolkit.draw_button("=", CLEAR_COLOR, &|| {}) },
            &|| { self.render_code(assignment.expression.as_ref()) }
        ])
    }

    fn render_variable_reference(&self, variable_reference: &VariableReference) -> T::DrawResult {
        let mut controller = self.controller.borrow_mut();
        let loaded_code = controller.loaded_code.as_ref().unwrap();
        let assignment = loaded_code.find_node(variable_reference.assignment_id);
        if let(Some(CodeNode::Assignment(assignment))) = assignment {
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
        let render_function_reference_fn = || {
            self.render_code(&function_call.function_reference)
        };

        let mut renderers : Vec<Box<Fn() -> T::DrawResult>> = vec![Box::new(render_function_reference_fn)];
        renderers.push(Box::new(|| {
            self.render_function_call_arguments(
                function_call.function_reference().function_id,
                function_call.args())}));
        self.ui_toolkit.draw_all_on_same_line(
            renderers.iter()
                .map(|b| b.as_ref())
                .collect())
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

        if let(Some(function)) = self.controller.borrow_mut().find_function(function_id) {
            color = BLUE_COLOR;
            function_name = function.name().to_string();
        }
        self.ui_toolkit.draw_button(&function_name, color, &|| {})
    }

    fn render_function_call_arguments(&self, function_id: ID, args: Vec<&lang::Argument>) -> T::DrawResult {
        let function = self.controller.borrow().find_function(function_id)
            .map(|func| func.clone());
        match function {
            Some(function) => {
                self.render_args_for_found_function(&*function, args)
            },
            None => {
                self.render_args_for_missing_function(args)
            }
        }
    }

    fn render_function_call_argument(&self, argument: &lang::Argument) -> T::DrawResult {
        let mut type_symbol = "".to_string();
        {
            let controller = self.controller.borrow();
            let genie = controller.code_genie().unwrap();
            type_symbol = match genie.get_type_for_arg(argument.argument_definition_id) {
                Some(arg_type) => arg_type.symbol.clone(),
                None => "\u{f059}".to_string(),
            };
        }
        self.ui_toolkit.draw_all_on_same_line(vec![
            &|| {
                let cont2 = Rc::clone(&self.controller);
                let node_id_to_select = argument.id;
                self.ui_toolkit.draw_small_button(&type_symbol, BLACK_COLOR, move ||{
                    let mut controller = cont2.borrow_mut();
                    controller.mark_as_editing(node_id_to_select);
                })
            },
            &|| {
                self.render_code(argument.expr.as_ref())
            },
        ])
    }

    fn render_args_for_found_function(&self, function: &Function, args: Vec<&lang::Argument>) -> T::DrawResult {
        let provided_arg_by_definition_id : HashMap<ID,lang::Argument> = args.into_iter()
            .map(|arg| (arg.argument_definition_id, arg.clone())).collect();
        let expected_args = function.takes_args();

        let draw_results = expected_args.iter().map(|expected_arg| {
            // TODO: display the argument name somewhere in here?
            if let(Some(provided_arg)) = provided_arg_by_definition_id.get(&expected_arg.id) {
                self.render_code(&CodeNode::Argument(provided_arg.clone()))
            } else {
                self.render_missing_function_argument(expected_arg)
            }
        }).collect();

        // TODO: implement this
        // UHHH: what was this^ TODO for?
        self.ui_toolkit.draw_all(draw_results)
    }

    fn render_missing_function_argument(&self, arg: &lang::ArgumentDefinition) -> T::DrawResult {
        self.ui_toolkit.draw_button(
            "this shouldn't have happened, you've got a missing function arg somehow",
            RED_COLOR,
            &|| {})
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

    fn render_args_for_missing_function(&self, args: Vec<&lang::Argument>) -> T::DrawResult {
        // TODO: implement this
        self.ui_toolkit.draw_all(vec![])
    }

    fn render_string_literal(&self, string_literal: &StringLiteral) -> T::DrawResult {
        self.render_inline_editable_button(
            &format!("\u{F10D} {} \u{F10E}", string_literal.value),
            CLEAR_COLOR,
            &CodeNode::StringLiteral(string_literal.clone())
        )
    }

    fn render_run_button(&self, code_node: &CodeNode) -> T::DrawResult {
        let controller = self.controller.clone();
        let code_node = code_node.clone();
        self.ui_toolkit.draw_button("Run", GREY_COLOR, move ||{
            let mut controller = controller.borrow_mut();
            controller.run(&code_node);
        })
    }

    fn render_inline_editable_button(&self, label: &str, color: Color, code_node: &CodeNode) -> T::DrawResult {
        let controller = self.controller.clone();
        let id = code_node.id();
        self.ui_toolkit.draw_button(label, color, move || {
            let mut controller = controller.borrow_mut();
            controller.mark_as_editing(id);
        })
    }

    fn is_selected(&self, code_node: &CodeNode) -> bool {
        Some(code_node.id()) == *self.controller.borrow().get_selected_node_id()
    }

    fn is_editing(&self, code_node: &CodeNode) -> bool {
        self.is_selected(code_node) && self.controller.borrow().editing
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
            CodeNode::Argument(argument) => {
                self.render_insert_code_node()
            }
            _ => {
                self.controller.borrow_mut().editing = false;
                self.ui_toolkit.draw_button(&format!("Not possible to edit {:?}", code_node), RED_COLOR, &||{})
            }
        }
    }

    fn draw_inline_text_editor<F: Fn(&str) -> CodeNode + 'static>(&self, initial_value: &str, new_node_fn: F) -> T::DrawResult {
        let controller = Rc::clone(&self.controller);
        let controller2 = Rc::clone(&self.controller);
        self.ui_toolkit.draw_text_input(
            initial_value,
            move |new_value| {
                let new_node = new_node_fn(new_value);
                controller.borrow_mut().loaded_code.as_mut().unwrap().replace(&new_node)
            },
            move || {
                controller2.borrow_mut().editing = false
            }
        )
    }
}
