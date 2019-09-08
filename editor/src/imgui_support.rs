use glium::glutin::ElementState::Pressed;
use glium::glutin::Event;
use glium::glutin::VirtualKeyCode as Key;
use glium::glutin::WindowEvent::*;
use imgui::{FontGlyphRange, ImFontConfig, ImGui, ImGuiCol, Ui};
use std::time::Instant;

use super::editor::{Key as AppKey, Keypress};
use crate::color_schemes::COLOR_SCHEME;
use imgui_winit_support;

pub fn run<F: FnMut(&Ui, Option<Keypress>) -> bool>(title: String,
                                                    clear_color: [f32; 4],
                                                    mut run_ui: F) {
    use glium::{Display, Surface};
    use imgui_glium_renderer::Renderer;

    let mut events_loop = glutin::EventsLoop::new();
    let context = glutin::ContextBuilder::new().with_vsync(true);
    let icon =
        glutin::Icon::from_rgba(include_bytes!("../winicon.bin").to_vec(), 128, 128).unwrap();
    let builder =
        glutin::WindowBuilder::new().with_title(title)
                                    .with_window_icon(Some(icon))
                                    .with_dimensions(glutin::dpi::LogicalSize::new(1024f64,
                                                                                   768f64));
    let display = Display::new(builder, context, &events_loop).unwrap();
    let window = display.gl_window();

    let mut imgui = ImGui::init();
    imgui.set_ini_filename(None);

    let hidpi_factor = window.get_hidpi_factor();

    let font_size = (15.0 * hidpi_factor) as f32;
    let icon_font_size = font_size / 1.75;
    let icon_y_offset = (-2.0 * hidpi_factor) as f32;

    unsafe {
        imgui_sys::igStyleColorsClassic(imgui_sys::igGetStyle());
    }
    let mut style = imgui.style_mut();
    //
    style.colors[ImGuiCol::WindowBg as usize] = COLOR_SCHEME.window_bg_color.into();
    style.colors[ImGuiCol::ButtonActive as usize] = COLOR_SCHEME.button_active_color.into();
    style.colors[ImGuiCol::ButtonHovered as usize] = COLOR_SCHEME.button_hover_color.into();
    style.colors[ImGuiCol::MenuBarBg as usize] = COLOR_SCHEME.menubar_color.into();
    style.colors[ImGuiCol::TitleBg as usize] = COLOR_SCHEME.titlebar_bg_color.into();
    style.colors[ImGuiCol::TitleBgCollapsed as usize] = COLOR_SCHEME.titlebar_bg_color.into();
    style.colors[ImGuiCol::TitleBgActive as usize] = COLOR_SCHEME.titlebar_active_bg_color.into();

    // debug code to print colors
    println!("titlebg: {:?}", style.colors[ImGuiCol::TitleBg as usize]);
    println!("titlebgactive: {:?}",
             style.colors[ImGuiCol::TitleBgActive as usize]);
    println!("titlebgcollapsed: {:?}",
             style.colors[ImGuiCol::TitleBgCollapsed as usize]);

    // merge mode off for the first entry, should be on for the rest of them
    // TODO: also i think you have to add the fonts in such a way that the more specific ranges are
    // listed first... idk, i'm testing it out
    imgui.fonts()
         .add_font_with_config(include_bytes!("../../fonts/calibri.ttf"),
                               ImFontConfig::new().oversample_h(1)
                                                  .pixel_snap_h(true)
                                                  .size_pixels(font_size)
                                                  .rasterizer_multiply(1.75),
                               &FontGlyphRange::default());

    let range = FontGlyphRange::from_slice(&[0xf100,
                                             0xf104, // the range for custom fonts, small because it's only the ones we use
                                             0]);
    imgui.fonts()
         .add_font_with_config(include_bytes!("../../fonts/fontcustom.ttf"),
                               ImFontConfig::new().glyph_offset((0.0, icon_y_offset))
                                                  .oversample_h(1)
                                                  .pixel_snap_h(true)
                                                  .size_pixels(icon_font_size)
                                                  .merge_mode(true)
                                                  .rasterizer_multiply(1.75),
                               &range);

    imgui.fonts()
         .add_font_with_config(include_bytes!("../../fonts/NanumGothic.ttf"),
                               ImFontConfig::new().oversample_h(1)
                                                  .pixel_snap_h(true)
                                                  .size_pixels(font_size)
                                                  .merge_mode(true)
                                                  .rasterizer_multiply(1.75),
                               &FontGlyphRange::korean());

    imgui.fonts()
         .add_font_with_config(include_bytes!("../../fonts/Osaka-UI-03.ttf"),
                               ImFontConfig::new().oversample_h(1)
                                                  .pixel_snap_h(true)
                                                  .size_pixels(font_size)
                                                  .merge_mode(true)
                                                  .rasterizer_multiply(1.75),
                               &FontGlyphRange::japanese());

    let range = FontGlyphRange::from_slice(&[0xf004,
                                             0xf5c8, // the range for font awesome regular 400
                                             0]);
    imgui.fonts()
         .add_font_with_config(include_bytes!("../../fonts/fa-regular-400.ttf"),
                               ImFontConfig::new().glyph_offset((0.0, icon_y_offset))
                                                  .oversample_h(1)
                                                  .pixel_snap_h(true)
                                                  .size_pixels(icon_font_size)
                                                  .merge_mode(true)
                                                  .rasterizer_multiply(1.75),
                               &range);

    let range = FontGlyphRange::from_slice(&[0xf000,
                                             0xf72f, // the range for font awesome solid 900
                                             0]);
    imgui.fonts()
         .add_font_with_config(include_bytes!("../../fonts/fa-solid-900.ttf"),
                               ImFontConfig::new().glyph_offset((0.0, icon_y_offset))
                                                  .oversample_h(1)
                                                  .pixel_snap_h(true)
                                                  .size_pixels(icon_font_size)
                                                  .merge_mode(true)
                                                  .rasterizer_multiply(1.75),
                               &range);

    let range = FontGlyphRange::from_slice(&[0xf298,
                                             0xf298, // the range for font awesome brands 400 (that we use)
                                             0]);
    imgui.fonts()
         .add_font_with_config(include_bytes!("../../fonts/fa-brands-400.ttf"),
                               ImFontConfig::new().glyph_offset((0.0, icon_y_offset))
                                                  .oversample_h(1)
                                                  .pixel_snap_h(true)
                                                  .size_pixels(icon_font_size)
                                                  .merge_mode(true)
                                                  .rasterizer_multiply(1.75),
                               &range);

    imgui.set_font_global_scale((1.0 / hidpi_factor) as f32);

    let mut renderer = Renderer::init(&mut imgui, &display).expect("Failed to initialize renderer");

    imgui_winit_support::configure_keys(&mut imgui);

    let mut last_frame = Instant::now();
    let mut quit = false;

    loop {
        let mut keypress: Option<Keypress> = None;

        events_loop.poll_events(|event| {
                       imgui_winit_support::handle_event(&mut imgui,
                                                         &event,
                                                         window.get_hidpi_factor(),
                                                         hidpi_factor);

                       if let Event::WindowEvent { event, .. } = event {
                           match event {
                               CloseRequested => quit = true,
                               KeyboardInput { input, .. } => {
                                   let pressed = input.state == Pressed;
                                   if pressed {
                                       match input.virtual_keycode {
                                           Some(key) => {
                                               if let Some(key) = map_key(key) {
                                                   keypress =
                                                       Some(Keypress::new(key,
                                                                          imgui.key_ctrl(),
                                                                          imgui.key_shift()));
                                               }
                                           }
                                           _ => {}
                                       }
                                   }
                               }
                               _ => (),
                           }
                       }
                   });

        let now = Instant::now();
        let delta = now - last_frame;
        let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
        last_frame = now;

        imgui_winit_support::update_mouse_cursor(&imgui, &window);

        let frame_size = imgui_winit_support::get_frame_size(&window, hidpi_factor).unwrap();

        let ui = imgui.frame(frame_size, delta_s);
        if !run_ui(&ui, keypress) {
            break;
        }

        let mut target = display.draw();
        target.clear_color(clear_color[0],
                           clear_color[1],
                           clear_color[2],
                           clear_color[3]);
        renderer.render(&mut target, ui).expect("Rendering failed");
        target.finish().unwrap();

        if quit {
            break;
        }
    }
}

fn map_key(key: Key) -> Option<AppKey> {
    match key {
        Key::A => Some(AppKey::A),
        Key::B => Some(AppKey::B),
        Key::C => Some(AppKey::C),
        Key::D => Some(AppKey::D),
        Key::H => Some(AppKey::H),
        Key::J => Some(AppKey::J),
        Key::K => Some(AppKey::K),
        Key::L => Some(AppKey::L),
        Key::W => Some(AppKey::W),
        Key::X => Some(AppKey::X),
        Key::R => Some(AppKey::R),
        Key::O => Some(AppKey::O),
        Key::U => Some(AppKey::U),
        Key::V => Some(AppKey::V),
        Key::Escape => Some(AppKey::Escape),
        Key::Tab => Some(AppKey::Tab),
        Key::Up => Some(AppKey::UpArrow),
        Key::Down => Some(AppKey::DownArrow),
        Key::Left => Some(AppKey::LeftArrow),
        Key::Right => Some(AppKey::RightArrow),
        _ => None,
    }
}
