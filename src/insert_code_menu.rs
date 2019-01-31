use super::code_editor::InsertionPoint;
use super::env_genie::EnvGenie;
use super::code_editor::CodeGenie;
use super::code_editor::PLACEHOLDER_ICON;
use super::lang;
use super::code_generation;
use super::structs;

use objekt::{clone_trait_object};
use lazy_static::lazy_static;
use itertools::Itertools;

use std::collections::HashMap;

lazy_static! {
    static ref OPTIONS_GENERATORS : Vec<Box<InsertCodeMenuOptionGenerator + Send + Sync>> = vec![
        Box::new(InsertVariableReferenceOptionGenerator {}),
        Box::new(InsertFunctionOptionGenerator {}),
        Box::new(InsertLiteralOptionGenerator {}),
        Box::new(InsertConditionalOptionGenerator {}),
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
    fn selected_index(&self, num_options: usize) -> usize {
        (self.selected_option_index % num_options as isize) as usize
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
            InsertionPoint::Argument(field_id) | InsertionPoint::StructLiteralField(field_id) => {
                let node = code_genie.find_node(field_id).unwrap();
                let exact_type = code_genie.guess_type(node, env_genie);
                self.new_params(Some(exact_type))
            },
            InsertionPoint::Editing(_) => panic!("shouldn't have gotten here"),
            InsertionPoint::ListLiteralElement { list_literal_id, .. } => {
                let list_literal = code_genie.find_node(list_literal_id).unwrap();
                match list_literal {
                    lang::CodeNode::ListLiteral(list_literal) => {
                        self.new_params(Some(list_literal.element_type.clone()))
                    }
                    _ => panic!("should always be a list literal... ugh"),
                }
            }
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

    pub fn list_of_something(&self) -> Option<String> {
        let input_str = self.lowercased_trimmed_search_str();
        if input_str.starts_with("list") {
            Some(input_str.trim_start_matches("list").trim().into())
        } else {
            None
        }
    }
}

// TODO: types of insert code generators
// 1: variable
// 2: function call to capitalize
// 3: new string literal
// 4: placeholder

trait InsertCodeMenuOptionGenerator : objekt::Clone {
    fn options(&self, search_params: &CodeSearchParams, code_genie: &CodeGenie,
               env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption>;
}

clone_trait_object!(InsertCodeMenuOptionGenerator);

#[derive(Clone)]
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
struct InsertVariableReferenceOptionGenerator {}

struct Variable {
    locals_id: lang::ID,
    type_id: lang::ID,
    name: String,
}

// also handles function arguments
impl InsertCodeMenuOptionGenerator for InsertVariableReferenceOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams, code_genie: &CodeGenie,
               env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        let insertion_id = search_params.insertion_point.node_id();

        let mut variables_by_type_id : HashMap<lang::ID, Vec<Variable>> = code_genie.find_assignments_that_come_before_code(insertion_id)
            .into_iter()
            .map(|assignment| {
                let assignment_clone : lang::Assignment = (*assignment).clone();
                let guessed_type_id = code_genie.guess_type(&lang::CodeNode::Assignment(assignment_clone), env_genie).id();
                Variable { locals_id: assignment.id, type_id: guessed_type_id, name: assignment.name.clone() }
            })
            .chain(
                env_genie.code_takes_args(code_genie.root().id())
                    .map(|arg| Variable { locals_id: arg.id, type_id: arg.arg_type.id(), name: arg.short_name })
            )
            .group_by(|variable| variable.type_id)
            .into_iter()
            .map(|(id, variables)| (id, variables.collect()))
            .collect();

        let mut variables : Vec<Variable> = if let Some(search_type) = &search_params.return_type {
            variables_by_type_id.remove(&search_type.id()).unwrap_or_else(|| vec![])
        } else {
            Iterator::flatten(variables_by_type_id.drain().map(|(_, v)| v)).collect()
        };

        let input_str = search_params.lowercased_trimmed_search_str();
        if !input_str.is_empty() {
            variables = variables.into_iter()
                .filter(|variable| variable.name.to_lowercase().contains(&input_str))
                .collect();
        }

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

#[derive(Clone)]
struct InsertLiteralOptionGenerator {}

impl InsertCodeMenuOptionGenerator for InsertLiteralOptionGenerator {
    fn options(&self, search_params: &CodeSearchParams, _code_genie: &CodeGenie,
               env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        let mut options = vec![];
        let input_str = &search_params.lowercased_trimmed_search_str();
        if let Some(ref return_type) = search_params.return_type {
            if return_type.matches_spec(&lang::STRING_TYPESPEC) {
                options.push(self.string_literal_option(input_str));
            } else if return_type.matches_spec(&lang::NULL_TYPESPEC) {
                options.push(self.null_literal_option());
            } else if return_type.matches_spec(&lang::LIST_TYPESPEC) {
                options.push(self.list_literal_option(env_genie, &return_type));
            } else if let Some(strukt) = env_genie.find_struct(return_type.typespec_id) {
                options.push(self.strukt_option(strukt));
            }

            // design decision made here: all placeholders have types. therefore, it is now
            // required for a placeholder node to have a type, meaning we need to know what the
            // type of a placeholder is to create it. under current conditions that's ok, but i
            // think we can make this less restrictive in the future if we need to
            options.push(self.placeholder_option(input_str, return_type));
        } else {
            if let Some(list_search_query) = search_params.list_of_something() {
                let matching_list_type_options = env_genie
                    .find_types_matching(&list_search_query)
                    .map(|t| {
                        let list_type = lang::Type::with_params(&*lang::LIST_TYPESPEC, vec![t]);
                        self.list_literal_option(env_genie, &list_type)
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
    fn options(&self, search_params: &CodeSearchParams, _code_genie: &CodeGenie,
               _env_genie: &EnvGenie) -> Vec<InsertCodeMenuOption> {
        let mut options = vec![];
        if !search_params.insertion_point.is_block_expression() {
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

