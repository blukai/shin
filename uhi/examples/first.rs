use anyhow::Context as _;
use fontdue::layout::{Layout as TextLayout, TextStyle};
use glam::Vec2;
use gpu::{
    egl,
    gl::{self, GlContexter},
};
use raw_window_handle as rwh;
use std::ffi::c_void;
use window::{Window, WindowAttrs, WindowEvent};

const FONT: &[u8] = include_bytes!("../../fixtures/JetBrainsMono-Regular.ttf");

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        println!(
            "{level:<5} {file}:{line} > {text}",
            level = record.level(),
            file = record.file().unwrap_or_else(|| record.target()),
            line = record
                .line()
                .map_or_else(|| "??".to_string(), |line| line.to_string()),
            text = record.args(),
        );
    }

    fn flush(&self) {}
}

impl Logger {
    fn init() {
        log::set_logger(&Logger).expect("could not set logger");
        log::set_max_level(log::LevelFilter::Trace);
    }
}

struct InitializedGraphicsContext {
    draw_buffer: uhi::DrawBuffer<uhi::GlRenderer>,
    texture_service: uhi::TextureService<uhi::GlRenderer>,
    font_service: uhi::FontService,
    renderer: uhi::GlRenderer,

    font_handle: uhi::FontHandle,
    text_layout: TextLayout,

    context: egl::Context,
    surface: egl::Surface,
    gl: gl::Context,
}

enum GraphicsContext {
    Initialized(InitializedGraphicsContext),
    Uninit,
}

impl GraphicsContext {
    fn new_uninit() -> Self {
        Self::Uninit
    }

    fn init(
        &mut self,
        display_handle: rwh::DisplayHandle,
        window_handle: rwh::WindowHandle,
        width: u32,
        height: u32,
    ) -> anyhow::Result<&mut InitializedGraphicsContext> {
        assert!(matches!(self, Self::Uninit));

        let context = egl::Context::new(
            display_handle,
            egl::Config {
                min_swap_interval: Some(0),
                ..egl::Config::default()
            },
        )?;
        let surface = egl::Surface::new(&context, window_handle, width, height)?;

        context.make_current(surface.as_ptr())?;

        let gl = unsafe {
            gl::Context::load_with(|procname| context.get_proc_address(procname) as *mut c_void)
        };

        let version = unsafe { gl.get_string(gl::VERSION) }.context("could not get version")?;
        let shading_language_version = unsafe { gl.get_string(gl::SHADING_LANGUAGE_VERSION) }
            .context("could not get shading language version")?;
        log::info!(
            "initialized gl version {version}, shading language version {shading_language_version}"
        );

        let draw_buffer = uhi::DrawBuffer::default();
        let texture_service = uhi::TextureService::<uhi::GlRenderer>::default();
        let mut font_service = uhi::FontService::default();
        let renderer = uhi::GlRenderer::new(&gl)?;

        let font_handle = font_service
            .create_font(FONT, 14.0)
            .context("could not create font")?;
        let text_layout =
            fontdue::layout::Layout::new(fontdue::layout::CoordinateSystem::PositiveYDown);

        *self = Self::Initialized(InitializedGraphicsContext {
            draw_buffer,
            texture_service,
            font_service,
            renderer,

            font_handle,
            text_layout,

            context,
            surface,
            gl,
        });
        let Self::Initialized(init) = self else {
            unreachable!();
        };
        Ok(init)
    }
}

struct Context {
    window: Box<dyn Window>,
    window_size: (u32, u32),
    graphics_context: GraphicsContext,
    close_requested: bool,
}

impl Context {
    fn new() -> anyhow::Result<Self> {
        let window = window::create_window(WindowAttrs::default())?;
        let graphics_context = GraphicsContext::new_uninit();

        Ok(Self {
            window,
            window_size: (0, 0),
            graphics_context,
            close_requested: false,
        })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        self.window.pump_events()?;

        while let Some(event) = self.window.pop_event() {
            log::debug!("event: {event:?}");

            match event {
                WindowEvent::Configure { logical_size } => {
                    self.window_size = logical_size;

                    match self.graphics_context {
                        GraphicsContext::Uninit => {
                            self.graphics_context.init(
                                self.window.display_handle()?,
                                self.window.window_handle()?,
                                logical_size.0,
                                logical_size.1,
                            )?;
                        }
                        GraphicsContext::Initialized(ref mut igc) => {
                            igc.surface.resize(logical_size.0, logical_size.1)?;
                        }
                    }
                }
                WindowEvent::Resize { physical_size } => {
                    self.window_size = physical_size;

                    if let GraphicsContext::Initialized(ref mut igc) = self.graphics_context {
                        igc.surface.resize(physical_size.0, physical_size.1)?;
                    }
                }
                WindowEvent::CloseRequested => {
                    self.close_requested = true;
                    return Ok(());
                }
            }
        }

        if let GraphicsContext::Initialized(InitializedGraphicsContext {
            ref mut draw_buffer,
            ref mut texture_service,
            ref mut font_service,
            ref renderer,

            font_handle,
            ref mut text_layout,

            ref context,
            ref surface,
            ref gl,
        }) = self.graphics_context
        {
            draw_buffer.push_rect(uhi::RectShape::with_fill(
                uhi::Rect::from_center_size(
                    Vec2::new(self.window_size.0 as f32, self.window_size.1 as f32) / 2.0,
                    100.0,
                ),
                uhi::Fill::with_color(uhi::Rgba8::FUCHSIA),
            ));

            text_layout.append(
                &[font_service.get_fontdue_font(font_handle)],
                &TextStyle::new("hello, sailor!", 14.0, 0),
            );

            for glyph in text_layout.glyphs() {
                let (handle, coords) =
                    font_service.get_texture_for_char(font_handle, glyph.parent, texture_service);

                let min = Vec2::new(glyph.x, glyph.y);
                let size = Vec2::new(glyph.width as f32, glyph.height as f32);
                draw_buffer.push_rect(uhi::RectShape::with_fill(
                    uhi::Rect::new(min, min + size),
                    uhi::Fill::new(
                        uhi::Rgba8::ORANGE,
                        uhi::FillTexture {
                            kind: uhi::TextureKind::Internal(handle),
                            coords,
                        },
                    ),
                ));
            }

            unsafe {
                context.make_current(surface.as_ptr())?;

                gl.clear_color(0.0, 0.0, 0.3, 1.0);
                gl.clear(gl::COLOR_BUFFER_BIT);

                renderer.render(gl, self.window_size, draw_buffer, texture_service)?;

                context.swap_buffers(surface.as_ptr())?;
            }

            draw_buffer.clear();
            text_layout.clear();
        }

        Ok(())
    }
}

fn main() {
    Logger::init();

    let mut ctx = Context::new().expect("could not create context");

    while !ctx.close_requested {
        ctx.iterate().expect("iteration failure");
    }
}
