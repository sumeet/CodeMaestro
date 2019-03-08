use super::code_editor::InsertionPoint;
use super::env_genie::EnvGenie;
use super::code_editor::CodeGenie;
use super::code_editor::get_type_from_list;
use super::code_editor::PLACEHOLDER_ICON;
use super::lang;
use super::code_generation;
use super::structs;

use downcast_rs::impl_downcast;
use objekt::{clone_trait_object};
use lazy_static::lazy_static;
use itertools::Itertools;

use std::collections::HashMap;

lazy_static! {
    // the order is significant here. it defines which order the options appear in (no weighting
    // system yet)
    static ref OPTIONS_GENERATORS : Vec<Box<InsertCodeMenuOptionGenerator + Send + Sync>> = vec![
        Box::new(InsertListIndexOfLocal {}),
        Box::new(InsertVariableReferenceOptionGenerator {}),
        Box::new(InsertStructFieldGetOfLocal {}),
        Box::new(InsertFunctionOptionGenerator {}),
        Box::new(InsertConditionalOptionGenerator {}),
        Box::new(InsertMatchOptionGenerator {}),
        Box::new(InsertAssignmentOptionGenerator {}),
        Box::new(InsertLiteralOptionGenerator {}),
    ];
}

pub struct InsertCodeMenu {
    input_str: String,
    selected_option_index: isize,
    pub insertion_point: InsertionPoint,
}

impl InsertCodeMenu {
    pub fn for_insertion_point(insertion_point: InsertionPoint) -> Option<Self> {
        match insertion_point {
            // this means you're editing a literal or variable name or smth, so no menu for
            // that (i guess)
            InsertionPoint::Editing(_) => None,
            _ => Some(
                Self { input_str: "".to_string(), selected_option_index: 0, insertion_point }
            )
        }
    }

    pub fn select_next(&mut self) {
        // this could possibly overflow, but i wouldn't count on it... HAXXXXX
        self.selected_option_index += 1;
    }

    pub fn select_prev(&mut self) {
        // this could possibly overflow, but i wouldn't count on it... HAXXXXX
        self.selected_option_index -= 1;
    }

    pub fn input_str(&self) -> &str {
        &self.input_str
    }

    pub fn set_search_str(&mut self, input_str: &str) {
        if input_str != self.input_str {
            self.input_str = input_str.to_string();
            self.selected_option_index = 0;
        }
    }

    // HACK: this modulo stuff is an insane hack but it lets me not have to pass a code genie into
    // select_next
    // XXX: copy and paste to opener.rs
    fn selected_index(&self, num_options: usize) -> usize {
        if num_options == 0 {
            return 0
        }
        let selected = self.selected_option_index % num_options as isize;
        if selected == 0 {
            0
        } else if selected > 0 {
            selected as usize
        } else {
            (num_options as isize + selected) as usize
        }
    }

    pub fn selected_option_code(&self, code_genie: &CodeGenie, env_genie: &EnvGenie) -> Option<lang::CodeNode> {
        let all_options = self.list_options(code_genie, env_genie);
        if all_options.is_empty() {
            return None
        }
        let selected_index = self.selected_index(all_options.len());
        Some(all_options.get(selected_index)?.new_node.clone())
    }

    // TODO: i think the selected option index can get out of sync with this generated list, leading
    // to a panic, say if someone types something and changes the number of options without changing
    // the selected index.
    // TODO: can we return iterators all the way down instead of vectors? pretty sure we can!
    pub fn list_options(&self, code_genie: &CodeGenie, env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        let search_params = self.search_params(code_genie, env_genie);
        let mut all_options : Vec<InsertCodeMenuOption> = OPTIONS_GENERATORS
            .iter()
            .flat_map(|generator| {
                generator.options(&search_params, code_genie, env_genie)
            })
            .collect();
        if all_options.is_empty() {
            return all_options
        }
        let selected_index = self.selected_index(all_options.len());
        all_options.get_mut(selected_index).as_mut()
            .map(|mut option| option.is_selected = true);
        all_options
    }

    fn search_params(&self, code_genie: &CodeGenie, env_genie: &EnvGenie) -> CodeSearchParams {
        match self.insertion_point {
            // TODO: if it's the last line of a function, we might wanna use the function's type...
            // but that could be too limiting
            InsertionPoint::Before(_) | InsertionPoint::After(_) | InsertionPoint::BeginningOfBlock(_) => {
                self.new_params(None)
            },
            InsertionPoint::StructLiteralField(field_id) => {
                let node = code_genie.find_node(field_id).unwrap();
                let exact_type = code_genie.guess_type(node, env_genie);
                self.new_params(Some(exact_type))
            },
            InsertionPoint::Replace(node_id_to_replace) => {
                let node = code_genie.find_node(node_id_to_replace).unwrap();
                let exact_type = code_genie.guess_type(node, env_genie);
                let parent = code_genie.find_parent(node.id());
                if let Some(lang::CodeNode::Assignment(assignment)) = parent {
                    // if we're replacing the value of an assignment statement, and that assignment
                    // isn't being used anywhere, then we could change the type to anything. so don't
                    // require a type when searching for nodes
                    if !code_genie.any_variable_referencing_assignment(assignment.id) {
                        return self.new_params(None)
                    }
                }
                self.new_params(Some(exact_type))
            }
            InsertionPoint::ListLiteralElement { list_literal_id, .. } => {
                let list_literal = code_genie.find_node(list_literal_id).unwrap();
                match list_literal {
                    lang::CodeNode::ListLiteral(list_literal) => {
                        self.new_params(Some(list_literal.element_type.clone()))
                    }
                    _ => panic!("should always be a list literal... ugh"),
                }
            }
            InsertionPoint::Editing(_) => panic!("shouldn't have gotten here"),
        }
    }

    // we don't have to clone that string
    fn new_params(&self, return_type: Option<lang::Type>) -> CodeSearchParams {
        CodeSearchParams {
            input_str: self.input_str.clone(),
            insertion_point: self.insertion_point,
            return_type
        }
    }
}

#[derive(Clone, Debug)]
// TODO: pretty sure these could all be references....
struct CodeSearchParams {
    return_type: Option<lang::Type>,
    input_str: String,
    insertion_point: InsertionPoint,
}

impl CodeSearchParams {
    pub fn lowercased_trimmed_search_str(&self) -> String {
        self.input_str.trim().to_lowercase()
    }

    // TODO: stil have to replace this in more places
    pub fn search_matches_identifier(&self, identifier: &str) -> bool {
        identifier.to_lowercase().contains(&self.lowercased_trimmed_search_str())
    }

    pub fn search_prefix(&self, prefix: &str) -> Option<String> {
        let input_str = self.lowercased_trimmed_search_str();
        if input_str.starts_with(prefix) {
            Some(input_str.trim_start_matches(prefix).trim().into())
        } else {
            None
        }
    }

    pub fn parse_number_input(&self) -> Option<i128> {
        self.lowercased_trimmed_search_str().parse().ok()
    }
}

// TODO: types of insert code generators
// 1: variable
// 2: function call to capitalize
// 3: new string literal
// 4: placeholder

trait InsertCodeMenuOptionGenerator : objekt::Clone + downcast_rs::Downcast {
    fn options(&self, search_params: &CodeSearchParams, code_genie: &CodeGenie,
               env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption>;
}

clone_trait_object!(InsertCodeMenuOptionGenerator);
impl_downcast!(InsertCodeMenuOptionGenerator);

#[derive(Clone, Debug)]
pub struct InsertCodeMenuOption {
    pub label: String,
    pub new_node: lang::CodeNode,
    pub is_selected: bool,
}

#[derive(Clone)]
struct InsertFunctionOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertFunctionOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams, _code_genie: &CodeGenie,
               env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        let mut functions : &mut Iterator<Item = &Box<lang::Function>> = &mut env_genie.all_functions();
        let mut a;
        let mut b;

        let input_str = search_params.lowercased_trimmed_search_str();
        if !input_str.is_empty() {
            a = functions
                .filter(|f| {
                    search_params.search_matches_identifier(&f.name())
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
struct InsertVariableReferenceOptionGenerator {}

struct Variable {
    locals_id: lang::ID,
    typ: lang::Type,
    name: String,
}

// this is used to see which assignments appear before a particular InsertionPoint.
//
// returns tuple -> (CodeNode position, is_inclusive)
fn assignment_search_position(insertion_point: InsertionPoint) -> (lang::ID, bool) {
    match insertion_point {
        InsertionPoint::BeginningOfBlock(id) => (id, false),
        InsertionPoint::Before(id) => (id, false),
        InsertionPoint::After(id) => (id, true),
        InsertionPoint::StructLiteralField(id) => (id, false),
        InsertionPoint::Editing(id) => (id, false),
        InsertionPoint::Replace(id) => (id, false),
        InsertionPoint::ListLiteralElement { list_literal_id, .. } => {
            (list_literal_id, false)
        },
    }
}

// shows insertion options for "locals", which are:
// 1. local variables via Assignment
// 2. function arguments
// 3. enum variants if you're inside a match branch
impl InsertCodeMenuOptionGenerator for InsertVariableReferenceOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams, code_genie: &CodeGenie,
               env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        let mut variables_by_type_id : HashMap<lang::ID, Vec<Variable>> = find_all_locals_preceding(
            search_params.insertion_point, code_genie, env_genie)
            .group_by(|variable| variable.typ.id())
            .into_iter()
            .map(|(id, variables)| (id, variables.collect()))
            .collect();

        let mut variables : Vec<Variable> = if let Some(search_type) = &search_params.return_type {
            variables_by_type_id.remove(&search_type.id()).unwrap_or_else(|| vec![])
        } else {
            Iterator::flatten(variables_by_type_id.drain().map(|(_, v)| v)).collect()
        };

        variables = variables.into_iter()
            .filter(|variable| search_params.search_matches_identifier(&variable.name))
            .collect();

        variables.into_iter().map(|variable| {
            let id = variable.locals_id;
            InsertCodeMenuOption {
                label: variable.name,
                new_node: code_generation::new_variable_reference(id),
                is_selected: false,
            }
        }).collect()
    }
}

fn find_all_locals_preceding<'a>(insertion_point: InsertionPoint, code_genie: &'a CodeGenie,
                                 env_genie: &'a EnvGenie) -> impl Iterator<Item = Variable> + 'a {
    find_assignments_and_function_args_preceding(insertion_point, code_genie, env_genie)
        .chain(find_enum_variants_preceding(insertion_point, code_genie, env_genie))
}


fn find_assignments_and_function_args_preceding<'a>(insertion_point: InsertionPoint,
                                                    code_genie: &'a CodeGenie, env_genie: &'a EnvGenie)
                                                    -> impl Iterator<Item = Variable> + 'a {
    let (insertion_id,
         is_search_inclusive) = assignment_search_position(insertion_point);
    code_genie.find_assignments_that_come_before_code(
        insertion_id, is_search_inclusive)
        .into_iter()
        .map(move |assignment| {
            let assignment_clone : lang::Assignment = (*assignment).clone();
            let guessed_type = code_genie.guess_type(&lang::CodeNode::Assignment(assignment_clone), env_genie);
            Variable { locals_id: assignment.id, typ: guessed_type, name: assignment.name.clone() }
        })
        .chain(
            env_genie.code_takes_args(code_genie.root().id())
                .map(|arg| Variable { locals_id: arg.id, typ: arg.arg_type, name: arg.short_name })
        )
}

fn find_enum_variants_preceding<'a>(insertion_point: InsertionPoint,
                                    code_genie: &'a CodeGenie,
                                    env_genie: &'a EnvGenie) -> impl Iterator<Item = Variable> + 'a {
    let (node_id, _) = assignment_search_position(insertion_point);
    code_genie.find_enum_variants_preceding_iter(node_id, env_genie)
        .map(|match_variant| {
            Variable {
                locals_id: match_variant.assignment_id(),
                typ: match_variant.typ,
                name: match_variant.enum_variant.name,
            }
        })
}

#[derive(Clone)]
struct InsertLiteralOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertLiteralOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams, _code_genie: &CodeGenie,
               env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        let mut options = vec![];
        let input_str = &search_params.input_str;
        if let Some(ref return_type) = search_params.return_type {
            if return_type.matches_spec(&lang::STRING_TYPESPEC) {
                options.push(self.string_literal_option(input_str.clone()));
            } else if return_type.matches_spec(&lang::NULL_TYPESPEC) {
                options.push(self.null_literal_option());
            } else if return_type.matches_spec(&lang::NUMBER_TYPESPEC) {
                if let Some(number) = search_params.parse_number_input() {
                    options.push(self.number_literal_option(number));
                }
            } else if return_type.matches_spec(&lang::LIST_TYPESPEC) {
                options.push(self.list_literal_option(env_genie, &return_type));
            } else if let Some(strukt) = env_genie.find_struct(return_type.typespec_id) {
                options.push(self.strukt_option(strukt));
            }

            // design decision made here: all placeholders have types. therefore, it is now
            // required for a placeholder node to have a type, meaning we need to know what the
            // type of a placeholder is to create it. under current conditions that's ok, but i
            // think we can make this less restrictive in the future if we need to
            options.push(self.placeholder_option(input_str.clone(), return_type));
        } else {
            if let Some(list_search_query) = search_params.search_prefix("list") {
                let matching_list_type_options = env_genie
                    .find_types_matching(&list_search_query)
                    .map(|t| {
                        let list_type = lang::Type::with_params(&*lang::LIST_TYPESPEC, vec![t]);
                        self.list_literal_option(env_genie, &list_type)
                    });
                options.extend(matching_list_type_options)
            }
            if search_params.search_matches_identifier("null") {
                options.push(self.null_literal_option())
            }
            if let Some(number) = search_params.parse_number_input() {
                options.push(self.number_literal_option(number))
            }
            if !input_str.is_empty() {
                options.push(self.string_literal_option(input_str.clone()));
            }
        }
        options
    }
}

impl InsertLiteralOptionGenerator {
    fn string_literal_option(&self, input_str: String) -> InsertCodeMenuOption {
        InsertCodeMenuOption {
            label: format!("\u{f10d}{}\u{f10e}", input_str),
            is_selected: false,
            new_node: code_generation::new_string_literal(input_str)
        }
    }

    fn number_literal_option(&self, number: i128) -> InsertCodeMenuOption {
        InsertCodeMenuOption {
            label: number.to_string(),
            is_selected: false,
            new_node: code_generation::new_number_literal(number)
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

    fn placeholder_option(&self, input_str: String, return_type: &lang::Type) -> InsertCodeMenuOption {
        InsertCodeMenuOption {
            label: format!("{} {}", PLACEHOLDER_ICON, input_str),
            is_selected: false,
            new_node: code_generation::new_placeholder(input_str, return_type.clone()),
        }
    }

    fn list_literal_option(&self, env_genie: &EnvGenie, list_literal_type: &lang::Type) -> InsertCodeMenuOption {
        let symbol = env_genie.get_symbol_for_type(list_literal_type);
        let element_type = &list_literal_type.params[0];
        let ts = env_genie.find_typespec(element_type.typespec_id).unwrap();
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
    fn options(&self, search_params: &CodeSearchParams, code_genie: &CodeGenie,
               _env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        let mut options = vec![];
        if !should_insert_block_expression(search_params.insertion_point, code_genie) {
            return options
        }

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

#[derive(Clone)]
struct InsertMatchOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertMatchOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams, code_genie: &CodeGenie,
               env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        if !should_insert_block_expression(search_params.insertion_point, code_genie) {
            return vec![];
        }

        let search_str = search_params.lowercased_trimmed_search_str();
        if !search_str.starts_with("match") {
            return vec![];
        }
        let (insertion_id, is_search_inclusive) = assignment_search_position(
            search_params.insertion_point);
        code_genie.find_assignments_that_come_before_code(insertion_id, is_search_inclusive)
            .into_iter()
            .filter_map(|assignment| {
                // TODO: also add a method similar to search_matches_identifier for search_prefix
                // searches
                if let Some(var_name_to_match) = search_params.search_prefix("match") {
                    if !assignment.name.to_lowercase().contains(var_name_to_match.as_str()) {
                        return None
                    }
                }

                let guessed_type = code_genie.guess_type(&lang::CodeNode::Assignment(assignment.clone()), env_genie);
                let eneom = env_genie.find_enum(guessed_type.typespec_id)?;

                Some(InsertCodeMenuOption {
                    label: format!("Match {}", assignment.name),
                    is_selected: false,
                    new_node: code_generation::new_match(eneom,
                                                         &guessed_type,
                                                         code_generation::new_variable_reference(assignment.id))
                })
            }).collect()
    }
}

#[derive(Clone)]
struct InsertAssignmentOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertAssignmentOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams, code_genie: &CodeGenie,
               _env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        if !should_insert_block_expression(search_params.insertion_point, code_genie) {
            return vec![];
        }

        let variable_name = if let Some(var_alias) = search_params.search_prefix("let") {
            var_alias
        } else {
            search_params.lowercased_trimmed_search_str().trim_end_matches(|c| c == '=' || c == ' ').to_string()
        };

        vec![InsertCodeMenuOption {
            label: format!("{} =", variable_name),
            is_selected: false,
            new_node: code_generation::new_assignment(
                variable_name.clone(),
                code_generation::new_placeholder(
                    variable_name,
                    lang::Type::from_spec(&*lang::NULL_TYPESPEC)))
        }]
    }
}

#[derive(Clone)]
struct InsertStructFieldGetOfLocal {}

impl InsertCodeMenuOptionGenerator for InsertStructFieldGetOfLocal {
    fn options(&self, search_params: &CodeSearchParams, code_genie: &CodeGenie,
               env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        let optionss = find_all_locals_preceding(
            search_params.insertion_point, code_genie, env_genie)
            .filter_map(|variable| {
                let strukt = env_genie.find_struct(variable.typ.typespec_id)?;

                Some(strukt.fields.iter().filter_map(move |struct_field| {
                    let dotted_name = format!("{}.{}", variable.name, struct_field.name);

                    if !(search_params.search_matches_identifier(&variable.name) ||
                         search_params.search_matches_identifier(&struct_field.name) ||
                         search_params.search_matches_identifier(&dotted_name)) {
                        return None
                    }
                    if let Some(search_type) = &search_params.return_type {
                        if !search_type.matches(&struct_field.field_type) {
                            return None
                        }
                    }
                    Some(InsertCodeMenuOption {
                        label: dotted_name,
                        new_node: code_generation::new_struct_field_get(
                            code_generation::new_variable_reference(variable.locals_id),
                            struct_field.id,
                        ),
                        is_selected: false,
                    })
                }))
            });
        itertools::Itertools::flatten(optionss).collect()
    }
}


#[derive(Clone)]
struct InsertListIndexOfLocal {}

impl InsertCodeMenuOptionGenerator for InsertListIndexOfLocal {
    fn options(&self, search_params: &CodeSearchParams, code_genie: &CodeGenie,
               env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        find_all_locals_preceding(
            search_params.insertion_point, code_genie, env_genie)
            .filter_map(|variable| {
                let list_elem_typ = get_type_from_list(variable.typ)?;
                if let Some(search_type) = &search_params.return_type {
                    if !search_type.matches(&list_elem_typ) {
                        return None
                    }
                }
                if !search_params.search_matches_identifier(&variable.name) {
                    return None
                }
                Some(InsertCodeMenuOption {
                    // TODO: can we add fonts to support these symbols?
                    //label: format!("{}⟦…⟧", variable.name),
                    label: format!("{}[\u{f292}]", variable.name),
                    new_node: code_generation::new_list_index(code_generation::new_variable_reference(
                        variable.locals_id)),
                    is_selected: false
                })
            })
            .collect()
    }
}


// hmmm this is used by code search
// TODO: move into insert_code_menu.rs
pub fn should_insert_block_expression(insertion_point: InsertionPoint, code_genie: &CodeGenie) -> bool {
    match insertion_point {
        InsertionPoint::BeginningOfBlock(_) | InsertionPoint::Before(_) |
            InsertionPoint::After(_) => true,
        InsertionPoint::Replace(node_id_to_replace) => {
            code_genie.is_block_expression(node_id_to_replace)
        }
        InsertionPoint::StructLiteralField(_) |
        InsertionPoint::Editing(_) | InsertionPoint::ListLiteralElement {..} => false,
    }
}
