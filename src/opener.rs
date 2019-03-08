use crate::editor::CommandBuffer;
use crate::editor::Controller;
use std::iter;
use crate::EnvGenie;
use lazy_static::lazy_static;
use matches::matches;
use itertools::Itertools;
use super::lang::Function;
use super::lang::TypeSpec;

lazy_static! {
    static ref CATEGORIES : Vec<Box<MenuCategory + Send + Sync>> = vec![
        Box::new(ChatTriggers {}),
        Box::new(JSONHTTPClients {}),
        Box::new(Functions {}),
        Box::new(Enums {}),
        Box::new(Structs {}),
    ];
}

pub struct Opener {
    pub input_str: String,
    pub selected_index: isize,
}

impl Opener {
    pub fn new() -> Self {
        Self {
            input_str: "".to_string(),
            selected_index: 0,
        }
    }

    // XXX: copy and paste from insert_code_menu.rs
    fn selected_index(&self, num_total_options: usize) -> usize {
        if num_total_options == 0 {
            return 0
        }
        let selected = self.selected_index % num_total_options as isize;
        if selected == 0 {
            0
        } else if selected > 0 {
            selected as usize
        } else {
            (num_total_options as isize + selected) as usize
        }
    }

    pub fn set_input_str(&mut self, input_str: String) {
        self.input_str = input_str;
        self.selected_index = 0;
    }

    pub fn select_next(&mut self) {
        self.selected_index += 1;
    }

    // controller used to see builtins
    pub fn list_options<'a>(&'a self, controller: &'a Controller,
                            env_genie: &'a EnvGenie<'a>) -> OptionsLister<'a> {
        OptionsLister::new(controller, env_genie, self)
    }
}

pub struct OptionsLister<'a> {
    pub controller: &'a Controller,
    pub env_genie: &'a EnvGenie<'a>,
    opener: &'a Opener,
}

impl<'a> OptionsLister<'a> {
    pub fn new(controller: &'a Controller, env_genie: &'a EnvGenie<'a>,
               opener: &'a Opener) -> Self {
        Self {
            controller,
            env_genie,
            opener,
        }
    }

    pub fn selected_option(&'a self) -> Option<MenuItem> {
        self.list().find(|menu_item| {
            if let MenuItem::Selectable { is_selected, .. } = menu_item {
                *is_selected
            } else {
                false
            }
        })
    }

    pub fn list(&'a self) -> impl ExactSizeIterator<Item = MenuItem> + 'a {
        let mut options = self.vec();
        let mut selectables = options.iter_mut()
            .filter(|menu_item| matches!(menu_item, MenuItem::Selectable { .. }))
            .collect_vec();
        let selected_index = self.opener.selected_index(selectables.len());
        if selectables.len() > selected_index {
            if let MenuItem::Selectable { ref mut is_selected, .. } = selectables[selected_index] {
                *is_selected = true
            }
        }
        options.into_iter()
    }

    // TODO: hax to make it work
    fn vec(&self) -> Vec<MenuItem> {
        Iterator::flatten(
            CATEGORIES.iter().filter_map(move |category| {
            let items = filter_matches(&self.opener.input_str, category.items(self))
                .collect_vec();
            if items.is_empty() {
                return None
            }
            Some(iter::once(MenuItem::Heading(category.label()))
                .chain(items.into_iter()))
            })
        ).collect()
    }

}

fn filter_matches<'a>(input_str: &'a str, items: impl Iterator<Item = MenuItem>) -> impl Iterator<Item = MenuItem> {
    // TODO: add fuzzy finding
    let input = input_str.trim().to_lowercase();
    items.filter(move |item| {
        match item {
            MenuItem::Selectable { label, .. } => label.to_lowercase().contains(&input),
            MenuItem::Heading(_) => panic!("this method shouldn't ever see a Heading"),
        }
    })
}

pub enum MenuItem {
    Heading(&'static str),
    Selectable { label: String, when_selected: Box<Fn(&mut CommandBuffer)>, is_selected: bool }
}

impl MenuItem {
    fn selectable(label: String, when_selected: impl Fn(&mut CommandBuffer) + 'static) -> Self {
        MenuItem::Selectable {
            label,
            when_selected: Box::new(when_selected),
            is_selected: false,
        }
    }
}

trait MenuCategory {
    fn label(&self) -> &'static str;
    fn items<'a>(&'a self, options_lister: &'a OptionsLister<'a>) -> Box<Iterator<Item = MenuItem> + 'a>;
}

struct ChatTriggers;

impl MenuCategory for ChatTriggers {
    fn label(&self) -> &'static str {
        "Chat triggers"
    }

    fn items<'a>(&'a self, options_lister: &'a OptionsLister<'a>) -> Box<Iterator<Item = MenuItem> + 'a> {
        Box::new(options_lister.env_genie.list_chat_triggers()
            .filter_map(move |ct| {
                if options_lister.controller.is_builtin(ct.id) {
                    return None
                }
                // TODO: we could avoid this clone by having load_chat_trigger take the
                // ID instead of the whole trigger
                let ct2 = ct.clone();
                Some(MenuItem::selectable(
                    ct.name.clone(),
                    move |command_buffer| {
                        let ct2 = ct2.clone();
                        command_buffer.load_chat_trigger(ct2)
                    }
                ))
            }))
    }
}

struct Functions;

impl MenuCategory for Functions {
    fn label(&self) -> &'static str {
        "Functions"
    }

    fn items<'a>(&'a self, options_lister: &'a OptionsLister<'a>) -> Box<Iterator<Item = MenuItem> + 'a> {
        Box::new(options_lister.env_genie.list_code_funcs()
            .filter_map(move |cf| {
                if options_lister.controller.is_builtin(cf.id()) {
                    return None
                }
                // TODO: we could avoid this clone by having load_chat_trigger take the
                // ID instead of the whole trigger
                let cf2 = cf.clone();
                Some(MenuItem::selectable(
                    cf.name.clone(),
                    move |command_buffer| {
                        let cf2 = cf2.clone();
                        command_buffer.load_code_func(cf2)
                    }
                ))
            }))
    }
}

struct JSONHTTPClients;

impl MenuCategory for JSONHTTPClients {
    fn label(&self) -> &'static str {
        "JSON HTTP Clients"
    }

    fn items<'a>(&'a self, options_lister: &'a OptionsLister<'a>) -> Box<Iterator<Item = MenuItem> + 'a> {
        Box::new(options_lister.env_genie.list_json_http_clients()
            .filter_map(move |cf| {
                if options_lister.controller.is_builtin(cf.id()) {
                    return None
                }
                // TODO: we could avoid this clone by having load_chat_trigger take the
                // ID instead of the whole trigger
                let cf2 = cf.clone();
                Some(MenuItem::selectable(
                    cf.name.clone(),
                    move |command_buffer| {
                        let cf2 = cf2.clone();
                        command_buffer.load_json_http_client(cf2)
                    }
                ))
            }))
    }
}

struct Enums;

impl MenuCategory for Enums {
    fn label(&self) -> &'static str {
        "Enums"
    }

    fn items<'a>(&'a self, options_lister: &'a OptionsLister<'a>) -> Box<Iterator<Item = MenuItem> + 'a> {
        Box::new(options_lister.env_genie.list_enums()
            .filter_map(move |eneom| {
                if options_lister.controller.is_builtin(eneom.id()) {
                    return None
                }
                // TODO: we could avoid this clone by having load_chat_trigger take the
                // ID instead of the whole trigger
                let eneom2 = eneom.clone();
                Some(MenuItem::selectable(
                    eneom.name.clone(),
                    move |command_buffer| {
                        let eneom2 = eneom2.clone();
                        command_buffer.load_typespec(eneom2)
                    }
                ))
            }))
    }
}

struct Structs;

impl MenuCategory for Structs {
    fn label(&self) -> &'static str {
        "Structs"
    }

    fn items<'a>(&'a self, options_lister: &'a OptionsLister<'a>) -> Box<Iterator<Item = MenuItem> + 'a> {
        Box::new(options_lister.env_genie.list_structs()
            .filter_map(move |strukt| {
                if options_lister.controller.is_builtin(strukt.id()) {
                    return None
                }
                // TODO: we could avoid this clone by having load_chat_trigger take the
                // ID instead of the whole trigger
                let strukt2 = strukt.clone();
                Some(MenuItem::selectable(
                    strukt.name.clone(),
                    move |command_buffer| {
                        let strukt = strukt2.clone();
                        command_buffer.load_typespec(strukt)
                    }
                ))
            }))
    }
}
