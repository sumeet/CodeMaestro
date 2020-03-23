use glium::glutin::ElementState::Pressed;
use glium::glutin::Event;
use glium::glutin::VirtualKeyCode as Key;
use glium::glutin::WindowEvent::*;
use imgui::{Context, FontConfig, FontGlyphRanges, FontSource, StyleColor, Ui};
use std::time::Instant;

use super::editor::{Key as AppKey, Keypress};
use crate::colorscheme;
use imgui_winit_support;
use imgui_winit_support::{HiDpiMode, WinitPlatform};

pub fn run<F: FnMut(&Ui, Option<Keypress>) -> bool>(title: String, mut run_ui: F) {
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

    let mut imgui = Context::create();
    imgui.set_ini_filename(None);

    let mut platform = WinitPlatform::init(&mut imgui);
    platform.attach_window(imgui.io_mut(), &window.window(), HiDpiMode::Rounded);

    let hidpi_factor = platform.hidpi_factor();
    //let hidpi_factor = 1.0;

    let font_size = (15.0 * hidpi_factor) as f32;
    let icon_font_size = font_size / 1.75;
    let icon_y_offset = (-3.0 * hidpi_factor) as f32;

    unsafe {
        imgui_sys::igStyleColorsClassic(imgui_sys::igGetStyle());
    }
    let mut style = imgui.style_mut();
    // currently duped in the loop so we can do it on every frame
    // TODO: clean that up
    style.colors[StyleColor::WindowBg as usize] = colorscheme!(window_bg_color).into();
    style.colors[StyleColor::ButtonActive as usize] = colorscheme!(button_active_color).into();
    style.colors[StyleColor::ButtonHovered as usize] = colorscheme!(button_hover_color).into();
    style.colors[StyleColor::MenuBarBg as usize] = colorscheme!(menubar_color).into();
    style.colors[StyleColor::TitleBg as usize] = colorscheme!(titlebar_bg_color).into();
    style.colors[StyleColor::TitleBgCollapsed as usize] = colorscheme!(titlebar_bg_color).into();
    style.colors[StyleColor::TitleBgActive as usize] =
        colorscheme!(titlebar_active_bg_color).into();

    // debug code to print colors
    //    println!("titlebg: {:?}", style.colors[StyleColor::TitleBg as usize]);
    //    println!("titlebgactive: {:?}",
    //             style.colors[StyleColor::TitleBgActive as usize]);
    //    println!("titlebgcollapsed: {:?}",
    //             style.colors[StyleColor::TitleBgCollapsed as usize]);

    // merge mode off for the first entry, should be on for the rest of them
    // TODO: also i think you have to add the fonts in such a way that the more specific ranges are
    // listed first... idk, i'm testing it out
    let font_sources =
        [FontSource::TtfData { data: include_bytes!("../../fonts/calibri.ttf"),
                               size_pixels: font_size,
                               config: Some(FontConfig { rasterizer_multiply: 1.75,
                                                         glyph_ranges:
                                                             FontGlyphRanges::default(),
                                                         pixel_snap_h: true,
                                                         oversample_h: 1,
                                                         ..FontConfig::default() }) },
         FontSource::TtfData { data: include_bytes!("../../fonts/fontcustom.ttf"),
                               size_pixels: font_size,
                               config: Some(FontConfig { glyph_offset: [0.0, icon_y_offset],
                                                         rasterizer_multiply: 1.75,
                                                         glyph_ranges:
                                                             FontGlyphRanges::from_slice(&[0xf100,
                                                                                           0xf104, // the range for custom fonts, small because it's only the ones we use
                                                                                           0]),
                                                         pixel_snap_h: true,
                                                         oversample_h: 1,
                                                         ..FontConfig::default() }) },
         FontSource::TtfData { data: include_bytes!("../../fonts/NanumGothic.ttf"),
                               size_pixels: font_size,
                               config: Some(FontConfig { rasterizer_multiply: 1.75,
                                                         glyph_ranges:
                                                             FontGlyphRanges::korean(),
                                                         pixel_snap_h: true,
                                                         oversample_h: 1,
                                                         ..FontConfig::default() }) },
         FontSource::TtfData { data: include_bytes!("../../fonts/Osaka-UI-03.ttf"),
                               size_pixels: font_size,
                               config: Some(FontConfig { rasterizer_multiply: 1.75,
                                                         glyph_ranges:
                                                             FontGlyphRanges::japanese(),
                                                         pixel_snap_h: true,
                                                         oversample_h: 1,
                                                         ..FontConfig::default() }) },
         FontSource::TtfData { data: include_bytes!("../../fonts/fa-regular-400.ttf"),
                               size_pixels: icon_font_size,
                               config: Some(FontConfig { rasterizer_multiply: 1.75,
                                                         glyph_offset: [0.0, icon_y_offset],
                                                         glyph_ranges:
                                                             FontGlyphRanges::from_slice(&[0xf004,
                                                                                           0xf5c8, // the range for font awesome regular 400
                                                                                           0]),
                                                         pixel_snap_h: true,
                                                         oversample_h: 1,
                                                         ..FontConfig::default() }) },
         FontSource::TtfData { data: include_bytes!("../../fonts/fa-solid-900.ttf"),
                               size_pixels: icon_font_size,
                               config: Some(FontConfig { glyph_offset: [0.0, icon_y_offset],
                                                         rasterizer_multiply: 1.75,
                                                         glyph_ranges:
                                                             FontGlyphRanges::from_slice(&[0xf000,
                                                                                           0xf72f, // the range for font awesome solid 900
                                                                                           0]),
                                                         pixel_snap_h: true,
                                                         oversample_h: 1,
                                                         ..FontConfig::default() }) },
         FontSource::TtfData { data: include_bytes!("../../fonts/fa-brands-400.ttf"),
                               size_pixels: icon_font_size,
                               config: Some(FontConfig { glyph_offset: [0.0, icon_y_offset],
                                                         rasterizer_multiply: 1.75,
                                                         glyph_ranges:
                                                             FontGlyphRanges::from_slice(&[0xf298,
                                                                                           0xf298, // the range for font awesome brands 400 (that we use)
                                                                                           0]),
                                                         pixel_snap_h: true,
                                                         oversample_h: 1,
                                                         ..FontConfig::default() }) }];

    //font_sources.reverse();

    imgui.fonts().add_font(&font_sources);

    imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

    let mut renderer = Renderer::init(&mut imgui, &display).expect("Failed to initialize renderer");

    // TODO: not sure if i need this in some form or if i can totally get rid of this line
    //imgui_winit_support::configure_keys(&mut imgui);

    let mut last_frame = Instant::now();
    let mut quit = false;

    loop {
        // duped above... clean this up TODO
        let mut style = imgui.style_mut();
        style.colors[StyleColor::WindowBg as usize] = colorscheme!(window_bg_color).into();
        style.colors[StyleColor::ButtonActive as usize] = colorscheme!(button_active_color).into();
        style.colors[StyleColor::ButtonHovered as usize] = colorscheme!(button_hover_color).into();
        style.colors[StyleColor::MenuBarBg as usize] = colorscheme!(menubar_color).into();
        style.colors[StyleColor::TitleBg as usize] = colorscheme!(titlebar_bg_color).into();
        style.colors[StyleColor::TitleBgCollapsed as usize] =
            colorscheme!(titlebar_bg_color).into();
        style.colors[StyleColor::TitleBgActive as usize] =
            colorscheme!(titlebar_active_bg_color).into();
        style.colors[StyleColor::FrameBg as usize] = colorscheme!(input_bg_color).into();

        let mut keypress: Option<Keypress> = None;

        events_loop.poll_events(|event| {
                       platform.handle_event(imgui.io_mut(), &window.window(), &event);

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

        let io = imgui.io_mut();
        platform.prepare_frame(io, &window.window())
                .expect("Failed to start frame");
        last_frame = io.update_delta_time(last_frame);
        let ui = imgui.frame();
        if !run_ui(&ui, keypress) {
            break;
        }

        let mut target = display.draw();
        target.clear_color_srgb(1.0, 1.0, 1.0, 1.0);
        platform.prepare_render(&ui, &window.window());
        renderer.render(&mut target, ui.render())
                .expect("Rendering failed");
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
