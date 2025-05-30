use anyhow::Context as _;
use glam::Vec2;
use gpu::{
    egl,
    gl::{self, GlContexter},
};
use raw_window_handle as rwh;
use std::ffi::c_void;
use uhi::TextLayoutSttings;
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
    egl_context: egl::Context,
    egl_surface: egl::Surface,
    gl: gl::Context,
    uhi: uhi::Context<uhi::GlRenderer>,
    uhi_renderer: uhi::GlRenderer,
    font_handle: uhi::FontHandle,
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

        let egl_context = egl::Context::new(
            display_handle,
            egl::Config {
                min_swap_interval: Some(0),
                ..egl::Config::default()
            },
        )?;
        let egl_surface = egl::Surface::new(&egl_context, window_handle, width, height)?;

        egl_context.make_current(egl_surface.as_ptr())?;

        let gl = unsafe {
            gl::Context::load_with(|procname| egl_context.get_proc_address(procname) as *mut c_void)
        };

        let version = unsafe { gl.get_string(gl::VERSION) }.context("could not get version")?;
        let shading_language_version = unsafe { gl.get_string(gl::SHADING_LANGUAGE_VERSION) }
            .context("could not get shading language version")?;
        log::info!(
            "initialized gl version {version}, shading language version {shading_language_version}"
        );

        let mut uhi = uhi::Context::new(false);
        let uhi_renderer = uhi::GlRenderer::new(&gl)?;
        let font_handle = uhi
            .font_service
            .create_font(FONT, 24.0)
            .context("could not create font")?;

        *self = Self::Initialized(InitializedGraphicsContext {
            egl_context,
            egl_surface,
            gl,

            uhi,
            uhi_renderer,
            font_handle,
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
                            igc.egl_surface.resize(logical_size.0, logical_size.1)?;
                        }
                    }
                }
                WindowEvent::Resize { physical_size } => {
                    self.window_size = physical_size;

                    if let GraphicsContext::Initialized(ref mut igc) = self.graphics_context {
                        igc.egl_surface.resize(physical_size.0, physical_size.1)?;
                    }
                }
                WindowEvent::CloseRequested => {
                    self.close_requested = true;
                    return Ok(());
                }
            }
        }

        if let GraphicsContext::Initialized(InitializedGraphicsContext {
            ref mut uhi,
            ref uhi_renderer,
            font_handle,

            ref egl_context,
            ref egl_surface,
            ref gl,
        }) = self.graphics_context
        {
            uhi.push_rect(uhi::RectShape::with_fill(
                uhi::Rect::from_center_size(
                    Vec2::new(self.window_size.0 as f32, self.window_size.1 as f32) / 2.0,
                    100.0,
                ),
                uhi::Fill::with_color(uhi::Rgba8::FUCHSIA),
            ));
            uhi.push_text(
                font_handle,
                "YO, sailor!",
                uhi::Rgba8::WHITE,
                Some(&TextLayoutSttings {
                    max_width: Some(self.window_size.0 as f32),
                    max_height: Some(self.window_size.1 as f32),
                    vertical_align: uhi::TextVAlign::Middle,
                    horizontal_align: uhi::TextHAlign::Center,
                    ..TextLayoutSttings::default()
                }),
            );

            unsafe {
                egl_context.make_current(egl_surface.as_ptr())?;

                gl.clear_color(0.0, 0.0, 0.3, 1.0);
                gl.clear(gl::COLOR_BUFFER_BIT);

                uhi_renderer.render(uhi, gl, self.window_size)?;

                egl_context.swap_buffers(egl_surface.as_ptr())?;
            }

            uhi.clear_draw_buffer();
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
