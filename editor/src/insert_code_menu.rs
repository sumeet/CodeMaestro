use super::code_editor::get_type_from_list;
use super::code_editor::CodeGenie;
use super::code_editor::InsertionPoint;
use super::code_generation;
use cs::env_genie::EnvGenie;
use cs::lang;
use cs::structs;

use itertools::Itertools;
use lazy_static::lazy_static;
use objekt::clone_trait_object;

use cs::chat_program::ChatProgram;
use std::collections::HashMap;

lazy_static! {
    // the order is significant here. it defines which order the options appear in (no weighting
    // system yet)
    static ref OPTIONS_GENERATORS : Vec<Box<dyn InsertCodeMenuOptionGenerator + Send + Sync>> = vec![
        Box::new(InsertFunctionWrappingOptionGenerator {}),
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

const FUNCTION_CALL_GROUP: &str = "Functions";
const LOCALS_GROUP: &str = "Local variables";
const LITERALS_GROUP: &str = "Create new value";
const CONTROL_FLOW_GROUP: &str = "Control flow";
const ASSIGN_VARIABLE_GROUP: &str = "New local variable";

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
            _ => Some(Self { input_str: "".to_string(),
                             selected_option_index: 0,
                             insertion_point }),
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
            return 0;
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

    pub fn selected_option_code(&self,
                                code_genie: &CodeGenie,
                                env_genie: &EnvGenie)
                                -> Option<lang::CodeNode> {
        let options_groups = self.grouped_options(code_genie, env_genie);
        let mut all_options = options_groups.into_iter()
                                            .flat_map(|og| og.options)
                                            .collect::<Vec<_>>();
        let selected_index = self.selected_index(all_options.len());
        if all_options.len() >= selected_index + 1 {
            Some(all_options.swap_remove(selected_index).new_node)
        } else {
            None
        }
    }

    pub fn grouped_options<'a>(&'a self,
                               code_genie: &'a CodeGenie,
                               env_genie: &'a EnvGenie)
                               -> Vec<InsertCodeMenuOptionsGroup> {
        let all_options = self.list_options(code_genie, env_genie);

        let mut options_groups = vec![];
        let selected_index = self.selected_index(all_options.len());
        for (group_name, options) in all_options.into_iter()
                                                .group_by(|o| o.group_name)
                                                .into_iter()
        {
            let mut options = options.collect::<Vec<_>>();
            // TODO: sorting should actually be decided by weights... but this will at least
            // keep the sorting order stable so the menu doesn't flicker
            options.sort_by(|a, b| a.sort_key.cmp(&b.sort_key));
            options_groups.push(InsertCodeMenuOptionsGroup { group_name,
                                                             options });
        }

        // then, sort the groups of options by the lowest option in each group
        options_groups.sort_by(|a, b| {
                          a.earliest_sort_key()
                           .unwrap()
                           .cmp(b.earliest_sort_key().unwrap())
                      });

        // then set the selected option
        options_groups.iter_mut()
                      .flat_map(|og| &mut og.options)
                      .nth(selected_index)
                      .map(|o| o.is_selected = true);

        options_groups
    }

    // TODO: i think the selected option index can get out of sync with this generated list, leading
    // to a panic, say if someone types something and changes the number of options without changing
    // the selected index.
    // TODO: can we return iterators all the way down instead of vectors? pretty sure we can!
    pub fn list_options(&self,
                        code_genie: &CodeGenie,
                        env_genie: &EnvGenie)
                        -> Vec<InsertCodeMenuOption> {
        let search_params = self.search_params(code_genie, env_genie);
        OPTIONS_GENERATORS.iter()
                          .flat_map(|generator| {
                              generator.options(&search_params, code_genie, env_genie)
                          })
                          .collect()
    }

    pub fn search_params(&self, code_genie: &CodeGenie, env_genie: &EnvGenie) -> CodeSearchParams {
        match self.insertion_point {
            // TODO: if it's the last line of a function, we might wanna use the function's type...
            // but that could be too limiting
            InsertionPoint::Before(_)
            | InsertionPoint::After(_)
            | InsertionPoint::BeginningOfBlock(_) => self.new_params(None),
            InsertionPoint::StructLiteralField(field_id) => {
                let node = code_genie.find_node(field_id).unwrap();
                let exact_type = code_genie.guess_type(node, env_genie);
                self.new_params(Some(exact_type))
            }
            InsertionPoint::Replace(node_id_to_replace) => {
                let node = code_genie.find_node(node_id_to_replace).unwrap();
                let exact_type = code_genie.guess_type(node, env_genie);
                let parent = code_genie.find_parent(node.id());
                if let Some(lang::CodeNode::Assignment(assignment)) = parent {
                    // if we're replacing the value of an assignment statement, and that assignment
                    // isn't being used anywhere, then we could change the type to anything. so don't
                    // require a type when searching for nodes
                    if !code_genie.any_variable_referencing_assignment(assignment.id) {
                        return self.new_params(None);
                    }
                }
                self.new_params(Some(exact_type))
            }
            InsertionPoint::Wrap(node_id_to_wrap) => {
                let node = code_genie.find_node(node_id_to_wrap).unwrap();
                let wrapped_node_type = code_genie.guess_type(node, env_genie);
                let exact_type = code_genie.guess_type(node, env_genie);
                let parent = code_genie.find_parent(node.id());
                if let Some(lang::CodeNode::Assignment(assignment)) = &parent {
                    // if we're replacing the value of an assignment statement, and that assignment
                    // isn't being used anywhere, then we could change the type to anything. so don't
                    // require a type when searching for nodes
                    if !code_genie.any_variable_referencing_assignment(assignment.id) {
                        return self.new_params(None).wraps_type(wrapped_node_type);
                    }
                }
                // block expressions (TODO??: except unless they're the last method) can safely be
                // replaced by any type because it's impossible for them to be referenced by anything
                if code_genie.is_block_expression(node_id_to_wrap) {
                    return self.new_params(None).wraps_type(wrapped_node_type);
                }
                self.new_params(Some(exact_type))
                    .wraps_type(wrapped_node_type)
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
        CodeSearchParams { input_str: self.input_str.clone(),
                           insertion_point: self.insertion_point,
                           return_type,
                           wraps_type: None }
    }
}

#[derive(Clone, Debug)]
// TODO: pretty sure these could all be references....
pub struct CodeSearchParams {
    pub return_type: Option<lang::Type>,
    pub wraps_type: Option<lang::Type>,
    input_str: String,
    insertion_point: InsertionPoint,
}

impl CodeSearchParams {
    pub fn wraps_type(mut self, typ: lang::Type) -> Self {
        self.wraps_type = Some(typ);
        self
    }

    pub fn lowercased_trimmed_search_str(&self) -> String {
        self.input_str.trim().to_lowercase()
    }

    // TODO: stil have to replace this in more places
    pub fn search_matches_identifier(&self, identifier: &str) -> bool {
        identifier.to_lowercase()
                  .contains(&self.lowercased_trimmed_search_str())
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

trait InsertCodeMenuOptionGenerator: objekt::Clone {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption>;
}

clone_trait_object!(InsertCodeMenuOptionGenerator);

#[derive(Debug)]
pub struct InsertCodeMenuOptionsGroup {
    pub group_name: &'static str,
    pub options: Vec<InsertCodeMenuOption>,
}

impl InsertCodeMenuOptionsGroup {
    fn earliest_sort_key(&self) -> Option<&str> {
        self.options.iter().map(|o| o.sort_key.as_ref()).min()
    }
}

#[derive(Clone, Debug)]
pub struct InsertCodeMenuOption {
    // TEST
    pub sort_key: String,
    pub new_node: lang::CodeNode,
    pub is_selected: bool,
    pub group_name: &'static str,
}

fn find_wrapped_node<'a>(code_genie: &'a CodeGenie,
                         code_search_params: &CodeSearchParams)
                         -> &'a lang::CodeNode {
    match code_search_params.insertion_point {
        InsertionPoint::Wrap(wrapped_node_id) => code_genie.find_node(wrapped_node_id)
                                                           .expect("couldn't find wrapped node id"),
        _ => panic!("we should've only gotten here and had a wrapped insertion point"),
    }
}

// TODO: it's a mostly copy + paste of InsertFunctionOptionGenerator, can clean it up
#[derive(Clone)]
struct InsertFunctionWrappingOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertFunctionWrappingOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        let wraps_type = search_params.wraps_type.as_ref();
        if wraps_type.is_none() {
            return vec![];
        }
        let wraps_type = wraps_type.unwrap();
        let found_functions = find_functions_ignoring_wraps_type(env_genie, search_params);
        let found_functions =
            found_functions.filter_map(move |f| {
                               let takes_args = f.takes_args();
                               let first_matching_arg =
                                   takes_args.iter()
                                             .find(|arg| arg.arg_type.matches(wraps_type))?;
                               Some((first_matching_arg.id, f))
                           });

        let wrapped_node = find_wrapped_node(code_genie, search_params);

        found_functions.map(|(arg_def_id, func)| {
                           InsertCodeMenuOption {
                new_node: code_generation::new_function_call_with_wrapped_arg(func,
                                                                              arg_def_id,
                                                                              wrapped_node.clone()),
                is_selected: false,
                group_name: FUNCTION_CALL_GROUP,
                               sort_key: func.id().to_string(),
            }
                       })
                       .collect()
    }
}

fn find_functions_ignoring_wraps_type<'a>(
    env_genie: &'a EnvGenie,
    search_params: &'a CodeSearchParams)
    -> impl Iterator<Item = &'a (dyn lang::Function + 'static)> + 'a {
    let input_str = search_params.lowercased_trimmed_search_str();
    let return_type = search_params.return_type.as_ref();
    env_genie.all_functions()
             .filter(move |func| {
                 if input_str.is_empty() {
                     true
                 } else {
                     search_params.search_matches_identifier(&func.name())
                 }
             })
             .filter(move |func| {
                 if return_type.is_none() {
                     true
                 } else {
                     func.returns().matches(return_type.unwrap())
                 }
             })
             // filter out ChatPrograms... we don't want them to show up in autocomplete and possibly
             // TODO don't even want them to be functions
             .filter(|f| f.downcast_ref::<ChatProgram>().is_none())
             .map(|func| func.as_ref())
}

#[derive(Clone)]
struct InsertFunctionOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertFunctionOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               _code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        if search_params.wraps_type.is_some() {
            return vec![];
        }
        let funcs = find_functions_ignoring_wraps_type(env_genie, search_params);
        funcs.map(|func| {
                 InsertCodeMenuOption {
                    new_node: code_generation::new_function_call_with_placeholder_args(func),
                    is_selected: false,
                    group_name: FUNCTION_CALL_GROUP,
                     sort_key: func.id().to_string(),
                }
             })
             .collect()
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
        // after needs to be inclusive, because lang::ID itself could be an assignment expression
        InsertionPoint::After(id) => (id, true),
        InsertionPoint::StructLiteralField(id) => (id, false),
        InsertionPoint::Editing(id) => (id, false),
        InsertionPoint::Replace(id) => (id, false),
        InsertionPoint::ListLiteralElement { list_literal_id, .. } => (list_literal_id, false),
        InsertionPoint::Wrap(id) => (id, false),
    }
}

// shows insertion options for "locals", which are:
// 1. local variables via Assignment
// 2. function arguments
// 3. enum variants if you're inside a match branch
impl InsertCodeMenuOptionGenerator for InsertVariableReferenceOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        // variables don't take arguments and can't wrap anything
        if search_params.wraps_type.is_some() {
            return vec![];
        }

        let mut variables_by_type_id : HashMap<lang::ID, Vec<Variable>> = find_all_locals_preceding(
            search_params.insertion_point, code_genie, env_genie)
            .group_by(|variable| variable.typ.id())
            .into_iter()
            .map(|(id, variables)| (id, variables.collect()))
            .collect();

        let mut variables: Vec<Variable> = if let Some(search_type) = &search_params.return_type {
            variables_by_type_id.remove(&search_type.id())
                                .unwrap_or_else(|| vec![])
        } else {
            Iterator::flatten(variables_by_type_id.drain().map(|(_, v)| v)).collect()
        };

        variables =
            variables.into_iter()
                     .filter(|variable| search_params.search_matches_identifier(&variable.name))
                     .collect();

        variables.into_iter()
                 .map(|variable| {
                     let id = variable.locals_id;
                     InsertCodeMenuOption { new_node: code_generation::new_variable_reference(id),
                                            sort_key: id.to_string(),
                                            is_selected: false,
                                            group_name: LOCALS_GROUP }
                 })
                 .collect()
    }
}

fn find_all_locals_preceding<'a>(insertion_point: InsertionPoint,
                                 code_genie: &'a CodeGenie,
                                 env_genie: &'a EnvGenie)
                                 -> impl Iterator<Item = Variable> + 'a {
    find_assignments_and_function_args_preceding(insertion_point, code_genie, env_genie)
        .chain(find_enum_variants_preceding(insertion_point, code_genie, env_genie))
}

fn find_assignments_and_function_args_preceding<'a>(insertion_point: InsertionPoint,
                                                    code_genie: &'a CodeGenie,
                                                    env_genie: &'a EnvGenie)
                                                    -> impl Iterator<Item = Variable> + 'a {
    let (insertion_id, is_search_inclusive) = assignment_search_position(insertion_point);
    code_genie.find_assignments_that_come_before_code(insertion_id, is_search_inclusive)
              .into_iter()
              .map(move |assignment| {
                  let assignment_clone: lang::Assignment = (*assignment).clone();
                  let guessed_type =
                      code_genie.guess_type(&lang::CodeNode::Assignment(assignment_clone),
                                            env_genie);
                  Variable { locals_id: assignment.id,
                             typ: guessed_type,
                             name: assignment.name.clone() }
              })
              .chain(env_genie.code_takes_args(code_genie.root().id())
                              .map(|arg| Variable { locals_id: arg.id,
                                                    typ: arg.arg_type,
                                                    name: arg.short_name }))
}

fn find_enum_variants_preceding<'a>(insertion_point: InsertionPoint,
                                    code_genie: &'a CodeGenie,
                                    env_genie: &'a EnvGenie)
                                    -> impl Iterator<Item = Variable> + 'a {
    let (node_id, _) = assignment_search_position(insertion_point);
    code_genie.find_enum_variants_preceding_iter(node_id, env_genie)
              .map(|match_variant| Variable { locals_id: match_variant.assignment_id(),
                                              typ: match_variant.typ,
                                              name: match_variant.enum_variant.name })
}

#[derive(Clone)]
struct InsertLiteralOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertLiteralOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               _code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        // TODO: struct literals are eligible to wrap if return type and wrap type matches one of
        // the fields... but any other literal probably can't wrap anything
        if search_params.wraps_type.is_some() {
            return vec![];
        }

        let mut options = vec![];
        let input_str = &search_params.input_str;
        if let Some(ref return_type) = search_params.return_type {
            self.generate_options_for_return_type(search_params,
                                                  env_genie,
                                                  &mut options,
                                                  input_str,
                                                  &return_type);
        } else {
            if let Some(list_search_query) = search_params.search_prefix("list") {
                let matching_list_type_options = env_genie
                    .find_types_matching(&list_search_query)
                    .map(|t| {
                        let list_type = lang::Type::with_params(&*lang::LIST_TYPESPEC, vec![t]);
                        self.list_literal_option(&list_type)
                    });
                options.extend(matching_list_type_options)
            }

            // struct literals
            // TODO: need to implement fuzzy matching because struct names sometimes have spaces and
            // lowercasing isn't good enough
            let lowercased_trimmed_search_str = search_params.lowercased_trimmed_search_str();
            let matching_struct_options =
                env_genie.find_public_structs_matching(&lowercased_trimmed_search_str)
                         .map(|strukt| self.strukt_option(strukt));
            options.extend(matching_struct_options);

            // wanna just show all literal options all the time because we want users to be able to
            // discover everything they can do from the menu

            // TODO: wanna boost up null if there's null anywhere
            // XXX: why did i have this conditional commented out before?
            if search_params.search_matches_identifier("null") {
                options.push(self.null_literal_option());
            }

            if let Some(number) = search_params.parse_number_input() {
                options.push(self.number_literal_option(number));
            } else if input_str.is_empty() {
                options.push(self.number_literal_option(0));
            }

            // TODO: maybe boost string literal if there is something entered?
            //            if !input_str.is_empty() {
            options.push(self.string_literal_option(input_str.clone()));
            //            }
        }
        options
    }
}

impl InsertLiteralOptionGenerator {
    fn string_literal_option(&self, input_str: String) -> InsertCodeMenuOption {
        InsertCodeMenuOption { is_selected: false,
                               sort_key: format!("stringliteral{}", input_str),
                               group_name: LITERALS_GROUP,
                               new_node: code_generation::new_string_literal(input_str) }
    }

    fn number_literal_option(&self, number: i128) -> InsertCodeMenuOption {
        InsertCodeMenuOption { is_selected: false,
                               sort_key: format!("numliteral{}", number),
                               group_name: LITERALS_GROUP,
                               new_node: code_generation::new_number_literal(number) }
    }

    fn null_literal_option(&self) -> InsertCodeMenuOption {
        InsertCodeMenuOption { is_selected: false,
                               // want this stupid thing to show up last
                               sort_key: "zzzznullliteral".to_string(),
                               group_name: LITERALS_GROUP,
                               new_node: code_generation::new_null_literal() }
    }

    fn strukt_option(&self, strukt: &structs::Struct) -> InsertCodeMenuOption {
        InsertCodeMenuOption { is_selected: false,
                               sort_key: format!("structliteral{}", strukt.id),
                               group_name: LITERALS_GROUP,
                               new_node:
                                   code_generation::new_struct_literal_with_placeholders(strukt) }
    }

    fn placeholder_option(&self,
                          input_str: String,
                          return_type: &lang::Type)
                          -> InsertCodeMenuOption {
        InsertCodeMenuOption { group_name: LITERALS_GROUP,
                               // placeholder should also show up last, but before null literal
                               sort_key: format!("zzzzplaceholder{}", input_str),
                               is_selected: false,
                               new_node: code_generation::new_placeholder(input_str,
                                                                          return_type.clone()) }
    }

    fn list_literal_option(&self, list_literal_type: &lang::Type) -> InsertCodeMenuOption {
        let element_type = &list_literal_type.params[0];
        InsertCodeMenuOption {
            group_name: LITERALS_GROUP,
            is_selected: false,
            sort_key: format!("listliteral{}", element_type.id()),
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
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               _env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        if !should_insert_block_expression(search_params.insertion_point, code_genie) {
            return vec![];
        }

        if let Some(wraps_type) = search_params.wraps_type.as_ref() {
            // if wrapping a boolean, then we should suggest creating a conditional.
            if wraps_type.matches_spec(&*lang::BOOLEAN_TYPESPEC) {
                return vec![Self::generate_option(search_params)];
            // otherwise we shouldn't pop up in the suggestions
            } else {
                return vec![];
            }
        }

        let mut options = vec![];
        let search_str = search_params.lowercased_trimmed_search_str();
        if "if".contains(&search_str) || "conditional".contains(&search_str) {
            options.push(Self::generate_option(search_params))
        }
        options
    }
}

#[derive(Clone)]
struct InsertMatchOptionGenerator {}

// this inserts match statements for enum local variables
impl InsertCodeMenuOptionGenerator for InsertMatchOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        if !should_insert_block_expression(search_params.insertion_point, code_genie) {
            return vec![];
        }

        if let Some(wraps_type) = search_params.wraps_type.as_ref() {
            return self.new_option_if_enum(env_genie, wraps_type, || {
                           find_wrapped_node(code_genie, search_params).clone()
                       })
                       .into_iter()
                       .collect();
        }

        // pretty sure we want to show matches regardless of whether or not the user typed match...
        //let search_str = search_params.lowercased_trimmed_search_str();
        //
        // though we may want it to go to the top (weight) if someone types match!!!! (if we add a
        // (concept of weights)
        //        if !search_str.starts_with("match") {
        //            return vec![];
        //        }

        let (insertion_id, is_search_inclusive) =
            assignment_search_position(search_params.insertion_point);
        let mut from_old_system =
            code_genie.find_assignments_that_come_before_code(insertion_id, is_search_inclusive)
                      .into_iter()
                      .filter_map(|assignment| {
                          // TODO: also add a method similar to search_matches_identifier for search_prefix
                          // searches
                          if let Some(var_name_to_match) = search_params.search_prefix("match") {
                              if !assignment.name
                                            .to_lowercase()
                                            .contains(var_name_to_match.as_str())
                              {
                                  return None;
                              }
                          }

                          let guessed_type =
                          code_genie.guess_type(&lang::CodeNode::Assignment(assignment.clone()),
                                                env_genie);
                          self.new_option_if_enum(env_genie, &guessed_type, || {
                                  println!("old system, id is {}", assignment.id);
                                  code_generation::new_variable_reference(assignment.id)
                              })
                      })
                      .collect_vec();

        // CURRENTLYWORKINGON: the code underneath here uses find_assignments_that_come_before_node...
        // however, that is too low level of a function. it would be good if we could instead use
        // find_all_locals_preceding, which takes into account enum variants, and function args...
        let from_new_system = find_all_locals_preceding(search_params.insertion_point,
                                                        code_genie,
                                                        env_genie).filter_map(|variable| {
                                  self.new_option_if_enum(env_genie, &variable.typ, || {
                                      println!("new system, id is {}", variable.locals_id);
                                      code_generation::new_variable_reference(variable.locals_id)
                                  })
                              })
                              .collect_vec();
        // from_old_system.extend_from_slice(&from_new_system);
        // from_old_system
        println!("from old system: {:?}", from_old_system);
        println!("from new system: {:?}", from_new_system);

        from_new_system
    }
}

impl InsertMatchOptionGenerator {
    fn new_option_if_enum(&self,
                          env_genie: &EnvGenie,
                          typ: &lang::Type,
                          match_expr: impl FnOnce() -> lang::CodeNode)
                          -> Option<InsertCodeMenuOption> {
        let eneom = env_genie.find_enum(typ.typespec_id)?;

        Some(InsertCodeMenuOption { sort_key: format!("match{}", eneom.id),
                                    group_name: CONTROL_FLOW_GROUP,
                                    is_selected: false,
                                    new_node: code_generation::new_match(eneom,
                                                                         typ,
                                                                         match_expr()) })
    }
}

#[derive(Clone)]
struct InsertAssignmentOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertAssignmentOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               _env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        if !should_insert_block_expression(search_params.insertion_point, code_genie) {
            return vec![];
        }

        let lowercased_trimmed_search_str = search_params.lowercased_trimmed_search_str();

        let variable_name = if let Some(var_alias) = search_params.search_prefix("let") {
            var_alias
        } else {
            lowercased_trimmed_search_str.trim_end_matches(|c| c == '=' || c == ' ')
                                         .to_string()
        };

        let sort_key_prefix = if lowercased_trimmed_search_str.contains('=') {
            // if the user typed a =, then it's very likely they wanted an assignment statement. sort
            // this up to the top, in that case
            "000"
        } else {
            "zzz"
        };

        // don't show this option when there's no variable name typed in!
        if variable_name.is_empty() {
            return vec![];
        }

        vec![InsertCodeMenuOption {
            group_name: ASSIGN_VARIABLE_GROUP,
            sort_key: format!("{}newvariable{}", sort_key_prefix, variable_name),
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
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        // struct field gets are just variables and don't take args or anything, so we can't wrap
        // anything here
        if search_params.wraps_type.is_some() {
            return vec![];
        }

        let optionss = find_all_locals_preceding(search_params.insertion_point,
                                                 code_genie,
                                                 env_genie).filter_map(|variable| {
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
                        group_name: LOCALS_GROUP,
                        sort_key: format!("structfieldget{}", struct_field.id),
                        new_node: code_generation::new_struct_field_get(
                            code_generation::new_variable_reference(variable.locals_id),
                            struct_field.id,
                        ),
                        is_selected: false,
                    })
                }))
                       });
        Iterator::flatten(optionss).collect()
    }
}

#[derive(Clone)]
struct InsertListIndexOfLocal {}

impl InsertCodeMenuOptionGenerator for InsertListIndexOfLocal {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
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
                    group_name: LOCALS_GROUP,
                    sort_key: format!("listindex{}", variable.locals_id),
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
pub fn should_insert_block_expression(insertion_point: InsertionPoint,
                                      code_genie: &CodeGenie)
                                      -> bool {
    match insertion_point {
        InsertionPoint::BeginningOfBlock(_)
        | InsertionPoint::Before(_)
        | InsertionPoint::After(_) => true,
        InsertionPoint::Replace(node_id) | InsertionPoint::Wrap(node_id) => {
            code_genie.is_block_expression(node_id)
        }
        InsertionPoint::StructLiteralField(_)
        | InsertionPoint::Editing(_)
        | InsertionPoint::ListLiteralElement { .. } => false,
    }
}

impl InsertLiteralOptionGenerator {
    fn generate_options_for_return_type(&self,
                                        search_params: &CodeSearchParams,
                                        env_genie: &EnvGenie,
                                        options: &mut Vec<InsertCodeMenuOption>,
                                        input_str: &String,
                                        return_type: &lang::Type) {
        if return_type.matches_spec(&lang::STRING_TYPESPEC) {
            options.push(self.string_literal_option(input_str.clone()));
        } else if return_type.matches_spec(&lang::NULL_TYPESPEC) {
            options.push(self.null_literal_option());
        } else if return_type.matches_spec(&lang::NUMBER_TYPESPEC) {
            if let Some(number) = search_params.parse_number_input() {
                options.push(self.number_literal_option(number));
            }
        } else if return_type.matches_spec(&lang::LIST_TYPESPEC) {
            options.push(self.list_literal_option(&return_type));
        } else if let Some(strukt) = env_genie.find_struct(return_type.typespec_id) {
            options.push(self.strukt_option(strukt));
        }
        // design decision made here: all placeholders have types. therefore, it is now
        // required for a placeholder node to have a type, meaning we need to know what the
        // type of a placeholder is to create it. under current conditions that's ok, but i
        // think we can make this less restrictive in the future if we need to
        options.push(self.placeholder_option(input_str.clone(), return_type));
    }
}

impl InsertConditionalOptionGenerator {
    fn generate_option(search_params: &CodeSearchParams) -> InsertCodeMenuOption {
        InsertCodeMenuOption { sort_key: "conditional".to_string(),
                               group_name: CONTROL_FLOW_GROUP,
                               is_selected: false,
                               new_node:
                                   code_generation::new_conditional(&search_params.return_type) }
    }
}
