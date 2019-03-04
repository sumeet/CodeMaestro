use crate::editor::CommandBuffer;
use crate::editor::Controller;
use crate::env::ExecutionEnvironment;
use std::cell::RefCell;
use std::rc::Rc;
use std::iter;
use crate::EnvGenie;
use lazy_static::lazy_static;

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
        self.list().nth(self.opener.selected_index as usize)
    }

    pub fn list(&'a self) -> impl Iterator<Item = MenuItem> + 'a {
        CATEGORIES.iter().flat_map(move |category| {
            iter::once(MenuItem::Heading("Chat triggers"))
                .chain(category.items(self))
        })
    }

}

pub enum MenuItem<'a> {
    Heading(&'static str),
    Selectable { label: &'a str, when_selected: Box<Fn(&mut CommandBuffer)> }
}

impl<'a> MenuItem<'a> {
    fn selectable(label: &'a str, when_selected: impl Fn(&mut CommandBuffer) + 'static) -> Self {
        MenuItem::Selectable {
            label,
            when_selected: Box::new(when_selected),
        }
    }
}

trait MenuCategory {
    fn category(&self) -> &'static str;
    fn items<'a>(&'a self, options_lister: &'a OptionsLister<'a>) -> Box<Iterator<Item = MenuItem<'a>> + 'a>;
}

struct ChatTriggers {}

impl MenuCategory for ChatTriggers {
    fn category(&self) -> &'static str {
        "Chat triggers"
    }

    fn items<'a>(&'a self, options_lister: &'a OptionsLister<'a>) -> Box<Iterator<Item = MenuItem<'a>> + 'a> {
        Box::new(options_lister.env_genie.list_chat_triggers()
            .map(|ct| {
                // TODO: we could avoid this clone by having load_chat_trigger take the
                // ID instead of the whole trigger
                let ct2 = ct.clone();
                MenuItem::selectable(
                    &ct.name,
                    move |command_buffer| {
                        let ct2 = ct2.clone();
                        command_buffer.load_chat_trigger(ct2)
                    }
                )
            }))
    }
}