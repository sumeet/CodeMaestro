#![feature(generators)]
#![feature(box_patterns)]
#![feature(drain_filter)]
#![feature(associated_type_defaults)]
#![feature(try_blocks)]
#![feature(unsized_locals)]
#![allow(incomplete_features)]
#![feature(const_generics)]
#![feature(trait_alias)]
#![feature(core_intrinsics)]
#![recursion_limit = "1024"]

// crates
use cfg_if::cfg_if;

// modules
mod app;
mod chat;
mod chat_test_window;
mod code_editor;
mod code_editor_renderer;
mod code_rendering;
mod code_validation;
mod color_schemes;
mod edit_types;
mod editor;
#[cfg(not(target_arch = "wasm32"))]
mod imgui_support;
#[cfg(not(target_arch = "wasm32"))]
mod imgui_toolkit;
mod insert_code_menu;
mod insert_code_menu_renderer;
mod json2;
mod json_http_client_builder;
mod opener;
mod save_state;
mod schema_builder;
mod send_to_server_overlay;
mod theme_editor_renderer;
mod ui_toolkit;
mod undo;
mod window_positions;
#[cfg(target_arch = "wasm32")]
mod yew_toolkit;

#[cfg(not(target_arch = "wasm32"))]
mod tokio_executor;
#[cfg(not(target_arch = "wasm32"))]
mod async_executor {
    pub use super::tokio_executor::*;
}
#[cfg(target_arch = "wasm32")]
mod stdweb_executor;
#[cfg(target_arch = "wasm32")]
mod async_executor {
    pub use super::stdweb_executor::*;
}

// TODO: this is a holdover because this module used to be part of this package...
// so we'd have to update all imports
pub use cs::code_generation;

// uses for this module
use crate::color_schemes::set_colorscheme;
#[cfg(not(target_arch = "wasm32"))]
use imgui_toolkit::draw_app;
#[cfg(target_arch = "wasm32")]
use yew_toolkit::draw_app;

cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        fn init_debug() {
            use stdweb::{console};
            ::std::panic::set_hook(Box::new(|info| {
                console!(error, format!("!!! RUST PANIC !!! {:?}", info));
            }));
        }
    } else {
        fn init_debug() {}
    }
}

pub fn main() {
    init_debug();
    let theme_from_plane = serde_json::from_str(include_str!("../../themefromplane.json")).unwrap();
    set_colorscheme(theme_from_plane);
    async_executor::with_executor_context(|async_executor| {
        let app = app::App::new_rc();
        draw_app(app, async_executor);
    })
}
