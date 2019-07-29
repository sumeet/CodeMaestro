use std::cell::RefCell;
use std::collections::HashMap;
use std::iter;

use gen_iter::GenIter;
use itertools::Itertools;
use serde_derive::{Deserialize, Serialize};

use super::editor;
use super::insert_code_menu::InsertCodeMenu;
use super::undo;
use crate::editor::Controller;
use cs::builtins::new_result;
use cs::code_function;
use cs::enums::EnumVariant;
use cs::env::ExecutionEnvironment;
use cs::env_genie::EnvGenie;
use cs::lang;
use cs::lang::CodeNode;
use cs::scripts;

pub struct CodeEditor {
    pub code_genie: CodeGenie,
    pub editing: bool,
    selected_node_id: Option<lang::ID>,
    pub insert_code_menu: Option<InsertCodeMenu>,
    mutation_master: MutationMaster,
    pub location: CodeLocation,
}

#[derive(Copy, Clone, Serialize, Deserialize, Debug)]
pub enum CodeLocation {
    Function(lang::ID),
    Script(lang::ID),
    Test(lang::ID),
    JSONHTTPClientURLParams(lang::ID),
    JSONHTTPClientURL(lang::ID),
    ChatTrigger(lang::ID),
}

impl CodeEditor {
    pub fn new(code: lang::CodeNode, location: CodeLocation) -> Self {
        Self { code_genie: CodeGenie::new(code),
               editing: false,
               selected_node_id: None,
               insert_code_menu: None,
               mutation_master: MutationMaster::new(),
               location }
    }

    pub fn id(&self) -> lang::ID {
        self.get_code().id()
    }

    pub fn get_code(&self) -> &lang::CodeNode {
        self.code_genie.root()
    }

    pub fn handle_keypress(&mut self, keypress: editor::Keypress) {
        use super::editor::Key;

        if keypress.key == Key::Escape {
            self.handle_cancel();
            return;
        }
        // don't perform any commands when in edit mode
        match (self.editing, &keypress.key, &keypress.shift, &keypress.ctrl) {
            (false, Key::J, false, false) | (false, Key::DownArrow, _, false) => {
                self.try_select_down_one_node()
            }
            (false, Key::K, false, false) | (false, Key::UpArrow, _, false) => {
                self.try_select_up_one_node()
            }
            (false, Key::B, false, false)
            | (false, Key::LeftArrow, _, false)
            | (false, Key::H, false, false) => self.try_select_back_one_node(),
            (false, Key::W, false, false)
            | (false, Key::RightArrow, _, false)
            | (false, Key::L, false, false) => self.try_select_forward_one_node(),
            (false, Key::C, false, false) => {
                if let Some(id) = self.selected_node_id {
                    self.mark_as_editing(InsertionPoint::Editing(id));
                }
            }
            (false, Key::D, false, false) => {
                self.delete_selected_code();
            }
            (false, Key::A, false, false) => {
                self.try_append_in_selected_node();
            }
            (false, Key::W, true, false) => {
                self.try_enter_wrap_for_selected_node();
            }
            (false, Key::R, false, false) => {
                self.try_enter_replace_edit_for_selected_node();
            }
            (false, Key::R, true, true) => {
                // TODO: this doesn't work right now
                //self.run(&self.get_code().clone());
            }
            (false, Key::R, false, true) => {
                self.redo();
            }
            (false, Key::O, true, false) => self.set_insertion_point_on_previous_line_in_block(),
            (false, Key::O, false, false) => self.set_insertion_point_on_next_line_in_block(),
            (false, Key::U, false, false) => self.undo(),
            (false, Key::V, true, false) => {
                self.select_current_line();
            }
            (_, Key::Tab, false, false) | (_, Key::DownArrow, _, _) => {
                self.insert_code_menu
                    .as_mut()
                    .map(|menu| menu.select_next());
            }
            (_, Key::Tab, true, false) | (_, Key::UpArrow, _, _) => {
                self.insert_code_menu
                    .as_mut()
                    .map(|menu| menu.select_prev());
            }
            _ => {}
        }
    }

    pub fn hide_insert_code_menu(&mut self) {
        self.insert_code_menu = None;
        self.editing = false
    }

    fn handle_cancel(&mut self) {
        self.editing = false;
        if self.insert_code_menu.is_none() {
            return;
        }
        // TODO: oh fuckkkkk the order these things are in... what the hell, and why?
        // so fragile...
        self.undo();
        self.hide_insert_code_menu()
    }

    pub fn mark_as_editing(&mut self, insertion_point: InsertionPoint) -> Option<()> {
        self.insert_code_menu = InsertCodeMenu::for_insertion_point(insertion_point);
        self.save_current_state_to_undo_history();
        self.selected_node_id = insertion_point.node_id_to_select_when_marking_as_editing();
        self.editing = true;
        Some(())
    }

    pub fn mark_as_not_editing(&mut self) {
        self.editing = false
    }

    pub fn undo(&mut self) {
        if let Some(history) = self.mutation_master
                                   .undo(self.get_code(), self.selected_node_id)
        {
            self.replace_code(history.root);
            self.set_selected_node_id(history.cursor_position);
        }
    }

    pub fn get_selected_node_id(&self) -> &Option<lang::ID> {
        &self.selected_node_id
    }

    pub fn set_selected_node_id(&mut self, code_node_id: Option<lang::ID>) {
        self.selected_node_id = code_node_id;
    }

    pub fn replace_code(&mut self, code: lang::CodeNode) {
        self.code_genie.replace(code);
    }

    fn try_select_up_one_node(&mut self) {
        let navigation = Navigation::new(&self.code_genie);
        if let Some(node_id) = navigation.navigate_up_from(self.selected_node_id) {
            self.set_selected_node_id(Some(node_id))
        }
    }

    fn try_select_down_one_node(&mut self) {
        let navigation = Navigation::new(&self.code_genie);
        if let Some(node_id) = navigation.navigate_down_from(self.selected_node_id) {
            self.set_selected_node_id(Some(node_id))
        }
    }

    pub fn try_select_back_one_node(&mut self) {
        let navigation = Navigation::new(&self.code_genie);
        if let Some(node_id) = navigation.navigate_back_from(self.selected_node_id) {
            self.set_selected_node_id(Some(node_id))
        }
    }

    pub fn try_select_forward_one_node(&mut self) {
        let navigation = Navigation::new(&self.code_genie);
        if let Some(node_id) = navigation.navigate_forward_from(self.selected_node_id) {
            self.set_selected_node_id(Some(node_id))
        }
    }

    fn try_enter_wrap_for_selected_node(&mut self) -> Option<()> {
        self.mark_as_editing(InsertionPoint::Wrap(self.selected_node_id?));
        Some(())
    }

    fn try_enter_replace_edit_for_selected_node(&mut self) -> Option<()> {
        let selected_node_id = self.selected_node_id?;
        match self.code_genie.find_parent(selected_node_id)? {
            lang::CodeNode::StructLiteralField(cn) => {
                let id = cn.id;
                self.mark_as_editing(InsertionPoint::StructLiteralField(id));
            }
            lang::CodeNode::Argument(_)
            | lang::CodeNode::Assignment(_)
            | lang::CodeNode::ListIndex(_) => {
                self.mark_as_editing(InsertionPoint::Replace(selected_node_id));
            }
            _ => (),
        }
        Some(())
    }

    fn get_selected_node(&self) -> Option<&lang::CodeNode> {
        self.code_genie.find_node(self.selected_node_id?)
    }

    fn try_append_in_selected_node(&mut self) -> Option<()> {
        let selected_node = self.get_selected_node()?;
        match selected_node {
            lang::CodeNode::ListLiteral(list_literal) => {
                let insertion_point = InsertionPoint::ListLiteralElement { list_literal_id:
                                                                               list_literal.id,
                                                                           pos: 0 };
                self.mark_as_editing(insertion_point);
                return Some(());
            }
            _ => (),
        }
        match self.code_genie.find_parent(selected_node.id())? {
            lang::CodeNode::ListLiteral(list_literal) => {
                let position_of_selected_node =
                    list_literal.elements
                                .iter()
                                .position(|el| el.id() == selected_node.id())?;
                let insertion_point =
                    InsertionPoint::ListLiteralElement { list_literal_id: list_literal.id,
                                                         pos: position_of_selected_node + 1 };
                self.mark_as_editing(insertion_point);
                return Some(());
            }
            _ => (),
        }
        Some(())
    }

    // TODO: factor duplicate code between this method and the next
    fn set_insertion_point_on_previous_line_in_block(&mut self) {
        if self.no_node_selected() {
            let block_id = self.get_code().id();
            self.mark_as_editing(InsertionPoint::BeginningOfBlock(block_id));
        } else if let Some(expression_id) = self.currently_focused_block_expression() {
            self.mark_as_editing(InsertionPoint::Before(expression_id));
        } else {
            self.hide_insert_code_menu()
        }
    }

    pub fn set_insertion_point_on_next_line_in_block(&mut self) {
        if self.no_node_selected() {
            let block_id = self.get_code().id();
            self.mark_as_editing(InsertionPoint::BeginningOfBlock(block_id));
        } else if let Some(expression_id) = self.currently_focused_block_expression() {
            self.mark_as_editing(InsertionPoint::After(expression_id));
        } else {
            self.hide_insert_code_menu()
        }
    }

    fn no_node_selected(&self) -> bool {
        self.get_selected_node().is_none()
    }

    fn currently_focused_block_expression(&self) -> Option<lang::ID> {
        self.code_genie
            .find_expression_inside_block_that_contains(self.selected_node_id?)
    }

    pub fn insertion_point(&self) -> Option<InsertionPoint> {
        match self.insert_code_menu.as_ref() {
            None => None,
            Some(menu) => Some(menu.insertion_point),
        }
    }

    // TODO: return a result instead of returning nothing? it seems like there might be places this
    // thing can error
    pub fn insert_code(&mut self, code_node: CodeNode, insertion_point: InsertionPoint) {
        let new_root = self.mutation_master
                           .insert_code(&code_node, insertion_point, &self.code_genie);
        self.replace_code(new_root);
        match post_insertion_cursor(&code_node, &self.code_genie) {
            PostInsertionAction::SelectNode(id) => {
                self.set_selected_node_id(Some(id));
            }
            PostInsertionAction::MarkAsEditing(insertion_point) => {
                self.mark_as_editing(insertion_point);
            }
        }
    }

    fn redo(&mut self) {
        if let Some(next_root) = self.mutation_master
                                     .redo(self.get_code(), self.selected_node_id)
        {
            self.replace_code(next_root.root);
            self.set_selected_node_id(next_root.cursor_position);
        }
    }

    fn delete_selected_code(&mut self) -> Option<()> {
        let deletion_result = self.mutation_master.delete_code(self.selected_node_id?,
                                                               &self.code_genie,
                                                               self.selected_node_id);
        // TODO: these save current state calls can go inside of the mutation master
        self.save_current_state_to_undo_history();
        self.replace_code(deletion_result.new_root);
        // TODO: intelligently select a nearby node to select after deleting
        self.set_selected_node_id(deletion_result.new_cursor_position);
        Some(())
    }

    fn select_current_line(&mut self) -> Option<()> {
        let code_id = self.code_genie
                          .find_expression_inside_block_that_contains(self.selected_node_id?)?;
        self.set_selected_node_id(Some(code_id));
        Some(())
    }

    pub fn save_current_state_to_undo_history(&mut self) {
        self.mutation_master
            .log_new_mutation(self.get_code(), self.selected_node_id)
    }
}

// the code genie traverses through the code, giving callers various information
pub struct CodeGenie {
    code: lang::CodeNode,
}

impl CodeGenie {
    pub fn new(code: lang::CodeNode) -> Self {
        Self { code }
    }

    pub fn replace(&mut self, code: lang::CodeNode) {
        self.code.replace(code);
    }

    pub fn code_id(&self) -> lang::ID {
        self.code.id()
    }

    pub fn root(&self) -> &lang::CodeNode {
        &self.code
    }

    // TODO: bug??? for when we add conditionals, it's possible this won't detect assignments made
    // inside of conditionals... ugh scoping is tough
    //
    // update: yeah... for conditionals, we'll have to make another recursive call and keep searching
    // up parent blocks. i think we can do this! just have to find assignments that come before the
    // conditional itself
    pub fn find_assignments_that_come_before_code<'a>(
        &'a self,
        node_id: lang::ID,
        is_inclusive: bool)
        -> Box<dyn Iterator<Item = &lang::Assignment> + 'a> {
        let block_expression_id = self.find_expression_inside_block_that_contains(node_id);
        if block_expression_id.is_none() {
            return Box::new(iter::empty());
        }
        let block_expression_id = block_expression_id.unwrap();
        match self.find_parent(block_expression_id) {
            Some(lang::CodeNode::Block(block)) => {
                // if this dies, it means we found a block that's a parent of a block expression,
                // but then when we looked inside the block it didn't contain that expression. this
                // really shouldn't happen
                let mut position_in_block = block.find_position(block_expression_id).unwrap();
                // the is_inclusive flag is used when doing a search for an `InsertionPoint::After`,
                // when we actually want to include the node in the search. this doesn't have any
                // meaning for when we recurse up the tree.
                if is_inclusive {
                    position_in_block += 1;
                }

                Box::new(block.expressions
                              .iter()
                              // position in the block is 0 indexed, so this will take every node up TO it
                              .take(position_in_block)
                              .filter_map(|code| code.into_assignment())
                              .chain(self.find_assignments_that_come_before_code(block.id, false)))
            }
            _ => panic!("this shouldn't have happened, find_expression_inside_block_that_contains \
                         returned a node whose parent isn't a block"),
        }
    }

    fn find_expression_inside_block_that_contains(&self, node_id: lang::ID) -> Option<lang::ID> {
        let parent = self.code.find_parent(node_id);
        match parent {
            Some(lang::CodeNode::Block(_)) => Some(node_id),
            Some(parent_node) => self.find_expression_inside_block_that_contains(parent_node.id()),
            None => None,
        }
    }

    pub fn find_node(&self, id: lang::ID) -> Option<&lang::CodeNode> {
        self.code.find_node(id)
    }

    pub fn find_parent(&self, id: lang::ID) -> Option<&lang::CodeNode> {
        self.code.find_parent(id)
    }

    pub fn all_parents_of(&self, mut id: lang::ID) -> impl Iterator<Item = &CodeNode> {
        GenIter(move || {
            while let Some(parent) = self.find_parent(id) {
                yield parent;
                id = parent.id();
            }
        })
    }

    pub fn any_variable_referencing_assignment(&self, assignment_id: lang::ID) -> bool {
        self.find_all_variables_referencing_assignment(assignment_id)
            .next()
            .is_some()
    }

    pub fn find_all_variables_referencing_assignment(
        &self,
        assignment_id: lang::ID)
        -> impl Iterator<Item = &lang::VariableReference> {
        self.root()
            .all_children_dfs_iter()
            .filter_map(|cn| cn.as_variable_reference())
            .filter(move |vr| vr.assignment_id == assignment_id)
    }

    pub fn guess_type(&self, code_node: &lang::CodeNode, env_genie: &EnvGenie) -> lang::Type {
        match code_node {
            CodeNode::FunctionCall(function_call) => {
                let func_id = function_call.function_reference().function_id;
                match env_genie.find_function(func_id) {
                    Some(ref func) => func.returns().clone(),
                    // TODO: do we really want to just return Null if we couldn't find the function?
                    None => lang::Type::from_spec(&*lang::NULL_TYPESPEC),
                }
            }
            CodeNode::StringLiteral(_) => lang::Type::from_spec(&*lang::STRING_TYPESPEC),
            CodeNode::NumberLiteral(_) => lang::Type::from_spec(&*lang::NUMBER_TYPESPEC),
            CodeNode::Assignment(assignment) => self.guess_type(&*assignment.expression, env_genie),
            CodeNode::Block(block) => {
                if block.expressions.len() > 0 {
                    let last_expression_in_block = &block.expressions[block.expressions.len() - 1];
                    self.guess_type(last_expression_in_block, env_genie)
                } else {
                    lang::Type::from_spec(&*lang::NULL_TYPESPEC)
                }
            }
            CodeNode::VariableReference(vr) => {
                if let Some(assignment) = self.find_node(vr.assignment_id) {
                    self.guess_type(assignment, env_genie)
                } else {
                    // couldn't find assignment with that variable name, looking for function args
                    env_genie.get_type_for_arg(vr.assignment_id)
                             .unwrap_or_else(|| {
                                 self.find_enum_variant_preceding_by_assignment_id(vr.id,
                                                                                   vr.assignment_id,
                                                                                   env_genie)
                                     .unwrap()
                                     .typ
                             })
                    //                        .expect(&format!("couldn't find arg for assignment {}", vr.assignment_id))
                }
            }
            CodeNode::FunctionReference(_) => lang::Type::from_spec(&*lang::NULL_TYPESPEC),
            CodeNode::Argument(arg) => env_genie.get_type_for_arg(arg.argument_definition_id)
                                                .unwrap(),
            CodeNode::Placeholder(placeholder) => placeholder.typ.clone(),
            CodeNode::NullLiteral(_) => lang::Type::from_spec(&*lang::NULL_TYPESPEC),
            CodeNode::StructLiteral(struct_literal) => {
                let strukt = env_genie.find_struct(struct_literal.struct_id).unwrap();
                lang::Type::from_spec(strukt)
            }
            CodeNode::StructLiteralField(struct_literal_field) => {
                let strukt_literal = self.find_parent(struct_literal_field.id)
                                         .unwrap()
                                         .into_struct_literal()
                                         .unwrap();
                let strukt = env_genie.find_struct(strukt_literal.struct_id).unwrap();
                strukt.field_by_id()
                      .get(&struct_literal_field.struct_field_id)
                      .unwrap()
                      .field_type
                      .clone()
            }
            CodeNode::ListLiteral(list_literal) => {
                lang::Type::with_params(&*lang::LIST_TYPESPEC,
                                        vec![list_literal.element_type.clone()])
            }
            // this means that both branches of a conditional must be of the same type.we need to
            // add a validation for that
            CodeNode::Conditional(conditional) => {
                self.guess_type(&conditional.true_branch, env_genie)
            }
            // need the same validation for match ^
            CodeNode::Match(mach) => {
                let first_variant =
                    mach.branch_by_variant_id
                        .values()
                        .next()
                        .expect("match statement must contain at least one variant");
                self.guess_type(first_variant, env_genie)
            }
            CodeNode::StructFieldGet(sfg) => {
                let struct_field = env_genie.find_struct_field(sfg.struct_field_id).unwrap();
                struct_field.field_type.clone()
            }
            CodeNode::ListIndex(list_index) => {
                let list_typ = self.guess_type(list_index.list_expr.as_ref(), env_genie);
                new_result(get_type_from_list(list_typ).unwrap_or_else(|| {
                               let list_typ =
                                   self.guess_type(list_index.list_expr.as_ref(), env_genie);
                               panic!(format!("couldn't extract list element from {:?}", list_typ))
                           }))
            }
        }
    }

    pub fn match_variant_by_variant_id(&self,
                                       mach: &lang::Match,
                                       env_genie: &EnvGenie)
                                       -> HashMap<lang::ID, MatchVariant> {
        let enum_type = self.guess_type(&mach.match_expression, env_genie);
        let eneom = env_genie.find_enum(enum_type.typespec_id).unwrap();
        eneom.variant_types(&enum_type.params)
             .into_iter()
             .map(|(variant, typ)| {
                 (variant.id,
                  MatchVariant { typ: typ.clone(),
                                 enum_variant: variant.clone(),
                                 match_id: mach.id })
             })
             .collect()
    }

    pub fn find_enum_variants_preceding(&self,
                                        node_id: lang::ID,
                                        env_genie: &EnvGenie)
                                        -> Vec<MatchVariant> {
        let mut prev = self.find_node(node_id).unwrap();
        let mut variants = vec![];
        for node in self.all_parents_of(node_id) {
            if let lang::CodeNode::Match(mach) = node {
                for (variant_id, branch) in mach.branch_by_variant_id.iter() {
                    if branch.id() == prev.id() {
                        let mut type_and_enum_by_variant_id =
                            self.match_variant_by_variant_id(mach, env_genie);
                        variants.push(type_and_enum_by_variant_id.remove(variant_id).unwrap());
                    }
                }
            }
            prev = node;
        }
        variants
    }

    pub fn find_enum_variants_preceding_iter<'a>(&'a self,
                                                 node_id: lang::ID,
                                                 env_genie: &'a EnvGenie)
                                                 -> impl Iterator<Item = MatchVariant> + 'a {
        let prev = self.find_node(node_id);
        GenIter(move || {
            if prev.is_none() {
                return;
            }
            let mut prev = prev.unwrap();
            for node in self.all_parents_of(node_id) {
                if let lang::CodeNode::Match(mach) = node {
                    for (variant_id, branch) in mach.branch_by_variant_id.iter() {
                        if branch.id() == prev.id() {
                            let mut type_and_enum_by_variant_id =
                                self.match_variant_by_variant_id(mach, env_genie);
                            yield type_and_enum_by_variant_id.remove(variant_id).unwrap()
                        }
                    }
                }
                prev = node;
            }
        })
    }

    pub fn find_enum_variant_preceding_by_assignment_id<'a>(&'a self,
                                                            behind_id: lang::ID,
                                                            assignment_id: lang::ID,
                                                            env_genie: &'a EnvGenie)
                                                            -> Option<MatchVariant> {
        self.find_enum_variants_preceding_iter(behind_id, env_genie)
            .find(|match_variant| match_variant.assignment_id() == assignment_id)
    }

    pub fn find_enum_variant_by_assignment_id(&self,
                                              assignment_id: lang::ID,
                                              env_genie: &EnvGenie)
                                              -> Option<MatchVariant> {
        self.code
            .all_children_dfs_iter()
            .filter_map(|code_node| {
                if let lang::CodeNode::Match(mach) = code_node {
                    for (variant_id, _branch) in mach.branch_by_variant_id.iter() {
                        if mach.variable_id(*variant_id) == assignment_id {
                            let mut type_and_enum_by_variant_id =
                                self.match_variant_by_variant_id(mach, env_genie);
                            return Some(type_and_enum_by_variant_id.remove(variant_id).unwrap());
                        }
                    }
                }
                None
            })
            .next()
    }

    pub fn is_block_expression(&self, node_id: lang::ID) -> bool {
        if let Some(CodeNode::Block(_)) = self.find_parent(node_id) {
            true
        } else {
            false
        }
    }
}

pub struct MatchVariant {
    pub typ: lang::Type,
    pub enum_variant: EnumVariant,
    pub match_id: lang::ID,
}

impl MatchVariant {
    pub fn assignment_id(&self) -> lang::ID {
        lang::Match::make_variable_id(self.match_id, self.enum_variant.id)
    }
}

pub struct Navigation<'a> {
    code_genie: &'a CodeGenie,
}

impl<'a> Navigation<'a> {
    pub fn new(code_genie: &'a CodeGenie) -> Self {
        Self { code_genie }
    }

    pub fn navigate_up_from(&self, code_node_id: Option<lang::ID>) -> Option<lang::ID> {
        let code_node_id = code_node_id?;
        let containing_block_expression_id =
            self.code_genie
                .find_expression_inside_block_that_contains(code_node_id)?;
        let position_inside_block_expression =
            self.code_genie
                .find_node(containing_block_expression_id)?
                .self_with_all_children_dfs()
                .filter(|cn| self.is_navigatable(cn))
                .position(|child_node| child_node.id() == code_node_id)?;

        let block = self.code_genie
                        .find_parent(containing_block_expression_id)?
                        .into_block()?;
        let position_of_block_expression_inside_block =
            block.find_position(containing_block_expression_id)?;

        let previous_position_inside_block =
            position_of_block_expression_inside_block.checked_sub(1)
                                                     .unwrap_or(0);
        let previous_block_expression = block.expressions.get(previous_position_inside_block)?;

        let expressions_in_previous_block_expression_up_to_our_index =
            previous_block_expression.self_with_all_children_dfs()
                                     .filter(|cn| self.is_navigatable(cn))
                                     .take(position_inside_block_expression + 1)
                                     .collect_vec();

        let expression_in_previous_block_expression_with_same_or_latest_index_id =
            expressions_in_previous_block_expression_up_to_our_index.get(position_inside_block_expression)
                .or_else(|| expressions_in_previous_block_expression_up_to_our_index.last())?;
        Some(expression_in_previous_block_expression_with_same_or_latest_index_id.id())
    }

    pub fn navigate_down_from(&self, code_node_id: Option<lang::ID>) -> Option<lang::ID> {
        // if nothing's selected and you try going down, let's just go to the first selectable node
        if code_node_id.is_none() {
            return self.navigate_forward_from(code_node_id);
        }
        let code_node_id = code_node_id.unwrap();
        let containing_block_expression_id =
            self.code_genie
                .find_expression_inside_block_that_contains(code_node_id)?;
        let position_inside_block_expression =
            self.code_genie
                .find_node(containing_block_expression_id)?
                .self_with_all_children_dfs()
                .filter(|cn| self.is_navigatable(cn))
                .position(|child_node| child_node.id() == code_node_id)?;

        let block = self.code_genie
                        .find_parent(containing_block_expression_id)?
                        .into_block()?;
        let position_of_block_expression_inside_block =
            block.find_position(containing_block_expression_id)?;
        let previous_position_inside_block =
            position_of_block_expression_inside_block.checked_add(1)
                                                     .unwrap_or(block.expressions.len() - 1);
        let previous_block_expression = block.expressions.get(previous_position_inside_block)?;

        let expressions_in_previous_block_expression_up_to_our_index =
            previous_block_expression.self_with_all_children_dfs()
                                     .filter(|cn| self.is_navigatable(cn))
                                     .take(position_inside_block_expression + 1)
                                     .collect_vec();

        let expression_in_previous_block_expression_with_same_or_latest_index_id =
            expressions_in_previous_block_expression_up_to_our_index.get(position_inside_block_expression)
                .or_else(|| expressions_in_previous_block_expression_up_to_our_index.last())?;
        Some(expression_in_previous_block_expression_with_same_or_latest_index_id.id())
    }

    pub fn navigate_back_from(&self, code_node_id: Option<lang::ID>) -> Option<lang::ID> {
        if code_node_id.is_none() {
            return None;
        }
        let mut go_back_from_id = code_node_id.unwrap();
        while let Some(prev_node) = self.prev_node_from(go_back_from_id) {
            if self.is_navigatable(prev_node) {
                return Some(prev_node.id());
            } else {
                go_back_from_id = prev_node.id()
            }
        }
        None
    }

    pub fn navigate_forward_from(&self, code_node_id: Option<lang::ID>) -> Option<lang::ID> {
        let mut go_back_from_id = code_node_id;
        while let Some(prev_node) = self.next_node_from(go_back_from_id) {
            if self.is_navigatable(prev_node) {
                return Some(prev_node.id());
            } else {
                go_back_from_id = Some(prev_node.id())
            }
        }
        None
    }

    fn prev_node_from(&self, code_node_id: lang::ID) -> Option<&lang::CodeNode> {
        let parent = self.code_genie.find_parent(code_node_id);
        if parent.is_none() {
            return None;
        }
        let parent = parent.unwrap();
        // first try the previous sibling
        if let Some(previous_sibling) = parent.previous_child(code_node_id) {
            // but since we're going back, if the previous sibling has children, then let's
            // select the last one. that feels more ergonomic while moving backwards
            let children = previous_sibling.all_children_dfs();
            if children.len() > 0 {
                return Some(children[children.len() - 1]);
            } else {
                return Some(previous_sibling);
            }
        }

        // if there is no previous sibling, try the parent
        Some(parent)
    }

    fn next_node_from(&self, code_node_id: Option<lang::ID>) -> Option<&lang::CodeNode> {
        if code_node_id.is_none() {
            return Some(self.code_genie.root());
        }

        let selected_node_id = code_node_id.unwrap();
        let selected_code = self.code_genie.find_node(selected_node_id).unwrap();
        let children = selected_code.children();
        let first_child = children.get(0);

        // if the selected node has children, then return the first child. depth first
        if let Some(first_child) = first_child {
            return Some(first_child);
        }

        let mut node_id_to_find_next_sibling_of = selected_node_id;
        while let Some(parent) = self.code_genie.find_parent(node_id_to_find_next_sibling_of) {
            if let Some(next_sibling) = parent.next_child(node_id_to_find_next_sibling_of) {
                return Some(next_sibling);
            }
            // if there is no sibling, then try going to the next sibling of the parent, recursively
            node_id_to_find_next_sibling_of = parent.id()
        }
        None
    }

    // navigation entails moving forward and backwards with the cursor, using the keyboard. i'd like
    // for this keyboard based navigation to feel ergonomic, so when you're navigating through items,
    // the cursor doesn't get stuck on elements that you didn't really care to navigate to. therefore
    // i've arrived at the following rules:
    fn is_navigatable(&self, code_node: &lang::CodeNode) -> bool {
        let parent = self.code_genie.find_parent(code_node.id());

        match (code_node, parent) {
            // if you've assigned something, you definitely want to change what's assigned.
            (_, Some(CodeNode::Assignment(_))) => true,
            // skip entire code blocks: you want to navigate individual elements, and entire codeblocks are
            // huge chunks of code
            (CodeNode::Block(_), _) => false,
            // you always want to be able to edit the name of an assignment
            (CodeNode::Assignment(_), _) => true,
            // instead of navigating over the entire function call, you want to navigate through its
            // innards. that is, the function reference (so you can change the function that's being
            // referred to), or the holes (arguments)
            (CodeNode::FunctionCall(_), _) => false,
            (CodeNode::FunctionReference(_), _) => true,
            // you always want to navigate to a list index
            (CodeNode::ListIndex(_), _) => true,
            // or a struct field get
            (CodeNode::StructFieldGet(_), _) => true,
            // skip elements with holes. function args and struct literal fields always contain inner elements
            // that can be changed. to change those, we can always invoke `r` (replace), which will
            // let you edit the value of the hole
            (CodeNode::Argument(_), _) | (CodeNode::StructLiteralField(_), _) => false,
            // you always want to move to literals
            (CodeNode::StringLiteral(_), _)
            | (CodeNode::NullLiteral(_), _)
            | (CodeNode::StructLiteral(_), _)
            | (CodeNode::ListLiteral(_), _)
            | (CodeNode::NumberLiteral(_), _) => true,
            // if our parent is one of these, then we're a hole, and therefore navigatable.
            (_, Some(CodeNode::Argument(_)))
            | (_, Some(CodeNode::StructLiteralField(_)))
            | (_, Some(CodeNode::ListLiteral(_)))
            | (_, Some(CodeNode::Match(_)))
            | (_, Some(CodeNode::Conditional(_))) => true,
            // we should be able to navigate to the index section of a ListIndex
            (cn, Some(CodeNode::ListIndex(lang::ListIndex { box index_expr, .. })))
                if { index_expr.id() == cn.id() } =>
            {
                true
            }
            // sometimes these scalary things hang out by themselves in blocks
            // TODO: do i really have to except all of these individually? maybe there's a more
            // general solution? maybe using code genie i can say: if node's parent is block and
            // node has no navigatable children, then it's navigatable.
            (CodeNode::Placeholder(_), Some(CodeNode::Block(_))) => true,
            (CodeNode::VariableReference(_), Some(CodeNode::Block(_))) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
struct MutationMaster {
    history: RefCell<undo::UndoHistory>,
}

impl MutationMaster {
    fn new() -> Self {
        MutationMaster { history: RefCell::new(undo::UndoHistory::new()) }
    }

    fn insert_code(&self,
                   node_to_insert: &lang::CodeNode,
                   insertion_point: InsertionPoint,
                   genie: &CodeGenie)
                   -> lang::CodeNode {
        let node_to_insert = node_to_insert.clone();
        match insertion_point {
            InsertionPoint::BeginningOfBlock(block_id) => {
                self.insertion_expression_in_beginning_of_block(block_id, node_to_insert, genie)
            }
            InsertionPoint::Before(id) | InsertionPoint::After(id) => {
                let parent =
                    genie.find_parent(id)
                         .expect("unable to insert new code, couldn't find parent to insert into");
                self.insert_new_expression_in_block(node_to_insert,
                                                    insertion_point,
                                                    parent.clone(),
                                                    genie)
            }
            InsertionPoint::StructLiteralField(struct_literal_field_id) => {
                self.insert_expression_into_struct_literal_field(node_to_insert,
                                                                 struct_literal_field_id,
                                                                 genie)
            }
            InsertionPoint::ListLiteralElement { list_literal_id,
                                                 pos, } => {
                self.insertion_expression_into_list_literal(node_to_insert,
                                                            list_literal_id,
                                                            pos,
                                                            genie)
            }
            InsertionPoint::Replace(node_id_to_replace)
            | InsertionPoint::Wrap(node_id_to_replace) => {
                self.replace_node(node_to_insert, node_id_to_replace, genie)
            }
            // TODO: perhaps we should have edits go through this codepath as well!
            InsertionPoint::Editing(_) => panic!("this is currently unused"),
        }
    }

    fn insertion_expression_into_list_literal(&self,
                                              node_to_insert: lang::CodeNode,
                                              list_literal_id: lang::ID,
                                              pos: usize,
                                              genie: &CodeGenie)
                                              -> lang::CodeNode {
        let mut list_literal = genie.find_node(list_literal_id)
                                    .unwrap()
                                    .into_list_literal()
                                    .clone();
        list_literal.elements.insert(pos, node_to_insert);
        let mut root = genie.root().clone();
        root.replace(lang::CodeNode::ListLiteral(list_literal));
        root
    }

    fn replace_node(&self,
                    code_node: lang::CodeNode,
                    node_id_to_replace: lang::ID,
                    genie: &CodeGenie)
                    -> lang::CodeNode {
        let mut root = genie.root().clone();
        root.replace_with(node_id_to_replace, code_node);
        root
    }

    fn insert_expression_into_struct_literal_field(&self,
                                                   code_node: lang::CodeNode,
                                                   struct_literal_field_id: lang::ID,
                                                   genie: &CodeGenie)
                                                   -> lang::CodeNode {
        let mut struct_literal_field = genie.find_node(struct_literal_field_id)
                                            .unwrap()
                                            .into_struct_literal_field()
                                            .unwrap()
                                            .clone();
        struct_literal_field.expr = Box::new(code_node);
        let mut root = genie.root().clone();
        root.replace(lang::CodeNode::StructLiteralField(struct_literal_field));
        root
    }

    fn insertion_expression_in_beginning_of_block(&self,
                                                  block_id: lang::ID,
                                                  node_to_insert: lang::CodeNode,
                                                  genie: &CodeGenie)
                                                  -> lang::CodeNode {
        let mut block = genie.find_node(block_id)
                             .unwrap()
                             .into_block()
                             .unwrap()
                             .clone();
        block.expressions.insert(0, node_to_insert);
        let mut root = genie.root().clone();
        root.replace(lang::CodeNode::Block(block));
        root
    }

    fn insert_new_expression_in_block(&self,
                                      code_node: lang::CodeNode,
                                      insertion_point: InsertionPoint,
                                      parent: lang::CodeNode,
                                      genie: &CodeGenie)
                                      -> lang::CodeNode {
        match parent {
            CodeNode::Block(mut block) => {
                let get_insertion_point = |node_id| {
                    let insertion_point_in_block_exprs =
                        block.expressions.iter().position(|exp| exp.id() == node_id);
                    insertion_point_in_block_exprs.expect("when the fuck does this happen?")
                };

                match insertion_point {
                    InsertionPoint::Before(id) => {
                        let insertion_point = get_insertion_point(id);
                        block.expressions.insert(insertion_point, code_node)
                    }
                    InsertionPoint::After(id) => {
                        let insertion_point = get_insertion_point(id);
                        block.expressions.insert(insertion_point + 1, code_node)
                    }
                    _ => panic!("bad insertion point type for a block: {:?}",
                                insertion_point),
                }

                let mut root = genie.root().clone();
                root.replace(CodeNode::Block(block));
                root
            }
            _ => panic!("should be inserting into type parent, got {:?} instead",
                        parent),
        }
    }

    pub fn delete_code(&self,
                       node_id_to_delete: lang::ID,
                       genie: &CodeGenie,
                       cursor_position: Option<lang::ID>)
                       -> DeletionResult {
        let parent = genie.find_parent(node_id_to_delete);
        if parent.is_none() {
            panic!("idk when this happens, let's take care of this if / when it does")
        }
        let parent = parent.unwrap();

        match parent {
            CodeNode::Block(block) => {
                let mut new_block = block.clone();
                new_block.expressions
                         .retain(|exp| exp.id() != node_id_to_delete);

                let deleted_expression_position_in_block =
                    block.find_position(node_id_to_delete).unwrap();
                let mut new_cursor_position = new_block.expressions
                                                       .get(deleted_expression_position_in_block)
                                                       .map(|code_node| code_node.id());
                // TODO: what to do if there's nothing left in the block?
                if new_cursor_position.is_none() {
                    new_cursor_position =
                        new_block.expressions
                                 .get(deleted_expression_position_in_block.checked_sub(1)
                                                                          .unwrap_or(0))
                                 .map(|code_node| code_node.id());
                }

                let mut new_root = genie.root().clone();
                new_root.replace(CodeNode::Block(new_block));

                DeletionResult::new(new_root, new_cursor_position)
            }
            CodeNode::ListLiteral(list_literal) => {
                let mut new_list_literal = list_literal.clone();
                let deleted_element_position_in_list =
                    list_literal.elements
                                .iter()
                                .position(|e| e.id() == node_id_to_delete)
                                .unwrap();
                new_list_literal.elements
                                .remove(deleted_element_position_in_list);

                let mut new_cursor_position = new_list_literal.elements
                                                              .get(deleted_element_position_in_list)
                                                              .map(|code_node| code_node.id());
                if new_cursor_position.is_none() {
                    new_cursor_position =
                        new_list_literal.elements
                                        .get(deleted_element_position_in_list.checked_sub(1)
                                                                             .unwrap_or(0))
                                        .map(|code_node| code_node.id());
                }
                if new_cursor_position.is_none() {
                    new_cursor_position = Some(list_literal.id)
                }

                let mut new_root = genie.root().clone();
                new_root.replace(CodeNode::ListLiteral(new_list_literal));

                //                self.log_new_mutation(&new_root, new_cursor_position);
                DeletionResult::new(new_root, new_cursor_position)
            }
            _ => DeletionResult::new(genie.root().clone(), cursor_position),
        }
    }

    fn log_new_mutation(&self, new_root: &lang::CodeNode, cursor_position: Option<lang::ID>) {
        self.history
            .borrow_mut()
            .record_previous_state(new_root, cursor_position);
    }

    pub fn undo(&self,
                current_root: &lang::CodeNode,
                cursor_position: Option<lang::ID>)
                -> Option<undo::UndoHistoryCell> {
        self.history
            .borrow_mut()
            .undo(current_root, cursor_position)
    }

    pub fn redo(&self,
                current_root: &lang::CodeNode,
                cursor_position: Option<lang::ID>)
                -> Option<undo::UndoHistoryCell> {
        self.history
            .borrow_mut()
            .redo(current_root, cursor_position)
    }
}

struct DeletionResult {
    new_root: lang::CodeNode,
    new_cursor_position: Option<lang::ID>,
}

impl DeletionResult {
    fn new(new_root: lang::CodeNode, new_cursor_position: Option<lang::ID>) -> Self {
        Self { new_root,
               new_cursor_position }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InsertionPoint {
    BeginningOfBlock(lang::ID),
    Before(lang::ID),
    After(lang::ID),
    StructLiteralField(lang::ID),
    Editing(lang::ID),
    ListLiteralElement {
        list_literal_id: lang::ID,
        pos: usize,
    },
    // TODO: it's possible we can generalize and replace the Argument, StructLiteralField and
    //       ListLiteralElement with Replace
    Replace(lang::ID),
    Wrap(lang::ID),
}

impl InsertionPoint {
    // TODO: move this to the code editor? i mean we only use it in there...
    fn node_id_to_select_when_marking_as_editing(&self) -> Option<lang::ID> {
        match *self {
            InsertionPoint::BeginningOfBlock(_) => None,
            InsertionPoint::Before(_) => None,
            InsertionPoint::After(_) => None,
            InsertionPoint::Replace(_) => None,
            InsertionPoint::Wrap(_) => None,
            InsertionPoint::StructLiteralField(id) => Some(id),
            InsertionPoint::Editing(id) => Some(id),
            // not sure if this is right....
            InsertionPoint::ListLiteralElement { list_literal_id, .. } => Some(list_literal_id),
        }
    }
}

enum PostInsertionAction {
    SelectNode(lang::ID),
    MarkAsEditing(InsertionPoint),
}

fn post_insertion_cursor(inserted_node: &CodeNode, code_genie: &CodeGenie) -> PostInsertionAction {
    if let CodeNode::FunctionCall(function_call) = inserted_node {
        // if we just inserted a function call, then go to the first arg if there is one
        for arg in function_call.args() {
            //let arg_expr_id = function_call.args[0].into_argument().expr.id();
            match &arg.expr {
                box lang::CodeNode::Placeholder(placeholder) => {
                    return PostInsertionAction::MarkAsEditing(InsertionPoint::Replace(placeholder.id))
                },
                _ => (),
            }
        }
        return PostInsertionAction::SelectNode(function_call.id);
    }

    if let CodeNode::StructLiteral(struct_literal) = inserted_node {
        // if we just inserted a function call, then go to the first arg if there is one
        if struct_literal.fields.len() > 0 {
            let id = struct_literal.fields[0].id();
            return PostInsertionAction::MarkAsEditing(InsertionPoint::StructLiteralField(id));
        } else {
            return PostInsertionAction::SelectNode(struct_literal.id);
        }
    }

    // right now i'm implementing both Assignment and ListIndex insertion, and think i found a
    // generic way of autoselecting the child placeholder in each case.
    for child in inserted_node.all_children_dfs_iter() {
        if let Some(placeholder) = child.into_placeholder() {
            return PostInsertionAction::MarkAsEditing(InsertionPoint::Replace(placeholder.id));
        }
    }

    // if we just inserted a function argument or struct literal field, then select the next one if
    // it's a placeholder
    let parent = code_genie.find_parent(inserted_node.id());
    if let Some(CodeNode::Argument(argument)) = parent {
        // if we just finished inserting into a function call argument, and the next argument is
        // a placeholder, then let's insert into that arg!!!!
        if let Some(CodeNode::FunctionCall(function_call)) = code_genie.find_parent(argument.id) {
            let just_inserted_argument_position =
                function_call.args
                             .iter()
                             .position(|arg| arg.id() == argument.id)
                             .unwrap();
            let maybe_next_arg = function_call.args.get(just_inserted_argument_position + 1);
            if let Some(CodeNode::Argument(lang::Argument { expr:
                                                                box CodeNode::Placeholder(placeholder),
                                                            .. })) = maybe_next_arg
            {
                return PostInsertionAction::MarkAsEditing(InsertionPoint::Replace(placeholder.id));
            }
        }
    } else if let Some(CodeNode::StructLiteralField(struct_literal_field)) = parent {
        // if we just finished inserting into a function call argument, and the next argument is
        // a placeholder, then let's insert into that arg!!!!
        if let Some(CodeNode::StructLiteral(struct_literal)) =
            code_genie.find_parent(struct_literal_field.id)
        {
            let just_inserted_argument_position =
                struct_literal.fields
                              .iter()
                              .position(|field| field.id() == struct_literal_field.id)
                              .unwrap();
            let maybe_next_field = struct_literal.fields
                                                 .get(just_inserted_argument_position + 1);
            if let Some(CodeNode::StructLiteralField(lang::StructLiteralField{ expr: box CodeNode::Placeholder(_), id, .. })) = maybe_next_field {
                return PostInsertionAction::MarkAsEditing(InsertionPoint::StructLiteralField(*id))
            }
        }
    }

    // nothing that we can think of to do next, just chill at the insertion point
    PostInsertionAction::SelectNode(inserted_node.id())
}

pub fn get_type_from_list(mut typ: lang::Type) -> Option<lang::Type> {
    if typ.typespec_id != lang::LIST_TYPESPEC.id {
        return None;
    }
    if typ.params.len() != 1 {
        return None;
    }
    Some(typ.params.remove(0))
}

pub fn update_code_in_env(location: CodeLocation,
                          code: lang::CodeNode,
                          cont: &mut Controller,
                          env: &mut ExecutionEnvironment) {
    match location {
        CodeLocation::Function(func_id) => {
            let func = env.find_function(func_id).cloned().unwrap();
            let mut code_function = func.downcast::<code_function::CodeFunction>().unwrap();
            code_function.set_code(code.into_block().unwrap().clone());
            env.add_function(*code_function);
        }
        CodeLocation::Script(_script_id) => {
            let script = scripts::Script { code: code.into_block().unwrap().clone() };
            cont.load_script(script)
        }
        CodeLocation::Test(test_id) => {
            let mut test = cont.get_test(test_id).unwrap().clone();
            test.set_code(code.into_block().unwrap().clone());
        }
        CodeLocation::JSONHTTPClientURLParams(client_id) => {
            let env_genie = EnvGenie::new(&env);
            let mut client = env_genie.get_json_http_client(client_id).unwrap().clone();
            client.gen_url_params = code.into_block().unwrap().clone();
            env.add_function(client);
        }
        CodeLocation::JSONHTTPClientURL(client_id) => {
            let env_genie = EnvGenie::new(&env);
            let mut client = env_genie.get_json_http_client(client_id).unwrap().clone();
            client.gen_url = code.into_block().unwrap().clone();
            env.add_function(client);
        }
        CodeLocation::ChatTrigger(chat_trigger_id) => {
            let env_genie = EnvGenie::new(&env);
            let mut chat_trigger = env_genie.get_chat_trigger(chat_trigger_id).unwrap().clone();
            chat_trigger.code = code.into_block().unwrap().clone();
            env.add_function(chat_trigger);
        }
    }
}
