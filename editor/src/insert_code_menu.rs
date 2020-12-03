use super::code_editor::CodeGenie;
use super::code_editor::InsertionPoint;
use super::code_generation;
use cs::env_genie::EnvGenie;
use cs::structs;
use cs::{enums, lang};

use itertools::Itertools;
use lazy_static::lazy_static;
use objekt::clone_trait_object;
use serde_derive::{Deserialize, Serialize};

use crate::code_editor::{
    get_result_type_from_indexing_into_list, get_type_from_list, locals, required_return_type,
    CodeLocation,
};
use cs::builtins;
use cs::builtins::get_success_type_from_option_or_result_typ;
use cs::chat_program::ChatProgram;
use cs::code_generation::new_anon_func;
use cs::lang::{arg_and_return_typs_for_anon_func, ArgumentDefinition, TypeSpec};

lazy_static! {
    // the order is significant here. it defines which order the options appear in (no weighting
    // system yet)
    static ref OPTIONS_GENERATORS : Vec<Box<dyn InsertCodeMenuOptionGenerator + Send + Sync>> = vec![
        Box::new(InsertFunctionWrappingOptionGenerator {}),
        Box::new(InsertListIndexOfLocal {}),
        Box::new(InsertReassignListIndexOfLocalVariable {}),
        Box::new(InsertVariableReferenceOptionGenerator {}),
        Box::new(InsertStructFieldGetOfLocal {}),
        Box::new(InsertFunctionOptionGenerator {}),
        Box::new(InsertConditionalOptionGenerator {}),
        Box::new(InsertWhileOptionGenerator {}),
        Box::new(InsertForLoopOptionGenerator {}),
        Box::new(InsertMatchOptionGenerator {}),
        Box::new(InsertAssignmentOptionGenerator {}),
        Box::new(InsertReassignmentOptionGenerator {}),
        Box::new(InsertLiteralOptionGenerator {}),
        Box::new(InsertAnonFuncOptionGenerator {}),
        Box::new(InsertEarlyReturnOptionGenerator {}),
        Box::new(InsertTryOptionGenerator {}),
        Box::new(InsertFunctionCallReplacementOptionGenerator {}),
    ];

    static ref UNWRAP_OPTIONS_GENERATORS : Vec<Box<dyn InsertCodeMenuOptionGenerator + Send + Sync>> = vec![
        Box::new(InsertUnwrapFromFunctionCallOptionGenerator {}),
    ];
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Variable {
    pub antecedent: locals::VariableAntecedent,
    pub locals_id: lang::ID,
    pub typ: lang::Type,
}

impl Variable {
    pub(crate) fn name(&self) -> &str {
        &self.antecedent.name
    }
}

pub fn find_all_variables_preceding<'a>(search_position: locals::SearchPosition,
                                        code_genie: &'a CodeGenie,
                                        env_genie: &'a EnvGenie)
                                        -> impl Iterator<Item = Variable> + 'a {
    locals::find_all_referencable_variables(search_position, code_genie, env_genie).map(move |antecedent| {
        let locals_id = antecedent.assignment_id();
        let place = antecedent.place;
        Variable {
            antecedent,
            locals_id,
            typ: code_genie.guess_type_for_variable(place, env_genie).unwrap(),
        }
    })
}

const FUNCTION_CALL_GROUP: &str = "Functions";
const LOCALS_GROUP: &str = "Local variables";
const LITERALS_GROUP: &str = "Create new value";
const CONTROL_FLOW_GROUP: &str = "Control flow";
const ARGUMENTS_UNWRAP_GROUP: &str = "Arguments";

#[derive(Clone)]
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
                                env_genie: &EnvGenie,
                                location: CodeLocation)
                                -> Option<lang::CodeNode> {
        let options_groups = self.grouped_options(code_genie, env_genie, location);
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
                               env_genie: &'a EnvGenie,
                               location: CodeLocation)
                               -> Vec<InsertCodeMenuOptionsGroup> {
        let all_options = self.list_options(code_genie, env_genie, location);

        let mut options_groups = vec![];
        let selected_index = self.selected_index(all_options.len());
        for (group_name, options) in all_options.into_iter()
                                                // TODO: more efficient than sorting?
                                                .sorted_by_key(|o| o.group_name)
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
                        env_genie: &EnvGenie,
                        location: CodeLocation)
                        -> Vec<InsertCodeMenuOption> {
        let search_params = self.search_params(code_genie, env_genie, location);
        match self.insertion_point {
            InsertionPoint::Unwrap(_) => {
                UNWRAP_OPTIONS_GENERATORS.iter()
                                         .flat_map(|generator| {
                                             generator.options(&search_params,
                                                               code_genie,
                                                               env_genie)
                                         })
                                         .collect()
            }
            _ => OPTIONS_GENERATORS.iter()
                                   .flat_map(|generator| {
                                       generator.options(&search_params, code_genie, env_genie)
                                   })
                                   .collect(),
        }
    }

    pub fn search_params(&self,
                         code_genie: &CodeGenie,
                         env_genie: &EnvGenie,
                         location: CodeLocation)
                         -> CodeSearchParams {
        match self.insertion_point {
            // TODO: if it's the last line of a function, we might wanna use the function's type...
            // but that could be too limiting
            InsertionPoint::Before(_)
            | InsertionPoint::After(_)
            | InsertionPoint::BeginningOfBlock(_) => self.new_params(None, location),
            InsertionPoint::StructLiteralField(field_id) => {
                let node = code_genie.find_node(field_id).unwrap();
                self.new_params(figure_out_return_typ_for_insertion(node, location, env_genie,
                                                                    code_genie),
                                location)
            }
            InsertionPoint::Replace(node_id_to_replace) => {
                let node = code_genie.find_node(node_id_to_replace).unwrap();
                self.new_params(figure_out_return_typ_for_insertion(node, location, env_genie,
                                                                    code_genie),
                                location)
            }
            InsertionPoint::Wrap(node_id_to_wrap) => {
                let node = code_genie.find_node(node_id_to_wrap).unwrap();
                let wrapped_node_type = code_genie.guess_type(node, env_genie).unwrap();
                let return_typ =
                    figure_out_return_typ_for_insertion(node, location, env_genie, code_genie);
                self.new_params(return_typ, location)
                    .wraps_type(wrapped_node_type)
            }
            InsertionPoint::Unwrap(node_id_to_unwrap) => {
                let node_to_unwrap = code_genie.find_node(node_id_to_unwrap).unwrap();
                let return_typ = figure_out_return_typ_for_insertion(node_to_unwrap,
                                                                     location,
                                                                     env_genie,
                                                                     code_genie);
                self.new_params(return_typ, location)
                    .unwraps_code(node_id_to_unwrap)
            }
            InsertionPoint::ListLiteralElement { list_literal_id, .. } => {
                let list_literal = code_genie.find_node(list_literal_id).unwrap();
                let guessed_typ = code_genie.guess_type(list_literal, env_genie).unwrap();
                let element_type = get_type_from_list(guessed_typ).unwrap();
                self.new_params(Some(element_type), location)
            }
            InsertionPoint::Editing(_) => panic!("shouldn't have gotten here"),
        }
    }

    // we don't have to clone that string
    fn new_params(&self,
                  return_type: Option<lang::Type>,
                  location: CodeLocation)
                  -> CodeSearchParams {
        CodeSearchParams { input_str: self.input_str.clone(),
                           insertion_point: self.insertion_point,
                           location,
                           return_type,
                           unwraps_code_id: None,
                           wraps_type: None }
    }
}

fn figure_out_return_typ_for_insertion(node: &lang::CodeNode,
                                       location: CodeLocation,
                                       env_genie: &EnvGenie,
                                       code_genie: &CodeGenie)
                                       -> Option<lang::Type> {
    let exact_type = code_genie.guess_type(node, env_genie).unwrap();
    if env_genie.is_generic(exact_type.typespec_id) {
        return None;
    }

    // TODO: will probably need to see if this is the last expression of the block, and in that case
    // use the required return type
    //
    // the previous comment, and oldie, but a goodie:
    // block expressions (TODO??: except unless they're the last method) can safely be
    // replaced by any type because it's impossible for them to be referenced by anything
    if code_genie.is_block_expression(node.id()) {
        return None;
    }

    match code_genie.find_parent(node.id()) {
        Some(lang::CodeNode::Assignment(assignment))
            if !code_genie.any_variable_referencing_assignment(assignment.id) =>
        {
            None
        }
        Some(lang::CodeNode::EarlyReturn(_)) => required_return_type(location, env_genie),
        _ => Some(exact_type),
    }
}

#[derive(Clone, Debug)]
// TODO: pretty sure these could all be references....
pub struct CodeSearchParams {
    pub location: CodeLocation,
    pub return_type: Option<lang::Type>,
    pub wraps_type: Option<lang::Type>,
    pub unwraps_code_id: Option<lang::ID>,
    input_str: String,
    insertion_point: InsertionPoint,
}

impl CodeSearchParams {
    pub fn search_matches_type(&self, typ: &lang::Type, env_genie: &EnvGenie) -> bool {
        if let Some(return_type) = &self.return_type {
            env_genie.types_match(return_type, typ)
        } else {
            // if there's no return type being searched for, then we match everything
            true
        }
    }

    pub fn is_wrapping_typ(&self, typ: &lang::Type, env_genie: &EnvGenie) -> bool {
        if let Some(wraps_typ) = &self.wraps_type {
            env_genie.types_match(wraps_typ, typ)
        } else {
            false
        }
    }

    pub fn unwraps_code(mut self, id: lang::ID) -> Self {
        self.unwraps_code_id = Some(id);
        self
    }

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
        let found_functions = found_functions.filter_map(move |f| {
                                                 let takes_args = f.takes_args();
                                                 let first_matching_arg =
                                   takes_args.iter()
                                             .find(|arg| {
                                                 env_genie.types_match(&arg.arg_type, wraps_type)
                                             })?;
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
                     env_genie.types_match(&func.returns(), return_type.unwrap())
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

// this is used to see which assignments appear before a particular InsertionPoint.
//
// returns tuple -> (CodeNode position, is_inclusive)
pub fn assignment_search_position(insertion_point: InsertionPoint) -> (lang::ID, bool) {
    match insertion_point {
        InsertionPoint::BeginningOfBlock(id) => (id, false),
        InsertionPoint::Before(id) => (id, false),
        // after needs to be inclusive, because lang::ID itself could be an assignment expression
        InsertionPoint::After(id) => (id, true),
        InsertionPoint::StructLiteralField(id) => (id, false),
        InsertionPoint::Editing(id) => (id, false),
        InsertionPoint::Replace(id) => (id, false),
        InsertionPoint::ListLiteralElement { list_literal_id, .. } => (list_literal_id, false),
        InsertionPoint::Wrap(id) | InsertionPoint::Unwrap(id) => (id, false),
    }
}

#[derive(Clone)]
struct InsertVariableReferenceOptionGenerator {}

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

        let variables = find_all_variables_preceding(search_params.insertion_point.into(),
                                                     code_genie,
                                                     env_genie).filter(|variable| {
                            search_params.search_matches_type(&variable.typ, env_genie)
                            && search_params.search_matches_identifier(variable.name())
                        });

        variables.map(|variable| {
                     let id = variable.locals_id;
                     InsertCodeMenuOption { new_node: code_generation::new_variable_reference(id),
                                            sort_key: id.to_string(),
                                            is_selected: false,
                                            group_name: LOCALS_GROUP }
                 })
                 .collect()
    }
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

        let input_str = &search_params.input_str;
        if let Some(ref return_type) = search_params.return_type {
            self.generate_options_for_return_type(search_params, env_genie, input_str, &return_type)
        } else {
            self.generate_options_when_no_return_type_specified(search_params, env_genie, input_str)
        }
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
            sort_key: format!("listliteral{}", element_type.hash()),
            new_node: lang::CodeNode::ListLiteral(lang::ListLiteral {
                id: lang::new_id(),
                element_type: element_type.clone(),
                elements: vec![]
            })
        }
    }

    fn enum_options<'a>(&'a self,
                        enum_name: &'a str,
                        eneom: &'a enums::Enum,
                        enum_typ: &'a lang::Type)
                        -> impl Iterator<Item = InsertCodeMenuOption> + 'a {
        eneom.variant_types(&enum_typ.params)
             .into_iter()
             .map(move |(variant, variant_type)| InsertCodeMenuOption { sort_key: format!("enumliteral{}",
                                                                            variant.id),
                                                          new_node: lang::CodeNode::EnumVariantLiteral(code_generation::new_enum_variant_literal(
                                                              enum_name.to_string(),
                                                              enum_typ.clone(),
                                                              variant.id,
                                                              variant_type.clone()
                                                          )),
                                                          is_selected: false,
                                                          group_name: LITERALS_GROUP })
    }

    fn generate_options_when_no_return_type_specified(&self,
                                                      search_params: &CodeSearchParams,
                                                      env_genie: &EnvGenie,
                                                      input_str: &String)
                                                      -> Vec<InsertCodeMenuOption> {
        let mut options = vec![];

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

        options
    }

    fn generate_options_for_return_type(&self,
                                        search_params: &CodeSearchParams,
                                        env_genie: &EnvGenie,
                                        input_str: &String,
                                        return_type: &lang::Type)
                                        -> Vec<InsertCodeMenuOption> {
        let mut options = vec![];

        if return_type.matches_spec(&lang::STRING_TYPESPEC) {
            options.push(self.string_literal_option(input_str.clone()));
        }
        if return_type.matches_spec(&lang::NULL_TYPESPEC) {
            options.push(self.null_literal_option());
        }
        if return_type.matches_spec(&lang::NUMBER_TYPESPEC) {
            if let Some(number) = search_params.parse_number_input() {
                options.push(self.number_literal_option(number));
            }
        } else if return_type.typespec_id == lang::ANY_TYPESPEC.id() {
            options.push(self.number_literal_option(0));
        }
        // TODO: kind of a nasty if check for params.len()... that's to make sure it's not Any
        // TODO: shouldn't there be a way to insert list and then select the type though?
        if return_type.matches_spec(&lang::LIST_TYPESPEC) && return_type.params.len() > 0 {
            options.push(self.list_literal_option(&return_type));
        }
        if let Some(strukt) = env_genie.find_struct(return_type.typespec_id) {
            options.push(self.strukt_option(strukt));
        } else if let Some(eneom) = env_genie.find_enum(return_type.typespec_id) {
            options.extend(self.enum_options(&eneom.name, eneom, return_type))
        }
        // design decision made here: all placeholders have types. therefore, it is now
        // required for a placeholder node to have a type, meaning we need to know what the
        // type of a placeholder is to create it. under current conditions that's ok, but i
        // think we can make this less restrictive in the future if we need to
        options.push(self.placeholder_option(input_str.clone(), return_type));

        options
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
        if !is_inserting_inside_block(search_params.insertion_point, code_genie) {
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
struct InsertEarlyReturnOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertEarlyReturnOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        if !is_inserting_inside_block(search_params.insertion_point, code_genie) {
            return vec![];
        }

        if let Some(wraps_typ) = &search_params.wraps_type {
            // let you early return if you're wrapping the right return type
            if let Some(required_return_typ_for_context) =
                required_return_type(search_params.location, env_genie)
            {
                if !env_genie.types_match(&required_return_typ_for_context, wraps_typ) {
                    return vec![];
                }
            }
        }

        vec![InsertCodeMenuOption { sort_key: "earlyreturn".to_string(),
                                    new_node: code_generation::new_early_return(),
                                    is_selected: false,
                                    group_name: CONTROL_FLOW_GROUP }]
    }
}

#[derive(Clone)]
struct InsertWhileOptionGenerator {}

impl InsertWhileOptionGenerator {
    fn generate_option() -> InsertCodeMenuOption {
        InsertCodeMenuOption { sort_key: "while".to_string(),
                               group_name: CONTROL_FLOW_GROUP,
                               is_selected: false,
                               new_node: code_generation::new_while_loop() }
    }
}

impl InsertCodeMenuOptionGenerator for InsertWhileOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               _env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        if !is_inserting_inside_block(search_params.insertion_point, code_genie) {
            return vec![];
        }

        if let Some(wraps_type) = search_params.wraps_type.as_ref() {
            // if wrapping a boolean, then we should suggest creating a conditional.
            return if wraps_type.matches_spec(&*lang::BOOLEAN_TYPESPEC) {
                vec![Self::generate_option()]
            // otherwise we shouldn't pop up in the suggestions
            } else {
                vec![]
            };
        }

        let mut options = vec![];
        let search_str = search_params.lowercased_trimmed_search_str();
        if "while".contains(&search_str) {
            options.push(Self::generate_option())
        }
        options
    }
}

#[derive(Clone)]
struct InsertForLoopOptionGenerator {}

impl InsertForLoopOptionGenerator {
    fn generate_option() -> InsertCodeMenuOption {
        InsertCodeMenuOption { sort_key: "for".to_string(),
                               group_name: CONTROL_FLOW_GROUP,
                               is_selected: false,
                               new_node: lang::CodeNode::ForLoop(code_generation::new_for_loop()) }
    }
}

impl InsertCodeMenuOptionGenerator for InsertForLoopOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               _env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        if !is_inserting_inside_block(search_params.insertion_point, code_genie) {
            return vec![];
        }

        if let Some(wraps_type) = search_params.wraps_type.as_ref() {
            // if wrapping a boolean, then we should suggest creating a for loop.
            //
            // TODO: this needs to actually put the list expression inside...
            if wraps_type.matches_spec(&*lang::LIST_TYPESPEC) {
                return vec![Self::generate_option()];
            // otherwise we shouldn't pop up in the suggestions
            } else {
                return vec![];
            };
        }

        let mut options = vec![];
        if search_params.search_matches_identifier("for") {
            options.push(Self::generate_option())
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
        if !is_inserting_inside_block(search_params.insertion_point, code_genie) {
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

        find_all_variables_preceding(search_params.insertion_point
                                                                               .into(),
                                                                  code_genie,
                                                                  env_genie).filter_map(|variable| {
            self.new_option_if_enum(env_genie, &variable.typ, || {
                    code_generation::new_variable_reference(variable.locals_id)
                })
        })
        .collect()
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
        if !is_inserting_inside_block(search_params.insertion_point, code_genie) {
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
            group_name: LOCALS_GROUP,
            sort_key: format!("{}newvariable{}", sort_key_prefix, variable_name),
            is_selected: false,
            new_node: code_generation::new_assignment_code_node(
                variable_name.clone(),
                code_generation::new_placeholder(
                    variable_name,
                    lang::Type::from_spec(&*lang::NULL_TYPESPEC)))
        }]
    }
}

#[derive(Clone)]
struct InsertReassignmentOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertReassignmentOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        // reassignments only go inside of block expressions
        if !is_inserting_inside_block(search_params.insertion_point, code_genie) {
            return vec![];
        }

        let lowercased_trimmed_search_str = search_params.lowercased_trimmed_search_str();

        let sort_key_prefix = if lowercased_trimmed_search_str.contains('=') {
            // if the user typed a =, then it's very likely they wanted an assignment statement. sort
            // this up to the top, in that case
            "000"
        } else {
            "zzz"
        };

        let lowercased_trimmed_search_str =
            lowercased_trimmed_search_str.trim_end_matches(|c| c == '=' || c == ' ');

        find_all_variables_preceding(search_params.insertion_point.into(), code_genie, env_genie)
            .filter(|var| {
                if lowercased_trimmed_search_str.is_empty() {
                    return true
                }
                var.name().contains(lowercased_trimmed_search_str)
            })
            .map(|var| {
                InsertCodeMenuOption {
                    sort_key: format!("{}changevariable{}", sort_key_prefix, var.name()),
                    new_node: code_generation::new_reassignment(var.locals_id,
                                                                code_generation::new_placeholder(var.name().into(), var.typ)).into(),
                    is_selected: false,
                    group_name: LOCALS_GROUP,
                }
            }).collect()
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
        let wraps_strukt =
            search_params.wraps_type
                         .as_ref()
                         .and_then(|wraps_typ| env_genie.find_struct(wraps_typ.typespec_id));

        if let Some(wraps_strukt) = wraps_strukt {
            return wraps_strukt.fields
                               .iter()
                               .filter_map(move |struct_field| {
                                   let dotted_name =
                                       format!("{}.{}", wraps_strukt.name, struct_field.name);
                                   if !(search_params.search_matches_identifier(&wraps_strukt.name) ||
                        search_params.search_matches_identifier(&struct_field.name) ||
                        search_params.search_matches_identifier(&dotted_name)) {
                        return None
                    }

                                   if !search_params.search_matches_type(&struct_field.field_type,
                                                                         env_genie)
                                   {
                                       return None;
                                   }

                                   let wrapped_node = find_wrapped_node(code_genie, search_params);

                                   Some(InsertCodeMenuOption {
                        group_name: LOCALS_GROUP,
                        sort_key: format!("00wrappingstructfieldget{}", struct_field.id),
                        new_node: code_generation::new_struct_field_get(
                            wrapped_node.clone(),
                            struct_field.id,
                        ),
                        is_selected: false,
                    })
                               })
                               .collect();
        }

        let strukt_variables_in_scope =
            find_all_variables_preceding(search_params.insertion_point.into(),
                                         code_genie,
                                         env_genie).filter_map(|variable| {
                                                       if search_params.wraps_type.is_some()
                    && !search_params.is_wrapping_typ(&variable.typ, env_genie)
                {
                    return None;
                }

                                                       env_genie.find_struct(variable.typ
                                                                                     .typespec_id)
                                                                .map(|strukt| (variable, strukt))
                                                   });

        strukt_variables_in_scope.flat_map(|(variable, strukt)| {
                                     strukt.fields.iter().filter_map(move |struct_field| {
                let dotted_name = format!("{}.{}", variable.name(), struct_field.name);
                if !(search_params.search_matches_identifier(&variable.name()) ||
                    search_params.search_matches_identifier(&struct_field.name) ||
                    search_params.search_matches_identifier(&dotted_name)) {
                    return None
                }

                if !search_params.search_matches_type(&struct_field.field_type, env_genie) {
                    return None
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
            })
                                 })
                                 .collect()
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
        find_all_variables_preceding(search_params.insertion_point.into(),  code_genie, env_genie)
            .filter_map(|variable| {
                let list_elem_result_typ = get_result_type_from_indexing_into_list(variable.typ.clone())?;
                // if let Some(search_type) = &search_params.return_type {
                //     if !search_type.matches(&list_elem_result_typ) {
                //         return None
                //     }
                // }
                if search_params.search_matches_type(&list_elem_result_typ, env_genie) && search_params.search_matches_identifier(&variable.name()) {
                    Some(InsertCodeMenuOption {
                        group_name: LOCALS_GROUP,
                        sort_key: format!("listindex{}", variable.locals_id),
                        new_node: code_generation::new_list_index(code_generation::new_variable_reference(
                            variable.locals_id)),
                        is_selected: false
                    })
                } else {
                    // also

                    let name = variable.name().to_string();

                    let list_elem_typ = get_type_from_list(variable.typ)?;
                    if let Some(search_type) = &search_params.return_type {
                        // TODO: this should probably call the search_params.matches_type func or whatever it's called
                        // there's another place in this file that does the same thing, should replace that (lazy)
                        if !env_genie.types_match(&search_type, &list_elem_typ) {
                            return None
                        }
                    }
                    if !search_params.search_matches_identifier(&name) {
                        return None
                    }
                    Some(InsertCodeMenuOption {
                        group_name: LOCALS_GROUP,
                        sort_key: format!("listindextry{}", variable.locals_id),
                        new_node: lang::CodeNode::Try(code_generation::new_try(code_generation::new_list_index(code_generation::new_variable_reference(
                            variable.locals_id)), list_elem_typ)),
                        is_selected: false
                    })
                }
            })
            .collect()
    }
}

#[derive(Clone)]
struct InsertReassignListIndexOfLocalVariable {}

impl InsertCodeMenuOptionGenerator for InsertReassignListIndexOfLocalVariable {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        if !is_inserting_inside_block(search_params.insertion_point, code_genie) {
            return vec![];
        }

        // TODO: not sure if it's an issue we'll be able to reassign anything
        find_all_variables_preceding(
            search_params.insertion_point.into(), code_genie, env_genie)
            .filter_map(|variable| {
                // TODO: for now let's block anything not a plain ol' assignment
                if let locals::VariableAntecedentPlace::Assignment {.. } = variable.antecedent.place {
                } else {
                    return None;
                }

                if !search_params.search_matches_identifier(&variable.name()) {
                    return None
                }

                let list_typ = get_type_from_list(variable.typ)?;

                Some(InsertCodeMenuOption {
                        group_name: LOCALS_GROUP,
                        sort_key: format!("reassignlistindex{}", variable.locals_id),
                        new_node: lang::CodeNode::ReassignListIndex(code_generation::new_reassign_list_index(variable.locals_id, list_typ)),
                        is_selected: false
                    })
            })
            .collect()
    }
}

// hmmm this is used by code search
// TODO: move into insert_code_menu.rs
pub fn is_inserting_inside_block(insertion_point: InsertionPoint, code_genie: &CodeGenie) -> bool {
    match insertion_point {
        InsertionPoint::BeginningOfBlock(_)
        | InsertionPoint::Before(_)
        | InsertionPoint::After(_) => true,
        InsertionPoint::Replace(node_id)
        | InsertionPoint::Wrap(node_id)
        | InsertionPoint::Unwrap(node_id) => code_genie.is_block_expression(node_id),
        InsertionPoint::StructLiteralField(_)
        | InsertionPoint::Editing(_)
        | InsertionPoint::ListLiteralElement { .. } => false,
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

#[derive(Clone)]
struct InsertAnonFuncOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertAnonFuncOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               _code_genie: &CodeGenie,
               _env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        if search_params.return_type.is_none() {
            return vec![];
        }
        let return_type = search_params.return_type.as_ref().unwrap();
        if return_type.typespec_id != lang::ANON_FUNC_TYPESPEC.id {
            return vec![];
        }

        let (anon_func_takes_typ, anon_func_return_typ) =
            arg_and_return_typs_for_anon_func(return_type.clone());

        // TODO: takes_arg is hardcoded to string, how can this be a configurable type?
        // how to set the short_name?
        let takes_arg = ArgumentDefinition::new(anon_func_takes_typ, "var".into());

        // TODO: this could also return FunctionReferences (doesn't exist yet) in addition to
        // AnonymousFunction
        vec![InsertCodeMenuOption { sort_key: "block".to_string(),
                                    new_node: new_anon_func(takes_arg, anon_func_return_typ),
                                    is_selected: false,
                                    group_name: "Executable Code" }]
    }
}

#[derive(Clone)]
struct InsertTryOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertTryOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               _env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        if let Some(wraps_type) = &search_params.wraps_type {
            if [*builtins::RESULT_ENUM_ID, *builtins::OPTION_ENUM_ID].contains(&wraps_type.typespec_id) {
                let wrapped_node = find_wrapped_node(code_genie, search_params);
                return vec![InsertCodeMenuOption { sort_key: "tryoption".to_string(),
                    new_node: lang::CodeNode::Try(code_generation::new_try(wrapped_node.clone(),
                                                                           get_success_type_from_option_or_result_typ(wraps_type).unwrap().clone())),
                    is_selected: false,
                    group_name: CONTROL_FLOW_GROUP }];
            }
            // let type_to_search_for =
            //     if let Ok(result_ok_typ) = get_ok_type_from_result_type(wraps_type) {
            //         result_ok_typ
            //     } else if let Ok(option_some_typ) = get_some_type_from_option_type(wraps_type) {
            //         option_some_typ
            //     } else {
            //         // not wrapping a Result or Option, bail out
            //         return vec![];
            //     };
        }
        vec![]
    }
}

#[derive(Clone)]
struct InsertUnwrapFromFunctionCallOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertUnwrapFromFunctionCallOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        let code = code_genie.find_node(search_params.unwraps_code_id.unwrap())
                             .unwrap();
        let function_call = match code {
            lang::CodeNode::FunctionCall(func_call) => func_call,
            _ => return vec![],
        };
        function_call.args
                     .iter()
                     .filter_map(|code_node| {
                         let argument = code_node.into_argument();
                         let guessed_typ_from_arg =
                             code_genie.guess_type(code_node, env_genie).unwrap();
                         if search_params.search_matches_type(&guessed_typ_from_arg, env_genie) {
                             return Some(InsertCodeMenuOption { sort_key:
                                                                    format!("unwrap{}",
                                                                            argument.expr.id()),
                                                                new_node:
                                                                    (*argument.expr).clone(),
                                                                is_selected: false,
                                                                group_name:
                                                                    ARGUMENTS_UNWRAP_GROUP });
                         }
                         None
                     })
                     .collect()
    }
}

#[derive(Clone)]
struct InsertFunctionCallReplacementOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertFunctionCallReplacementOptionGenerator {
    fn options(&self,
               search_params: &CodeSearchParams,
               code_genie: &CodeGenie,
               env_genie: &EnvGenie)
               -> Vec<InsertCodeMenuOption> {
        let code_node_being_replaced = match search_params.insertion_point {
            InsertionPoint::Replace(id) => code_genie.find_node(id).unwrap(),
            _ => return vec![],
        };
        let function_call = code_node_being_replaced.as_function_call();
        if function_call.is_err() {
            return vec![];
        }
        let function_call_being_replaced = function_call.unwrap();
        let guessed_types_from_func_call_being_replaced =
            function_call_being_replaced.iter_args()
                                        .map(|arg| {
                                            code_genie.guess_type(&arg.expr, env_genie).unwrap()
                                        })
                                        .collect::<Vec<_>>();

        // if we find any functions that have exactly the same arg types as the function call we're replacing,
        // defined in the same order, then we'll yield them from here
        find_functions_ignoring_wraps_type(env_genie, search_params).filter_map(|found_func| {
            let found_func_takes_args = found_func.takes_args();
            if found_func_takes_args.len() != function_call_being_replaced.args.len() {
                return None;
            }
            let arg_types_of_found_func = found_func_takes_args.iter().map(|arg_def| {
                &arg_def.arg_type
            });
            if guessed_types_from_func_call_being_replaced.iter().zip(arg_types_of_found_func).all(|(replaced_func_typ, func_arg_typ)| {
                env_genie.types_match(replaced_func_typ, func_arg_typ)
            }) {
               return Some(InsertCodeMenuOption {
                   sort_key: format!("000replacefunccall{}", found_func.id()),
                   new_node: code_generation::new_function_call_with_arg_exprs(found_func, function_call_being_replaced.iter_args().map(|arg| {
                       (*arg.expr).clone()
                   })),
                   is_selected: false,
                   group_name: FUNCTION_CALL_GROUP,
               })
            }
            None
        }).collect()
    }
}
