use anyhow::Context as _;
use glam::Vec2;
use gpu::{
    egl,
    gl::{self, GlContexter},
};
use raw_window_handle as rwh;
use std::ffi::c_void;
use window::{Event, PointerEvent, Window, WindowAttrs, WindowEvent};

const FONT: &[u8] = include_bytes!("../../fixtures/JetBrainsMono-Regular.ttf");

#[derive(Debug, Clone)]
enum UhiId {
    Pep,
}

struct UhiExterns;

impl uhi::Externs for UhiExterns {
    type WidgetId = UhiId;
    type TextureHandle = <uhi::GlRenderer as uhi::Renderer>::TextureHandle;
}

// Tableau I, by Piet Mondriaan
// https://en.wikipedia.org/wiki/File:Tableau_I,_by_Piet_Mondriaan.jpg
fn draw_mondriaan<E: uhi::Externs>(
    uhi: &mut uhi::Context<E>,
    font_handle: uhi::FontHandle,
    area: uhi::Rect,
) {
    use uhi::*;

    // TODO: figure out a nicer way to layout and draw stuff. with no heap allocations for the
    // layout!

    const SIZE: Vec2 = Vec2::new(1130.0, 1200.0);

    const TOP_HEIGHT: f32 = 640.0;
    const GAP: Constraint = uhi::Constraint::Length(20.0);
    const BOTTOM_HEIGHT: f32 = 540.0;

    let [top, gap, bottom] = vstack([
        Constraint::Percentage(TOP_HEIGHT / SIZE.y),
        GAP,
        Constraint::Percentage(BOTTOM_HEIGHT / SIZE.y),
    ])
    .split(area.clone());

    // top
    {
        uhi.draw_rect(RectShape::with_fill(
            top.clone(),
            Fill::with_color(Rgba8::WHITE),
        ));

        {
            let [left, lgap, mid, rgap, right] = hstack([
                Constraint::Percentage(500.0 / SIZE.x),
                GAP,
                Constraint::Fill(1.0),
                GAP,
                Constraint::Percentage(170.0 / SIZE.x),
            ])
            .split(top);

            {
                let [top, gap, bottom] = vstack([
                    Constraint::Percentage(10.0 / TOP_HEIGHT),
                    GAP,
                    Constraint::Fill(1.0),
                ])
                .split(left);
                uhi.draw_rect(RectShape::with_fill(top, Fill::with_color(Rgba8::RED)));
                uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));
                uhi.draw_rect(RectShape::with_fill(bottom, Fill::with_color(Rgba8::WHITE)));
            }

            uhi.draw_rect(RectShape::with_fill(lgap, Fill::with_color(Rgba8::BLACK)));
            uhi.draw_rect(RectShape::with_fill(mid, Fill::with_color(Rgba8::WHITE)));
            uhi.draw_rect(RectShape::with_fill(rgap, Fill::with_color(Rgba8::BLACK)));

            {
                let [top, tgap, mid, bgap, bottom] = vstack([
                    Constraint::Percentage(80.0 / TOP_HEIGHT),
                    GAP,
                    Constraint::Fill(1.0),
                    GAP,
                    Constraint::Percentage(130.0 / TOP_HEIGHT),
                ])
                .split(right);
                uhi.draw_rect(RectShape::with_fill(top, Fill::with_color(Rgba8::BLACK)));
                uhi.draw_rect(RectShape::with_fill(tgap, Fill::with_color(Rgba8::BLACK)));
                uhi.draw_rect(RectShape::with_fill(mid, Fill::with_color(Rgba8::WHITE)));
                uhi.draw_rect(RectShape::with_fill(bgap, Fill::with_color(Rgba8::BLACK)));
                uhi.draw_rect(RectShape::with_fill(bottom, Fill::with_color(Rgba8::WHITE)));
            }
        }
    }

    uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));

    // bottom
    {
        uhi.draw_rect(RectShape::with_fill(
            bottom.clone(),
            Fill::with_color(Rgba8::WHITE),
        ));

        {
            let [left, lgap, _, rgap, right] = hstack([
                Constraint::Percentage(500.0 / SIZE.x),
                GAP,
                Constraint::Fill(1.0),
                GAP,
                Constraint::Percentage(170.0 / SIZE.x),
            ])
            .split(bottom);

            let lmgap;
            {
                let [_, gap, right] = hstack([
                    Constraint::Percentage(100.0 / SIZE.x),
                    GAP,
                    Constraint::Fill(1.0),
                ])
                .split(left);
                lmgap = gap.clone();
                uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));

                {
                    let [top, gap, _] = vstack([
                        Constraint::Percentage(300.0 / BOTTOM_HEIGHT),
                        GAP,
                        Constraint::Fill(1.0),
                    ])
                    .split(right);
                    uhi.draw_rect(RectShape::with_fill(top, Fill::with_color(Rgba8::BLUE)));
                    uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));
                }
            }

            uhi.draw_rect(RectShape::with_fill(
                lgap.clone(),
                Fill::with_color(Rgba8::BLACK),
            ));
            uhi.draw_rect(RectShape::with_fill(
                rgap.clone(),
                Fill::with_color(Rgba8::BLACK),
            ));
            uhi.draw_rect(RectShape::with_fill(right, Fill::with_color(Rgba8::ORANGE)));

            // bottom-mid section [white .......... black ..]
            {
                let min_x = lmgap.max.x;
                let max_x = rgap.min.x;
                let [_, gap, bottom] = vstack([
                    Constraint::Fill(1.0),
                    GAP,
                    Constraint::Percentage(30.0 / SIZE.y),
                ])
                .split(Rect::new(
                    Vec2::new(min_x, 0.0),
                    Vec2::new(max_x, area.max.y),
                ));
                uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));

                {
                    let [left, gap, right] = hstack([
                        Constraint::Fill(1.0),
                        GAP,
                        Constraint::Percentage(100.0 / SIZE.x),
                    ])
                    .split(bottom);
                    uhi.draw_rect(RectShape::with_fill(left, Fill::with_color(Rgba8::WHITE)));
                    uhi.draw_rect(RectShape::with_fill(gap, Fill::with_color(Rgba8::BLACK)));
                    uhi.draw_rect(RectShape::with_fill(right, Fill::with_color(Rgba8::BLACK)));
                }
            }
        }
    }

    let text = "Tableau I, by Piet Mondriaan";
    let text_width = uhi
        .font_service
        .get_text_width(text, font_handle, &mut uhi.texture_service);
    let font_line_height = uhi.font_service.get_font_line_height(font_handle);
    let text_size = Vec2::new(text_width, font_line_height);
    let text_position = area.size() - Vec2::splat(24.0) - text_size;
    uhi.draw_rect(RectShape::with_fill(
        Rect::new(text_position, text_position + text_size),
        Fill::with_color(Rgba8::new(128, 128, 128, 128)),
    ));
    uhi.draw_text(text, font_handle, text_position, Rgba8::WHITE);
}

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
    uhi: uhi::Context<UhiExterns>,
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

        let mut uhi = uhi::Context::default();
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
            if !matches!(event, Event::Pointer(window::PointerEvent::Motion { .. })) {
                log::debug!("event: {event:?}");
            }

            match event {
                Event::Window(WindowEvent::Configure { logical_size }) => {
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
                Event::Window(WindowEvent::Resize { physical_size }) => {
                    self.window_size = physical_size;

                    if let GraphicsContext::Initialized(ref mut igc) = self.graphics_context {
                        igc.egl_surface.resize(physical_size.0, physical_size.1)?;
                    }
                }
                Event::Window(WindowEvent::CloseRequested) => {
                    self.close_requested = true;
                    return Ok(());
                }
                _ => {}
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
            let window_size = Vec2::new(self.window_size.0 as f32, self.window_size.1 as f32);

            draw_mondriaan(uhi, font_handle, uhi::Rect::new(Vec2::ZERO, window_size));

            // TextEdit::new(UhiId::Pep, &mut "kek".to_string()).draw(uhi, font_handle);

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

// TODO: figure out input state.
//
// widgets don't have to have event handler and drawer. it would be nicer to combine everything
// into a single function.

