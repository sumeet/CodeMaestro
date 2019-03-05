use crate::editor::CommandBuffer;
use crate::editor::Controller;
use std::iter;
use crate::EnvGenie;
use lazy_static::lazy_static;
use matches::matches;
use itertools::Itertools;

lazy_static! {
    static ref CATEGORIES : Vec<Box<MenuCategory + Send + Sync>> = vec![
        Box::new(ChatTriggers {}),
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
        (self.selected_index % num_total_options as isize) as usize
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
        CATEGORIES.iter().flat_map(move |category| {
            iter::once(MenuItem::Heading("Chat triggers"))
                .chain(category.items(self))
        }).collect()
    }

}

pub enum MenuItem<'a> {
    Heading(&'static str),
    Selectable { label: &'a str, when_selected: Box<Fn(&mut CommandBuffer)>, is_selected: bool }
}

impl<'a> MenuItem<'a> {
    fn selectable(label: &'a str, when_selected: impl Fn(&mut CommandBuffer) + 'static) -> Self {
        MenuItem::Selectable {
            label,
            when_selected: Box::new(when_selected),
            is_selected: false,
        }
    }
}

trait MenuCategory {
    fn category(&self) -> &'static str;
    fn items<'a>(&'a self, options_lister: &'a OptionsLister<'a>) -> Box<Iterator<Item = MenuItem<'a>> + 'a>;
}

struct ChatTriggers;

impl MenuCategory for ChatTriggers {
    fn category(&self) -> &'static str {
        "Chat triggers"
    }

    fn items<'a>(&'a self, options_lister: &'a OptionsLister<'a>) -> Box<Iterator<Item = MenuItem<'a>> + 'a> {
        Box::new(options_lister.env_genie.list_chat_triggers()
            .filter_map(move |ct| {
                if options_lister.controller.is_builtin(ct.id) {
                    return None
                }
                // TODO: we could avoid this clone by having load_chat_trigger take the
                // ID instead of the whole trigger
                let ct2 = ct.clone();
                Some(MenuItem::selectable(
                    &ct.name,
                    move |command_buffer| {
                        let ct2 = ct2.clone();
                        command_buffer.load_chat_trigger(ct2)
                    }
                ))
            }))
    }
}