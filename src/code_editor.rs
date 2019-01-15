use std::cell::RefCell;
use std::rc::Rc;

use itertools::Itertools;

use super::undo;
use super::lang;
use super::editor;:
use super::code_generation;

pub struct CodeEditor {
    genie: CodeGenie,
    editing: bool,
    selected_node_id: Option<lang::ID>,
    insert_code_menu: Option<InsertCodeMenu>,
    mutation_master: MutationMaster,
}

impl CodeEditor {
    pub fn new(code: &lang::CodeNode) -> Self {
        Self {
            genie: CodeGenie::new(code.clone()),
            editing: false,
            selected_node_id: None,
            insert_code_menu: None,
            mutation_master: MutationMaster::new(),
        }
    }

    pub fn id(&self) -> lang::ID {
        self.genie.id()
    }

    pub fn get_code(&self) -> &lang::CodeNode {
        self.genie.root()
    }

    pub fn handle_keypress(&mut self, keypress: editor::Keypress) {
        use super::editor::Key;

        if keypress.key == Key::Escape {
            self.handle_cancel();
            return
        }
        // don't perform any commands when in edit mode
        match (self.editing, keypress.key) {
            (false, Key::K) | (false, Key::UpArrow) => {
                self.try_select_up_one_node()
            },
            (false, Key::J) | (false, Key::DownArrow) => {
                self.try_select_down_one_node()
            },
            (false, Key::B) | (false, Key::LeftArrow) | (false, Key::H) => {
                self.try_select_back_one_node()
            },
            (false, Key::W) | (false, Key::RightArrow) | (false, Key::L) => {
                self.try_select_forward_one_node()
            },
            (false, Key::C) => {
                if let Some(id) = self.selected_node_id {
                    self.mark_as_editing(InsertionPoint::Editing(id));
                }
            },
            (false, Key::D) => {
                self.delete_selected_code();
            },
            (false, Key::A) => {
                self.try_append_in_selected_node();
            },
            (false, Key::R) => {
                if keypress.ctrl && keypress.shift {
                    self.run(&self.loaded_code.as_ref().unwrap().clone());
                } else if keypress.ctrl {
                    self.redo()
                } else {
                    self.try_enter_replace_edit_for_selected_node();
                }
            },
            (false, Key::O) => {
                if keypress.shift {
                    self.set_insertion_point_on_previous_line_in_block()
                } else {
                    self.set_insertion_point_on_next_line_in_block()
                }
            },
            (false, Key::U) => {
                self.undo()
            },
            (false, Key::V) if keypress.shift => {
                self.select_current_line()
            },
            (_, Key::Tab) => {
                self.insert_code_menu.as_mut().map(|menu| menu.select_next());
            }
            _ => {},
        }
    }

    pub fn hide_insert_code_menu(&mut self) {
        self.insert_code_menu = None;
        self.editing = false
    }

    fn handle_cancel(&mut self) {
        self.editing = false;
        if self.insert_code_menu.is_none() { return }
        // TODO: oh fuckkkkk the order these things are in... what the hell, and why?
        // so fragile...
        self.undo();
        self.hide_insert_code_menu()
    }

    fn mark_as_editing(&mut self, insertion_point: InsertionPoint) -> Option<()> {
        self.insert_code_menu = InsertCodeMenu::for_insertion_point(insertion_point,
                                                                    &self.code_genie()?);
        self.save_current_state_to_undo_history();
        self.selected_node_id = insertion_point.selected_node_id();
        self.editing = true;
        Some(())
    }

    fn undo(&mut self) {
        if let Some(history) = self.mutation_master.undo(self.get_code(), self.selected_node_id) {
            self.replace_code(&history.root);
            self.set_selected_node_id(history.cursor_position);
        }
    }

    fn set_selected_node_id(&mut self, code_node_id: Option<ID>) {
        self.selected_node_id = code_node_id;
    }

    fn replace_code(&mut self, code: &lang::CodeNode) {
        self.genie.replace(code)
    }

    fn try_select_up_one_node(&mut self) {
        let navigation = Navigation::new(&self.genie);
        if let Some(node_id) = navigation.navigate_up_from(self.selected_node_id) {
            self.set_selected_node_id(Some(node_id))
        }
    }

    fn try_select_down_one_node(&mut self) {
        let navigation = Navigation::new(&self.genie);
        if let Some(node_id) = navigation.navigate_down_from(self.selected_node_id) {
            self.set_selected_node_id(Some(node_id))
        }
    }

    pub fn try_select_back_one_node(&mut self) {
        let navigation = Navigation::new(&self.genie);
        if let Some(node_id) = navigation.navigate_back_from(self.selected_node_id) {
            self.set_selected_node_id(Some(node_id))
        }
    }

    pub fn try_select_forward_one_node(&mut self) {
        let navigation = Navigation::new(&self.genie);
        if let Some(node_id) = navigation.navigate_forward_from(self.selected_node_id) {
            self.set_selected_node_id(Some(node_id))
        }
    }

    fn try_enter_replace_edit_for_selected_node(&mut self) -> Option<()> {
        match self.genie.find_parent(self.selected_node_id?)? {
            lang::CodeNode::Argument(cn) => {
                self.mark_as_editing(InsertionPoint::Argument(cn.id));
            },
            lang::CodeNode::StructLiteralField(cn) => {
                self.mark_as_editing(InsertionPoint::StructLiteralField(cn.id));
            },
            _ => (),
        }
        Some(())
    }

    fn get_selected_node(&self) -> Option<&lang::CodeNode> {
        self.genie.find_node(self.selected_node_id?)
    }

    fn try_append_in_selected_node(&mut self) -> Option<()> {
        let selected_node = self.get_selected_node()?;
        match selected_node {
            lang::CodeNode::ListLiteral(list_literal) => {
                let insertion_point = InsertionPoint::ListLiteralElement {
                    list_literal_id: list_literal.id,
                    pos: 0
                };
                self.mark_as_editing(insertion_point);
                return Some(());
            }
            _ => ()
        }
        match self.genie.find_parent(selected_node.id())? {
            lang::CodeNode::ListLiteral(list_literal) => {
                let position_of_selected_node = list_literal.elements.iter()
                    .position(|el| el.id() == selected_node.id())?;
                let insertion_point = InsertionPoint::ListLiteralElement {
                    list_literal_id: list_literal.id,
                    pos: position_of_selected_node + 1
                };
                self.mark_as_editing(insertion_point);
                return Some(());
            }
            _ => (),
        }
        Some(())
    }

    // TODO: factor duplicate code between this method and the next
    fn set_insertion_point_on_previous_line_in_block(&mut self) {
        if let Some(expression_id) = self.currently_focused_block_expression() {
            self.mark_as_editing(InsertionPoint::Before(expression_id));
        } else {
            self.hide_insert_code_menu()
        }
    }

    fn set_insertion_point_on_next_line_in_block(&mut self) {
        if let Some(expression_id) = self.currently_focused_block_expression() {
            self.mark_as_editing(InsertionPoint::After(expression_id));
        } else {
            self.hide_insert_code_menu()
        }
    }

    fn currently_focused_block_expression(&self) -> Option<lang::ID> {
        self.genie
            .find_expression_inside_block_that_contains(self.selected_node_id?)
    }
}

// the code genie traverses through the code, giving callers various information
struct CodeGenie {
    code: lang::CodeNode,
}

impl CodeGenie {
    pub fn new(code: lang::CodeNode) -> Self {
        Self { code }
    }

    pub fn replace(&mut self, code: &lang::CodeNode) {
        self.code.replace(code)
    }

    pub fn code_id(&self) -> lang::ID {
        self.code.id()
    }

    pub fn root(&self) -> &lang::CodeNode {
        &self.code
    }

    fn find_expression_inside_block_that_contains(&self, node_id: ID) -> Option<ID> {
        let parent = self.code.find_parent(node_id);
        match parent {
            Some(lang::CodeNode::Block(_)) => Some(node_id),
            Some(parent_node) => self.find_expression_inside_block_that_contains(
                parent_node.id()),
            None => None
        }
    }

    fn find_node(&self, id: lang::ID) -> Option<&lang::CodeNode> {
        self.code.find_node(id)
    }

    fn find_parent(&self, id: lang::ID) -> Option<&lang::CodeNode> {
        self.code.find_parent(id)
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
        let containing_block_expression_id = self.code_genie
            .find_expression_inside_block_that_contains(code_node_id)?;
        let position_inside_block_expression = self.code_genie
            .find_node(containing_block_expression_id)?
            .self_with_all_children_dfs()
            .filter(|cn| self.is_navigatable(cn))
            .position(|child_node| child_node.id() == code_node_id)?;

        let block = self.code_genie.find_parent(containing_block_expression_id)?.into_block()?;
        let position_of_block_expression_inside_block = block.find_position(
            containing_block_expression_id)?;

        let previous_position_inside_block = position_of_block_expression_inside_block
            .checked_sub(1).unwrap_or(0);
        let previous_block_expression = block.expressions
            .get(previous_position_inside_block)?;

        let expressions_in_previous_block_expression_up_to_our_index = previous_block_expression
            .self_with_all_children_dfs()
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
            return self.navigate_forward_from(code_node_id)
        }
        let code_node_id = code_node_id.unwrap();
        let containing_block_expression_id = self.code_genie
            .find_expression_inside_block_that_contains(code_node_id)?;
        let position_inside_block_expression = self.code_genie
            .find_node(containing_block_expression_id)?
            .self_with_all_children_dfs()
            .filter(|cn| self.is_navigatable(cn))
            .position(|child_node| child_node.id() == code_node_id)?;

        let block = self.code_genie.find_parent(containing_block_expression_id)?.into_block()?;
        let position_of_block_expression_inside_block = block.find_position(containing_block_expression_id)?;
        let previous_position_inside_block = position_of_block_expression_inside_block
            .checked_add(1).unwrap_or(block.expressions.len() - 1);
        let previous_block_expression = block.expressions
            .get(previous_position_inside_block)?;

        let expressions_in_previous_block_expression_up_to_our_index = previous_block_expression
            .self_with_all_children_dfs()
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
            return None
        }
        let mut go_back_from_id = code_node_id.unwrap();
        while let Some(prev_node) = self.prev_node_from(go_back_from_id) {
            if self.is_navigatable(prev_node) {
                return Some(prev_node.id())
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
                return Some(prev_node.id())
            } else {
                go_back_from_id = Some(prev_node.id())
            }
        }
        None
    }

    fn prev_node_from(&self, code_node_id: ID) -> Option<&lang::CodeNode> {
        let parent = self.code_genie.find_parent(code_node_id);
        if parent.is_none() {
            return None
        }
        let parent = parent.unwrap();
        // first try the previous sibling
        if let Some(previous_sibling) = parent.previous_child(code_node_id) {
            // but since we're going back, if the previous sibling has children, then let's
            // select the last one. that feels more ergonomic while moving backwards
            let children = previous_sibling.all_children_dfs();
            if children.len() > 0 {
                return Some(children[children.len() - 1])
            } else {
                return Some(previous_sibling)
            }
        }

        // if there is no previous sibling, try the parent
        Some(parent)
    }

    fn next_node_from(&self, code_node_id: Option<lang::ID>) -> Option<&lang::CodeNode> {
        if code_node_id.is_none() {
            return Some(self.code_genie.root())
        }

        let selected_node_id = code_node_id.unwrap();
        let selected_code = self.code_genie.find_node(selected_node_id).unwrap();
        let children = selected_code.children();
        let first_child = children.get(0);

        // if the selected node has children, then return the first child. depth first
        if let Some(first_child) = first_child {
            return Some(first_child)
        }

        let mut node_id_to_find_next_sibling_of = selected_node_id;
        while let Some(parent) = self.code_genie.find_parent(node_id_to_find_next_sibling_of) {
            if let Some(next_sibling) = parent.next_child(node_id_to_find_next_sibling_of) {
                return Some(next_sibling)
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
        use super::lang::CodeNode;

        match code_node {
            // skip entire code blocks: you want to navigate individual elements, and entire codeblocks are
            // huge chunks of code
            CodeNode::Block(_) => false,
            // you always want to be able to edit the name of an assignment
            CodeNode::Assignment(_) => true,
            // instead of navigating over the entire function call, you want to navigate through its
            // innards. that is, the function reference (so you can change the function that's being
            // referred to), or the holes (arguments)
            CodeNode::FunctionCall(_) => false,
            CodeNode::FunctionReference(_) => true,
            // skip holes. function args and struct literal fields always contain inner elements
            // that can be changed. to change those, we can always invoke `r` (replace), which will
            // let you edit the value of the hole
            CodeNode::Argument(_) | CodeNode::StructLiteralField(_) => false,
            // you always want to move to literals
            CodeNode::StringLiteral(_) | CodeNode::NullLiteral | CodeNode::StructLiteral(_)
            | CodeNode::ListLiteral(_) => true,
            _ => match self.code_genie.find_parent(code_node.id()) {
                Some(parent) => {
                    match parent {
                        // if our parent is one of these, then we're a hole, and therefore navigatable.
                        CodeNode::Argument(_) | CodeNode::StructLiteralField(_) | CodeNode::ListLiteral(_) => true,
                        _ => false,
                    }
                }
                None => false
            }
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

    fn insert_code(&self, node_to_insert: &lang::CodeNode, insertion_point: InsertionPoint,
                   genie: &CodeGenie) -> lang::CodeNode {
        let node_to_insert = node_to_insert.clone();
        match insertion_point {
            InsertionPoint::Before(id) | InsertionPoint::After(id) => {
                let parent = genie.find_parent(id)
                    .expect("unable to insert new code, couldn't find parent to insert into");
                self.insert_new_expression_in_block(
                    node_to_insert, insertion_point, parent.clone(), genie)

            }
            InsertionPoint::Argument(argument_id) => {
                self.insert_expression_into_argument(node_to_insert, argument_id, genie)
            },
            InsertionPoint::StructLiteralField(struct_literal_field_id) => {
                self.insert_expression_into_struct_literal_field(node_to_insert, struct_literal_field_id, genie)
            },
            InsertionPoint::ListLiteralElement { list_literal_id, pos } => {
                self.insertion_expression_into_list_literal(node_to_insert, list_literal_id, pos, genie)
            }
            // TODO: perhaps we should have edits go through this codepath as well!
            InsertionPoint::Editing(_) => panic!("this is currently unused")
        }
    }

    fn insertion_expression_into_list_literal(&self, node_to_insert: lang::CodeNode,
                                              list_literal_id: lang::ID, pos: usize,
                                              genie: &CodeGenie) -> lang::CodeNode {
        let mut list_literal = genie.find_node(list_literal_id).unwrap().into_list_literal().clone();
        list_literal.elements.insert(pos, node_to_insert);
        let mut root = genie.root().clone();
        root.replace(&lang::CodeNode::ListLiteral(list_literal));
        root
    }

    fn insert_expression_into_argument(&self, code_node: lang::CodeNode, argument_id: lang::ID,
                                       genie: &CodeGenie) -> lang::CodeNode {
        let mut argument = genie.find_node(argument_id).unwrap().into_argument().clone();
        argument.expr = Box::new(code_node);
        let mut root = genie.root().clone();
        root.replace(&lang::CodeNode::Argument(argument));
        root
    }

    fn insert_expression_into_struct_literal_field(&self, code_node: lang::CodeNode,
                                                   struct_literal_field_id: lang::ID,
                                                   genie: &CodeGenie) -> lang::CodeNode {
        let mut struct_literal_field = genie.find_node(struct_literal_field_id).unwrap()
            .into_struct_literal_field().unwrap().clone();
        struct_literal_field.expr = Box::new(code_node);
        let mut root = genie.root().clone();
        root.replace(&lang::CodeNode::StructLiteralField(struct_literal_field));
        root
    }

    fn insert_new_expression_in_block(&self, code_node: lang::CodeNode,
                                      insertion_point: InsertionPoint,
                                      parent: lang::CodeNode,
                                      genie: &CodeGenie) -> lang::CodeNode {
        use super::lang::CodeNode;
        match parent {
            CodeNode::Block(mut block) => {
                let insertion_point_in_block_exprs = block.expressions.iter()
                    .position(|exp| exp.id() == insertion_point.node_id());
                let insertion_point_in_block_exprs = insertion_point_in_block_exprs
                    .expect("when the fuck does this happen?");

                match insertion_point {
                    InsertionPoint::Before(_) => {
                        block.expressions.insert(insertion_point_in_block_exprs, code_node)
                    },
                    InsertionPoint::After(_) => {
                        block.expressions.insert(insertion_point_in_block_exprs + 1, code_node)
                    },
                    _ => panic!("bad insertion point type for a block: {:?}", insertion_point)
                }

                let mut root = genie.root().clone();
                root.replace(&CodeNode::Block(block));
                root
            },
            _ => panic!("should be inserting into type parent, got {:?} instead", parent)
        }
    }

    pub fn delete_code(&self, node_id_to_delete: lang::ID, genie: &CodeGenie,
                       cursor_position: Option<lang::ID>) -> DeletionResult {
        let parent = genie.find_parent(node_id_to_delete);
        if parent.is_none() {
            panic!("idk when this happens, let's take care of this if / when it does")
        }
        let parent = parent.unwrap();

        use super::lang::CodeNode;
        match parent {
            CodeNode::Block(block) => {
                let mut new_block = block.clone();
                new_block.expressions.retain(|exp| exp.id() != node_id_to_delete);

                let deleted_expression_position_in_block = genie.find_position_in_block(
                    &block, node_id_to_delete).unwrap();
                let mut new_cursor_position = new_block.expressions
                    .get(deleted_expression_position_in_block)
                    .map(|code_node| code_node.id());
                if new_cursor_position.is_none() {
                    new_cursor_position = new_block.expressions
                        .get(deleted_expression_position_in_block - 1)
                        .map(|code_node| code_node.id());
                }

                let mut new_root = genie.root().clone();
                new_root.replace(&CodeNode::Block(new_block));

                DeletionResult::new(new_root, new_cursor_position)
            }
            CodeNode::ListLiteral(list_literal) => {
                let mut new_list_literal = list_literal.clone();
                let deleted_element_position_in_list = list_literal.elements.iter()
                    .position(|e| e.id() == node_id_to_delete).unwrap();
                new_list_literal.elements.remove(deleted_element_position_in_list);

                let mut new_cursor_position = new_list_literal.elements
                    .get(deleted_element_position_in_list)
                    .map(|code_node| code_node.id());
                if new_cursor_position.is_none() {
                    new_cursor_position = new_list_literal.elements
                        .get(deleted_element_position_in_list - 1)
                        .map(|code_node| code_node.id());
                }
                if new_cursor_position.is_none() {
                    new_cursor_position = Some(list_literal.id)
                }

                let mut new_root = genie.root().clone();
                new_root.replace(&CodeNode::ListLiteral(new_list_literal));

//                self.log_new_mutation(&new_root, new_cursor_position);
                DeletionResult::new(new_root, new_cursor_position)
            }
            _ => {
                DeletionResult::new(genie.root().clone(), cursor_position)
            }
        }
    }

    fn log_new_mutation(&self, new_root: &lang::CodeNode, cursor_position: Option<lang::ID>) {
        self.history.borrow_mut().record_previous_state(new_root, cursor_position);
    }

    pub fn undo(&self, current_root: &lang::CodeNode,
                cursor_position: Option<lang::ID>) -> Option<undo::UndoHistoryCell> {
        self.history.borrow_mut().undo(current_root, cursor_position)
    }

    pub fn redo(&self, current_root: &lang::CodeNode,
                cursor_position: Option<lang::ID>) -> Option<undo::UndoHistoryCell> {
        self.history.borrow_mut().redo(current_root, cursor_position)
    }
}

struct DeletionResult {
    new_root: lang::CodeNode,
    new_cursor_position: Option<lang::ID>,
}

impl DeletionResult {
    fn new(new_root: lang::CodeNode, new_cursor_position: Option<lang::ID>) -> Self {
        Self { new_root, new_cursor_position }
    }
}

// TODO: types of insert code generators
// 1: variable
// 2: function call to capitalize
// 3: new string literal
// 4: placeholder

#[derive(Clone, Debug)]
pub struct InsertCodeMenu {
    option_generators: Vec<Box<InsertCodeMenuOptionGenerator>>,
    selected_option_index: isize,
    search_params: CodeSearchParams,
    insertion_point: InsertionPoint,
}

// TODO: could probably have a single interface for generating this menu, inject the insertion point
// into the search params (because we can copy it easily) and then we don't need to box the searchers.
// every searcher can decide for itself if it can fill in that particular insertion point
impl InsertCodeMenu {
    // TODO: we could just construct all the generators and have them switch on insertion
    // point, intead of doing it here... maybe
    pub fn for_insertion_point(insertion_point: InsertionPoint, genie: &CodeGenie) -> Option<Self> {
        match insertion_point {
            InsertionPoint::Before(_) | InsertionPoint::After(_) => {
                Some(Self::new_expression_inside_code_block(insertion_point))
            },
            InsertionPoint::Argument(field_id) | InsertionPoint::StructLiteralField(field_id) => {
                let node = genie.find_node(field_id).unwrap();
                let exact_type = genie.guess_type(node);
                Some(Self::fill_in_field_with_exact_type(exact_type, genie, insertion_point))
            },
            InsertionPoint::Editing(_) => None,
            InsertionPoint::ListLiteralElement { list_literal_id, .. } => {
                let list_literal = genie.find_node(list_literal_id).unwrap();
                match list_literal {
                    CodeNode::ListLiteral(list_literal) => {
                        Some(Self::fill_in_field_with_exact_type(
                            list_literal.element_type.clone(),
                            genie,
                            insertion_point))
                    }
                    _ => panic!("should always be a list literal... ugh"),
                }
            }
        }
    }

    fn new_expression_inside_code_block(insertion_point: InsertionPoint) -> Self {
        Self {
            // TODO: should probably be able to insert new assignment expressions as well
            option_generators: vec![
                Box::new(InsertFunctionOptionGenerator {}),
                Box::new(InsertLiteralOptionGenerator {}),
                Box::new(InsertConditionalOptionGenerator {}),
            ],
            selected_option_index: 0,
            search_params: CodeSearchParams::empty(),
            insertion_point,
        }
    }

    fn fill_in_field_with_exact_type(exact_type: lang::Type, genie: &CodeGenie,
                                     insertion_point: InsertionPoint) -> Self {
        Self {
            option_generators: vec![
                Box::new(InsertVariableReferenceOptionGenerator {
                    // TODO: this is gonna bite me in the ass, but w/e
                    insertion_id: insertion_point.node_id()
                }),
                Box::new(InsertFunctionOptionGenerator {}),
                Box::new(InsertLiteralOptionGenerator {}),
            ],
            selected_option_index: 0,
            search_params: CodeSearchParams::with_type(&exact_type),
            insertion_point,
        }
    }

    fn selected_option_code(&self, code_genie: &CodeGenie) -> Option<CodeNode> {
        let all_options = self.list_options(code_genie);
        if all_options.is_empty() {
            return None
        }
        let selected_index = self.selected_index(all_options.len());
        Some(all_options.get(selected_index)?.new_node.clone())
    }

    fn select_next(&mut self) {
        // this could possibly overflow, but i wouldn't count on it... HAXXXXX
        self.selected_option_index += 1;
    }

    fn search_str(&self) -> &str {
        &self.search_params.input_str
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
    fn list_options(&self, code_genie: &CodeGenie) -> Vec<InsertCodeMenuOption> {
        let mut all_options : Vec<InsertCodeMenuOption> = self.option_generators
            .iter()
            .flat_map(|generator| {
                generator.options(&self.search_params, code_genie)
            })
            .collect();
        if all_options.is_empty() {
            return all_options
        }
        let selected_index = self.selected_index(all_options.len());
        all_options.get_mut(selected_index).as_mut()
            .map(|option| option.is_selected = true);
        all_options
    }

    // HACK: this modulo stuff is an insane hack but it lets me not have to pass a code genie into
    // select_next
    fn selected_index(&self, num_options: usize) -> usize {
        (self.selected_option_index % num_options as isize) as usize
    }
}

trait InsertCodeMenuOptionGenerator : objekt::Clone {
    fn options(&self, search_params: &CodeSearchParams, code_genie: &CodeGenie) -> Vec<InsertCodeMenuOption>;
}

clone_trait_object!(InsertCodeMenuOptionGenerator);

// TODO: should the insertion point go inside here as well? that way we wouldn't have to store off
// the ID in the variable reference searcher
#[derive(Clone, Debug)]
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

    pub fn lowercased_trimmed_search_str(&self) -> String {
        self.input_str.trim().to_lowercase()
    }

    pub fn list_of_something(&self) -> Option<String> {
        let input_str = self.lowercased_trimmed_search_str();
        if input_str.starts_with("list") {
            Some(input_str.trim_start_matches("list").trim().into())
        } else {
            None
        }
    }
}

#[derive(Clone)]
struct InsertCodeMenuOption {
    label: String,
    new_node: CodeNode,
    is_selected: bool,
}

#[derive(Clone)]
struct InsertFunctionOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertFunctionOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams, genie: &CodeGenie) -> Vec<InsertCodeMenuOption> {
        let mut functions : &mut Iterator<Item = &Box<Function>> = &mut genie.all_functions();
        let mut a;
        let mut b;

        let input_str = search_params.lowercased_trimmed_search_str();
        if !input_str.is_empty() {
            a = functions
                .filter(|f| {
                    f.name().to_lowercase().contains(&input_str)
                });
            functions = &mut a;
        }

        let return_type = &search_params.return_type;
        if return_type.is_some() {
            b = functions
                .filter(|f| f.returns().matches(return_type.as_ref().unwrap()));
            functions = &mut b;
        }

        functions.map(|func| {
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
    insertion_id: lang::ID,
}

impl InsertCodeMenuOptionGenerator for InsertVariableReferenceOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams,
               genie: &CodeGenie) -> Vec<InsertCodeMenuOption> {
        let assignments_by_type_id : HashMap<lang::ID, Vec<lang::Assignment>> = genie.find_assignments_that_come_before_code(self.insertion_id)
            .into_iter()
            .group_by(|assignment| {
                let assignment : Assignment = (**assignment).clone();
                genie.guess_type(&CodeNode::Assignment(assignment)).id()
            })
            .into_iter()
            .map(|(id, assignments)| (id, assignments.cloned().collect::<Vec<Assignment>>()))
            .collect();

        let mut assignments = if let Some(search_type) = &search_params.return_type {
            // XXX: this won't work for generics i believe
            assignments_by_type_id.get(&search_type.id()).map_or_else(
                || vec![],
                |assignments| assignments.iter()
                    .map(|assignment| assignment.clone()).collect()
            )
        } else {
            assignments_by_type_id.iter()
                .flat_map(|(_id, assignments)| assignments)
                .map(|assignment| assignment.clone())
                .collect()
        };

        let input_str = search_params.lowercased_trimmed_search_str();
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
    fn options(&self, search_params: &CodeSearchParams, genie: &CodeGenie) -> Vec<InsertCodeMenuOption> {
        let mut options = vec![];
        let input_str = &search_params.lowercased_trimmed_search_str();
        let return_type = &search_params.return_type;
        if let Some(ref return_type) = search_params.return_type {
            if return_type.matches_spec(&lang::STRING_TYPESPEC) {
                options.push(self.string_literal_option(input_str));
            } else if return_type.matches_spec(&lang::NULL_TYPESPEC) {
                options.push(self.null_literal_option());
            } else if return_type.matches_spec(&lang::LIST_TYPESPEC) {
                options.push(self.list_literal_option(genie, &return_type));
            } else if let Some(strukt) = genie.find_struct(return_type.typespec_id) {
                options.push(self.strukt_option(strukt));
            }

            // design decision made here: all placeholders have types. therefore, it is now
            // required for a placeholder node to have a type, meaning we need to know what the
            // type of a placeholder is to create it. under current conditions that's ok, but i
            // think we can make this less restrictive in the future if we need to
            options.push(self.placeholder_option(input_str, return_type));
        } else {
            if let Some(list_search_query) = search_params.list_of_something() {
                let matching_list_type_options = genie
                    .find_types_matching(&list_search_query)
                    .map(|t| {
                        let list_type = lang::Type::with_params(&*lang::LIST_TYPESPEC, vec![t]);
                        self.list_literal_option(genie, &list_type)
                    });
                options.extend(matching_list_type_options)
            }
            if "null".contains(input_str) {
                options.push(self.null_literal_option())
            }
            if !input_str.is_empty() {
                options.push(self.string_literal_option(input_str));
            }
        }
        options
    }
}

impl InsertLiteralOptionGenerator {
    fn string_literal_option(&self, input_str: &str) -> InsertCodeMenuOption {
        InsertCodeMenuOption {
            label: format!("\u{f10d}{}\u{f10e}", input_str),
            is_selected: false,
            new_node: code_generation::new_string_literal(input_str)
        }
    }


    fn null_literal_option(&self) -> InsertCodeMenuOption {
        InsertCodeMenuOption {
            label: lang::NULL_TYPESPEC.symbol.clone(),
            is_selected: false,
            new_node: code_generation::new_null_literal(),
        }
    }

    fn strukt_option(&self, strukt: &structs::Struct) -> InsertCodeMenuOption {
        InsertCodeMenuOption {
            label: format!("{} {}", strukt.symbol, strukt.name),
            is_selected: false,
            new_node: code_generation::new_struct_literal_with_placeholders(strukt),
        }
    }

    fn placeholder_option(&self, input_str: &str, return_type: &lang::Type) -> InsertCodeMenuOption {
        InsertCodeMenuOption {
            label: format!("{} {}", PLACEHOLDER_ICON, input_str),
            is_selected: false,
            new_node: code_generation::new_placeholder(input_str, return_type.clone()),
        }
    }

    fn list_literal_option(&self, genie: &CodeGenie, list_literal_type: &lang::Type) -> InsertCodeMenuOption {
        let symbol = genie.get_symbol_for_type(list_literal_type);
        let element_type = &list_literal_type.params[0];
        let ts = genie.get_typespec(element_type.typespec_id).unwrap();
        InsertCodeMenuOption {
            label: format!("{}: {}", symbol, ts.readable_name()),
            is_selected: false,
            new_node: lang::CodeNode::ListLiteral(lang::ListLiteral {
                id: lang::new_id(),
                element_type: element_type.clone(),
                elements: vec![]
            })
        }
    }
}

#[derive(Clone)]
struct InsertConditionalOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertConditionalOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams, genie: &CodeGenie) -> Vec<InsertCodeMenuOption> {
        let mut options = vec![];
        let search_str = search_params.lowercased_trimmed_search_str();
        if "if".contains(&search_str) || "conditional".contains(&search_str) {
            options.push(
                InsertCodeMenuOption {
                    label: "If".to_string(),
                    is_selected: false,
                    new_node: code_generation::new_conditional(&search_params.return_type)
                }
            )
        }
        options
    }
}

#[derive(Debug, Clone, Copy)]
pub enum InsertionPoint {
    Before(lang::ID),
    After(lang::ID),
    Argument(lang::ID),
    StructLiteralField(lang::ID),
    Editing(lang::ID),
    ListLiteralElement { list_literal_id: lang::ID, pos: usize },
}

impl InsertionPoint {
    // the purpose of this method is unclear therefore it's dangerous. remove this in a refactoring
    // because it's not really widely used
    fn node_id(&self) -> lang::ID {
        match *self {
            InsertionPoint::Before(id) => id,
            InsertionPoint::After(id) => id,
            InsertionPoint::Argument(id) => id,
            InsertionPoint::StructLiteralField(id) => id,
            InsertionPoint::Editing(id) => id,
            InsertionPoint::ListLiteralElement { list_literal_id, .. } => {
                list_literal_id
            },
        }
    }

    fn selected_node_id(&self) -> Option<lang::ID> {
        match *self {
            InsertionPoint::Before(id) => None,
            InsertionPoint::After(id) => None,
            InsertionPoint::Argument(id) => Some(id),
            InsertionPoint::StructLiteralField(id) => Some(id),
            InsertionPoint::Editing(id) => Some(id),
            // not sure if this is right....
            InsertionPoint::ListLiteralElement { list_literal_id, .. } => Some(list_literal_id),
        }
    }
}
