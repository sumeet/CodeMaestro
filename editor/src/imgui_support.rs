use glium::glutin::event_loop::EventLoop;
use glutin::window::{Icon, WindowBuilder};
use imgui::{Context, FontConfig, FontGlyphRanges, FontSource, StyleColor, Ui};
use std::time::Instant;

use super::editor::{Key as AppKey, Keypress};
use crate::colorscheme;
use imgui_winit_support;
use imgui_winit_support::{HiDpiMode, WinitPlatform};

pub fn run<F: FnMut(&Ui, Option<Keypress>) -> bool>(title: String, mut run_ui: F) {
    use glium::{Display, Surface};
    use imgui_glium_renderer::Renderer;

    let mut event_loop = EventLoop::new();
    let context = glutin::ContextBuilder::new().with_vsync(true);
    let icon = Icon::from_rgba(include_bytes!("../winicon.bin").to_vec(), 128, 128).unwrap();
    let builder =
        WindowBuilder::new().with_title(title)
                            .with_window_icon(Some(icon))
                            .with_inner_size(glutin::dpi::LogicalSize::new(1024f64, 768f64));
    let display = Display::new(builder, context, &event_loop).unwrap();

    let mut imgui = Context::create();
    imgui.set_ini_filename(None);

    let backend = init_clipboard().unwrap();
    imgui.set_clipboard_backend(Box::new(backend));

    let mut platform = WinitPlatform::init(&mut imgui);
    let gl_window = display.gl_window();
    let window = gl_window.window();
    platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Rounded);

    let hidpi_factor = platform.hidpi_factor();
    //let hidpi_factor = 1.0;

    let font_size = (17.0 * hidpi_factor) as f32;
    let icon_font_size = font_size / 1.75;
    let fontawesome_icon_y_offset = (-2.0 * hidpi_factor) as f32;
    // let custom_icon_y_offset = 1.; //(-0.25 * hidpi_factor) as f32;
    let noto_sans_font_size = font_size * 1.75;
    let noto_sans_y_offset = 2.;

    unsafe {
        imgui_sys::igStyleColorsClassic(imgui_sys::igGetStyle());
    }

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
         // same as above but increased range
         FontSource::TtfData { data: include_bytes!("../../fonts/calibri.ttf"),
                               size_pixels: font_size,
                               config: Some(FontConfig { rasterizer_multiply: 1.75,
                                                         glyph_ranges:
                                                         // The "General Punctuation" range:
                                                         // https://en.wikipedia.org/wiki/General_Punctuation
                                                             FontGlyphRanges::from_slice(&[
                                                                 0x2000, 0x206F,
                                                             0]),
                                                         pixel_snap_h: true,
                                                         oversample_h: 1,
                                                         ..FontConfig::default() }) },
         FontSource::TtfData { data: include_bytes!("../../fonts/NotoSansSymbols-Black.ttf"),
                               size_pixels: noto_sans_font_size,
                               config: Some(FontConfig { glyph_offset: [0.0, noto_sans_y_offset],
                                                         rasterizer_multiply: 1.75,
                                                         glyph_ranges:
                                                         // The "Miscellaneous Technical" range:
                                                         // https://www.compart.com/en/unicode/block/U+2300
                                                             FontGlyphRanges::from_slice(&[0x2300,
                                                                                           0x23ff,
                                                                                           0]),
                                                         pixel_snap_h: true,
                                                         oversample_h: 1,
                                                         ..FontConfig::default() }) },
         FontSource::TtfData { data: include_bytes!("../../fonts/NotoSansMath-Regular.ttf"),
                               size_pixels: noto_sans_font_size,
                               config: Some(
                                            FontConfig { glyph_offset: [0.0, noto_sans_y_offset],
                    rasterizer_multiply: 1.75,
                    glyph_ranges:
                    // The "Supplemental Mathematical Operators block" range:
                    // https://en.wikipedia.org/wiki/Supplemental_Mathematical_Operators
                    // and
                    // The "Mathematical Operators block" range:
                    // https://en.wikipedia.org/wiki/Mathematical_Operators_(Unicode_block)
                    FontGlyphRanges::from_slice(&[0x2A00, 0x2AFF, // "supplemental"
                        0x2200, 0x22FF, // mathematical operators
                        0]),
                    pixel_snap_h: true,
                    oversample_h: 1,
                    ..FontConfig::default() },
        ) },
         // FontSource::TtfData { data: include_bytes!("../../fonts/fontcustom.ttf"),
         //                       size_pixels: font_size,
         //                       config: Some(FontConfig { glyph_offset:
         //                                                     [0.0, custom_icon_y_offset],
         //                                                 rasterizer_multiply: 1.75,
         //                                                 glyph_ranges:
         //                                                     FontGlyphRanges::from_slice(&[0xf100,
         //                                                                                   0xf104, // the range for custom fonts, small because it's only the ones we use
         //                                                                                   0]),
         //                                                 pixel_snap_h: true,
         //                                                 oversample_h: 1,
         //                                                 ..FontConfig::default() }) },
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
                                                         glyph_offset:
                                                             [0.0, fontawesome_icon_y_offset],
                                                         glyph_ranges:
                                                             FontGlyphRanges::from_slice(&[0xf004,
                                                                                           0xf5c8, // the range for font awesome regular 400
                                                                                           0]),
                                                         pixel_snap_h: true,
                                                         oversample_h: 1,
                                                         ..FontConfig::default() }) },
         FontSource::TtfData { data: include_bytes!("../../fonts/fa-solid-pro-900.ttf"),
                               size_pixels: icon_font_size,
                               config: Some(FontConfig { glyph_offset:
                                                             [0.0, fontawesome_icon_y_offset],
                                                         rasterizer_multiply: 1.75,
                                                         glyph_ranges:
                                                             FontGlyphRanges::from_slice(&[0xf000,
                                                                                           0xf8ed, // the range for font awesome solid 900 // XXX: only up to the violin, it probably goes higher
                                                                                           0]),
                                                         pixel_snap_h: true,
                                                         oversample_h: 1,
                                                         ..FontConfig::default() }) },
         FontSource::TtfData { data: include_bytes!("../../fonts/fa-brands-400.ttf"),
                               size_pixels: icon_font_size,
                               config: Some(FontConfig { glyph_offset:
                                                             [0.0, fontawesome_icon_y_offset],
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
    imgui.io_mut().config_windows_move_from_title_bar_only = true;
    let mut renderer = Renderer::init(&mut imgui, &display).expect("Failed to initialize renderer");
    let mut last_frame = Instant::now();

    loop {
        // TODO: maybe there's a way not to do this on every iteration of the loop to be more efficient?
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

        event_loop.run_return(|event, _, control_flow| match event {
                      Event::NewEvents(_) => {
                          let now = Instant::now();
                          imgui.io_mut().update_delta_time(now - last_frame);
                          last_frame = now;
                      }
                      Event::MainEventsCleared => {
                          let gl_window = display.gl_window();
                          platform.prepare_frame(imgui.io_mut(), &gl_window.window())
                                  .expect("Failed to start frame");
                          gl_window.window().request_redraw();
                      }
                      Event::RedrawRequested(_) => {
                          let ui = imgui.frame();
                          if !run_ui(&ui, keypress) {
                              *control_flow = ControlFlow::Exit;
                          }
                          keypress = None;
                          let gl_window = display.gl_window();
                          let mut target = display.draw();
                          target.clear_color_srgb(1.0, 1.0, 1.0, 1.0);
                          platform.prepare_render(&ui, &gl_window.window());
                          renderer.render(&mut target, ui.render())
                                  .expect("Rendering failed");
                          target.finish().unwrap();
                      }
                      Event::WindowEvent { event: WindowEvent::CloseRequested,
                                           .. } => *control_flow = ControlFlow::Exit,
                      event => {
                          let gl_window = display.gl_window();
                          platform.handle_event(imgui.io_mut(), &gl_window.window(), &event);

                          if let Event::WindowEvent { event:
                                                          WindowEvent::KeyboardInput { input,
                                                                                       .. },
                                                      .. } = event
                          {
                              let pressed = input.state == Pressed;
                              if pressed {
                                  match input.virtual_keycode {
                                      Some(key) => {
                                          if let Some(key) = map_key(key) {
                                              let io = imgui.io();
                                              keypress = Some(Keypress::new(key,
                                                                            io.key_ctrl,
                                                                            io.key_shift))
                                          }
                                      }
                                      _ => {}
                                  }
                              }
                          }
                      }
                  });
    }
}

fn map_key(key: VirtualKeyCode) -> Option<AppKey> {
    match key {
        VirtualKeyCode::A => Some(AppKey::A),
        VirtualKeyCode::B => Some(AppKey::B),
        VirtualKeyCode::C => Some(AppKey::C),
        VirtualKeyCode::D => Some(AppKey::D),
        VirtualKeyCode::E => Some(AppKey::E),
        VirtualKeyCode::H => Some(AppKey::H),
        VirtualKeyCode::J => Some(AppKey::J),
        VirtualKeyCode::K => Some(AppKey::K),
        VirtualKeyCode::L => Some(AppKey::L),
        VirtualKeyCode::W => Some(AppKey::W),
        VirtualKeyCode::X => Some(AppKey::X),
        VirtualKeyCode::R => Some(AppKey::R),
        VirtualKeyCode::O => Some(AppKey::O),
        VirtualKeyCode::U => Some(AppKey::U),
        VirtualKeyCode::V => Some(AppKey::V),
        VirtualKeyCode::Delete => Some(AppKey::Delete),
        VirtualKeyCode::Return => Some(AppKey::Enter),
        VirtualKeyCode::Escape => Some(AppKey::Escape),
        VirtualKeyCode::Tab => Some(AppKey::Tab),
        VirtualKeyCode::Up => Some(AppKey::UpArrow),
        VirtualKeyCode::Down => Some(AppKey::DownArrow),
        VirtualKeyCode::Left => Some(AppKey::LeftArrow),
        VirtualKeyCode::Right => Some(AppKey::RightArrow),
        _ => None,
    }
}

// from https://github.com/Gekkio/imgui-rs/blob/master/imgui-examples/examples/support/clipboard.rs
use clipboard::{ClipboardContext, ClipboardProvider};
use glutin::event::ElementState::Pressed;
use glutin::event::{Event, WindowEvent};
use glutin::event_loop::ControlFlow;
use imgui::{ClipboardBackend, ImStr, ImString};
use winit::event::VirtualKeyCode;
use winit::platform::desktop::EventLoopExtDesktop;

pub struct ClipboardSupport(ClipboardContext);

pub fn init_clipboard() -> Option<ClipboardSupport> {
    ClipboardContext::new().ok()
                           .map(|ctx| ClipboardSupport(ctx))
}
impl ClipboardBackend for ClipboardSupport {
    fn get(&mut self) -> Option<ImString> {
        self.0.get_contents().ok().map(|text| text.into())
    }
    fn set(&mut self, text: &ImStr) {
        let _ = self.0.set_contents(text.to_str().to_owned());
    }
}
