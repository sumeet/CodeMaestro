use debug_cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use super::uuid::Uuid;

use failure::{err_msg};
use failure::Error as Error;
use super::code_loading::{serialize};
use super::env::{ExecutionEnvironment};
use super::editor_views::{FunctionCallView};
use super::lang;
use super::lang::{
    Value,CodeNode,Function,FunctionCall,FunctionReference,StringLiteral,ID,Error as LangError,Assignment,Block,
    VariableReference};


pub const BLUE_COLOR: [f32; 4] = [0.196, 0.584, 0.721, 1.0];
pub const BLACK_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
pub const RED_COLOR: [f32; 4] = [0.858, 0.180, 0.180, 1.0];
pub const GREY_COLOR: [f32; 4] = [0.521, 0.521, 0.521, 1.0];
pub const PURPLE_COLOR: [f32; 4] = [0.486, 0.353, 0.952, 1.0];
pub const CLEAR_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

pub type Color = [f32; 4];


#[derive(Clone)]
struct InsertCodeNodeMenu {
    functions: Vec<Box<Function>>,
    selected_fn_id: ID,
    insertion_point: InsertionPoint,
}

impl InsertCodeNodeMenu {
    fn new(insertion_point: InsertionPoint, functions: Vec<Box<Function>>) -> Self {
        // there should always be at least one function!!!!
        let selected_fn_id = functions.get(0).unwrap().id();
        Self { functions, selected_fn_id, insertion_point }
    }

    fn new_function_call_with_selected_function(&self) -> FunctionCall {
        FunctionCall {
            id: Uuid::new_v4(),
            function_reference: FunctionReference {
                id: Uuid::new_v4(),
                function_id: self.selected_fn_id,
            },
            args: vec![],
        }
    }
}

pub struct Controller {
    execution_environment: ExecutionEnvironment,
    selected_node_id: Option<ID>,
    editing: bool,
    insert_code_node_menu: Option<InsertCodeNodeMenu>,
    loaded_code: Option<CodeNode>,
    error_console: String,
}

#[derive(Debug, Clone, Copy)]
pub enum InsertionPoint {
    Before(ID),
    After(ID),
}

impl InsertionPoint {
    fn node_id(&self) -> ID {
        match *self {
            InsertionPoint::Before(id) => id,
            InsertionPoint::After(id) => id,
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Key {
    A,
    B,
    C,
    D,
    W,
    X,
    R,
    O,
    Escape,
}

impl<'a> Controller {
    pub fn new() -> Controller {
        Controller {
            execution_environment: ExecutionEnvironment::new(),
            selected_node_id: None,
            loaded_code: None,
            error_console: String::new(),
            insert_code_node_menu: None,
            editing: false,
        }
    }

    // TODO: return a result instead of returning nothing? it seems like there might be places this
    // thing can error
    fn insert_code(&mut self, code_node: CodeNode, insertion_point: InsertionPoint) {
        if self.loaded_code.is_none() {
            panic!("why would we try to insert any code and there isn't any loaded?!")
        }
        let mut loaded_code = self.loaded_code.as_mut().unwrap();
        let parent = loaded_code.find_parent(insertion_point.node_id());
        match parent {
            Some(CodeNode::Block(mut block)) => {
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
                }

                self.loaded_code.as_mut().unwrap().replace(&CodeNode::Block(block));
            },
            _ => panic!("unable to insert new code")
        }
    }

    fn hide_insert_code_menu(&mut self) {
        self.insert_code_node_menu = None;
        self.editing = false
    }

    pub fn insertion_point(&self) -> Option<InsertionPoint> {
        match self.insert_code_node_menu.as_ref() {
            None => None,
            Some(menu) => Some(menu.insertion_point),
        }
    }

    pub fn handle_key_press(&mut self, key: Key) {
        if key == Key::Escape {
            self.handle_cancel();
            return
        }
        // don't perform any commands when in edit mode
        if self.editing { return }
        match key {
            Key::B => {
                self.try_select_back_one_node()
            },
            Key::W => {
                self.try_select_forward_one_node()
            },
            Key::C => {
                // TODO: pry need more logic here
                self.editing = true
            },
            Key::R => {
                self.run(&self.loaded_code.as_ref().unwrap().clone())
            },
            Key::O => {
                self.set_insertion_point()
            }
            _ => {},
        }
    }

    fn handle_cancel(&mut self) {
        self.editing = false;
        if self.insert_code_node_menu.is_none() { return }

        match self.insert_code_node_menu.as_ref().unwrap().insertion_point {
            InsertionPoint::After(id) => self.selected_node_id = Some(id),
            InsertionPoint::Before(id) => self.selected_node_id = Some(id)
        }
        self.hide_insert_code_menu()
    }

    fn set_insertion_point(&mut self) {
        if let(Some(expression_id)) = self.currently_focused_block_expression() {
            self.insert_code_node_menu = Some(InsertCodeNodeMenu::new(
                InsertionPoint::After(expression_id),
                self.execution_environment.list_functions()
            ));
            self.editing = true;
            self.selected_node_id = None;
        } else {
            self.hide_insert_code_menu()
        }
    }

    fn currently_focused_block_expression(&self) -> Option<ID> {
        if self.selected_node_id.is_none() {
            return None
        }
        let selected_node_id = self.selected_node_id.unwrap();
        self.find_expression_inside_block_that_contains(selected_node_id)
    }

    fn find_expression_inside_block_that_contains(&self, node_id: ID) -> Option<ID> {
        if self.loaded_code.is_none() { return None }
        let mut loaded_code = self.loaded_code.as_ref().unwrap().clone();
        let parent = loaded_code.find_parent(node_id);
        match parent {
            Some(CodeNode::Block(_)) => Some(node_id),
            Some(parent_node) => self.find_expression_inside_block_that_contains(
                parent_node.id()),
            None => None
        }
    }

    pub fn try_select_back_one_node(&mut self) {
        let root_node_was_selected = self.select_loaded_code_if_nothing_selected();
        if root_node_was_selected.is_err() || root_node_was_selected.unwrap() {
            // if nothing was selected, and we selected the root node, then our job is done.
            return
        }

        let selected_node_id = self.get_selected_node_id().unwrap();
        let mut loaded_code = self.loaded_code.as_ref().unwrap().clone();
        let parent = loaded_code.find_parent(selected_node_id);
        if parent.is_none() {
            return
        }
        let mut parent = parent.unwrap();

        // first try selecting the previous sibling
        if let(Some(mut previous_sibling)) = parent.previous_child(selected_node_id) {
            // but since we're going back, if the previous sibling has children, then let's
            // select the last one. that feels more ergonomic while moving backwards
            let children = previous_sibling.children_mut();
            if children.len() > 0 {
                self.set_selected_node_id(Some(children[0].id()))
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
        let mut loaded_code = self.loaded_code.as_ref().unwrap().clone();

        let mut selected_code = loaded_code.find_node(selected_node_id).as_mut().unwrap().clone();
        let children = selected_code.children_mut();
        let first_child = children.get(0);

        if let(Some(first_child)) = first_child {
            self.set_selected_node_id(Some(first_child.id()));
            return
        }

        let mut node_id_to_find_next_sibling_of = selected_node_id;
        while let(Some(mut parent))= loaded_code.find_parent(node_id_to_find_next_sibling_of) {
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
}

pub trait UiToolkit {
    type DrawResult;

    fn draw_all(&self, draw_results: Vec<Self::DrawResult>) -> Self::DrawResult;
    fn draw_window(&self, window_name: &str, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_layout_with_bottom_bar(&self, draw_content_fn: &Fn() -> Self::DrawResult, draw_bottom_bar_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
    fn draw_empty_line(&self) -> Self::DrawResult;
    fn draw_button<F: Fn() + 'static>(&self, label: &str, color: [f32; 4], f: F) -> Self::DrawResult;
    fn draw_small_button<F: Fn() + 'static>(&self, label: &str, color: [f32; 4], f: F) -> Self::DrawResult;
    fn draw_text_box(&self, text: &str) -> Self::DrawResult;
    fn draw_text_input<F: Fn(&str) -> () + 'static, D: FnOnce() + 'static>(&self, existing_value: &str, onchange: F, ondone: D) -> Self::DrawResult;
    fn draw_all_on_same_line(&self, draw_fns: Vec<&Fn() -> Self::DrawResult>) -> Self::DrawResult;
    fn draw_border_around(&self, draw_fn: &Fn() -> Self::DrawResult) -> Self::DrawResult;
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
        ])
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
            return self.ui_toolkit.focused(&|| { self.draw_inline_editor(code_node) })
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
            }
        };

        if self.is_selected(code_node) {
            self.ui_toolkit.draw_border_around(&draw)
        } else {
            let mut drawn : Vec<T::DrawResult> = vec![];
            drawn.push(draw());
            if self.is_insertion_pointer_immediately_after(code_node.id()) {
                drawn.push(self.render_insert_code_node())
            }
            self.ui_toolkit.draw_all(drawn)
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
        let menu = self.controller.borrow().insert_code_node_menu.as_ref().unwrap().clone();

        self.ui_toolkit.focused(&||{
            let controller = Rc::clone(&self.controller);
            let insertion_point = menu.insertion_point.clone();
            let new_function_call = CodeNode::FunctionCall(menu.new_function_call_with_selected_function());
            self.ui_toolkit.draw_text_input("", |_|{}, move ||{
                let mut cont2 = controller.borrow_mut();
                let id = new_function_call.id();
                cont2.hide_insert_code_menu();
                cont2.insert_code(new_function_call, insertion_point);
                cont2.set_selected_node_id(Some(id))
            })
        });

        self.render_menu_bar_with_options(menu)
    }

    fn render_menu_bar_with_options(&self, menu: InsertCodeNodeMenu) -> <T as UiToolkit>::DrawResult {
        let mut function_line: Vec<Box<Fn() -> T::DrawResult>> = vec![];
        for function in menu.functions {
            let is_option_selected = menu.selected_fn_id == function.id();
            let button_color = if is_option_selected { RED_COLOR } else { BLACK_COLOR };
            function_line.push(Box::new(move || {
                let draw = || {
                    self.ui_toolkit.draw_small_button(&function.name(), button_color, &|| {})
                };
                if is_option_selected {
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
        let loaded_code = controller.loaded_code.as_mut().unwrap();
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
            self.render_function_reference(&function_call.function_reference)
        };

        let mut renderers : Vec<Box<Fn() -> T::DrawResult>> = vec![Box::new(render_function_reference_fn)];
        renderers.push(Box::new(move || {
            self.render_function_call_arguments(function_call.function_reference.function_id, &function_call.args)
        }));
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
        let mut color = RED_COLOR;
        let mut function_name = format!("Error: function ID {} not found", function_id);

        if let(Some(function)) = self.controller.borrow_mut().find_function(function_id) {
            color = BLUE_COLOR;
            function_name = function.name().to_string();
        }
        self.ui_toolkit.draw_button(&function_name, color, &|| {})
    }

    fn render_function_call_arguments(&self, function_id: ID, args: &Vec<lang::Argument>) -> T::DrawResult {
        let function = self.controller.borrow_mut().find_function(function_id)
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

    fn render_args_for_found_function(&self, function: &Function, args: &Vec<lang::Argument>) -> T::DrawResult {
        let provided_arg_by_definition_id : HashMap<ID,&lang::Argument> = args.iter()
            .map(|arg| (arg.argument_definition_id, arg)).collect();
        let expected_args = function.takes_args();

        let draw_results = expected_args.iter().map(|expected_arg| {
            // TODO: display the argument name somewhere in here?
            if let(Some(provided_arg)) = provided_arg_by_definition_id.get(&expected_arg.id) {
                self.render_code(&provided_arg.expr)
            } else {
                self.render_missing_function_argument(expected_arg)
            }
        }).collect();

        // TODO: implement this
        self.ui_toolkit.draw_all(draw_results)
    }

    fn render_missing_function_argument(&self, arg: &lang::ArgumentDefinition) -> T::DrawResult {
        let mut r = RED_COLOR;
        r[3] = 0.4;
        self.ui_toolkit.draw_button( &format!("{} \u{F5C8}", arg.short_name),
            r,
            &|| {})
    }

    fn render_args_for_missing_function(&self, args: &Vec<lang::Argument>) -> T::DrawResult {
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

    fn render_inline_editable_button(&self, label: &str, color: [f32; 4], code_node: &CodeNode) -> T::DrawResult {
        let controller = self.controller.clone();
        let id = code_node.id();
        self.ui_toolkit.draw_button(label, color, move || {
            let mut controller = controller.borrow_mut();
            controller.set_selected_node_id(Some(id));
            controller.editing = true;
        })
    }

    fn is_selected(&self, code_node: &CodeNode) -> bool {
        Some(code_node.id()) == *self.controller.borrow().get_selected_node_id()
    }

    fn is_editing(&self, code_node: &CodeNode) -> bool {
        self.is_selected(code_node) && self.controller.borrow().editing
    }

    fn draw_inline_editor(&self, code_node: &CodeNode) -> T::DrawResult {
        match code_node {
            CodeNode::StringLiteral(string_literal) => {
                let sl = string_literal.clone();
                self.draw_inline_text_editor(
                    &string_literal.value,
                    move |new_value| {
                        let mut new_literal = sl.clone();
                        new_literal.value = new_value.to_string();
                        CodeNode::StringLiteral(new_literal)
                    })
            },
            CodeNode::Assignment(assignment) => {
                let a = assignment.clone();
                self.draw_inline_text_editor(
                    &assignment.name,
                    move |new_value| {
                        let mut new_assignment = a.clone();
                        new_assignment.name = new_value.to_string();
                        CodeNode::Assignment(new_assignment)
                    })
            },
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

