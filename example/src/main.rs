use std::iter;

use anyhow::{Context as _, anyhow};
use gl::Apier as _;
use raw_window_handle as rwh;
use window::{Event, Window, WindowAttrs, WindowEvent};

use example::{GlContext, GlRenderer};

const DEFAULT_FONT_DATA: &[u8] = include_bytes!("../fixtures/JetBrainsMono-Regular.ttf");

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let msg = format!(
            "{level:<5} {target}:{line:<4} > {text}",
            level = record.level(),
            target = record.target(),
            line = record
                .line()
                .map_or_else(|| "?".to_string(), |line| line.to_string()),
            text = record.args(),
        );

        #[cfg(not(target_family = "wasm"))]
        eprintln!("{msg}");

        #[cfg(target_family = "wasm")]
        js::GLOBAL
            .get("console")
            .get("log")
            .call(&[js::Value::from_str(msg.as_str())])
            .expect("could not log");
    }

    fn flush(&self) {}
}

impl Logger {
    fn init() {
        log::set_logger(&Logger).expect("could not set logger");
        log::set_max_level(log::LevelFilter::Trace);
    }
}

fn draw_text<E: sx::Externs>(
    text: &str,
    mut font_instance: sx::FontInstanceRefMut,
    fg: sx::Rgba,
    position: sx::Vec2,
    texture_service: &mut sx::TextureService,
    draw_buffer: &mut sx::DrawBuffer<E>,
) {
    let font_ascent = font_instance.ascent();
    let mut x_offset: f32 = position.x;
    for ch in text.chars() {
        let glyph = font_instance.get_or_rasterize_glyph(ch, texture_service);
        let glyph_advance_width = glyph.advance_width();
        draw_buffer.push_rect(sx::RectShape::new_with_fill(
            glyph
                .bounding_rect()
                .translate(sx::Vec2::new(x_offset, position.y + font_ascent)),
            sx::Fill::new(
                fg,
                sx::FillTexture {
                    texture: sx::TextureHandleKind::Internal(glyph.texture_handle()),
                    coords: glyph.texture_coords(),
                },
            ),
        ));
        x_offset += glyph_advance_width;
    }
}

struct Context {
    window: Box<dyn Window>,
    close_requested: bool,
    input: input::State,
    gl_context: GlContext,

    texture_service: sx::TextureService,
    font_service: sx::FontService,
    default_font_handle: sx::FontHandle,
    draw_buffer: sx::DrawBuffer<GlRenderer>,
    gl_renderer: GlRenderer,
}

impl Context {
    fn new() -> anyhow::Result<Self> {
        let window =
            window::create_window(WindowAttrs::default()).context("could not create window")?;

        let gl_context = {
            #[cfg(not(target_family = "wasm"))]
            {
                let display_handle = window
                    .display_handle()
                    .context("display handle is unavailable")?;
                match display_handle.as_raw() {
                    rwh::RawDisplayHandle::Wayland(wayland_display) => {
                        GlContext::from_wayland_display(wayland_display.display.as_ptr())?
                    }
                    _ => return Err(anyhow!(format!("unsupported display: {display_handle:?}"))),
                }
            }

            #[cfg(target_family = "wasm")]
            {
                let window_handle = window
                    .window_handle()
                    .context("window handle is unavailable")?;
                match window_handle.as_raw() {
                    rwh::RawWindowHandle::Web(web) => {
                        let canvas_selector = format!("canvas[data-raw-handle=\"{}\"]", web.id);
                        GlContext::from_canvas_selector(canvas_selector.as_str())?
                    }
                    _ => return Err(anyhow!(format!("unsupported window: {window_handle:?}"))),
                }
            }
        };

        let mut font_service = sx::FontService::default();
        let default_font_handle = font_service
            .register_font_slice(DEFAULT_FONT_DATA)
            .context("default font is invalid")?;
        let gl_renderer =
            GlRenderer::new(&gl_context.api).context("could not create gl renderer")?;

        Ok(Self {
            window,
            close_requested: false,
            input: input::State::default(),
            gl_context,

            texture_service: sx::TextureService::default(),
            font_service,
            default_font_handle,
            draw_buffer: sx::DrawBuffer::default(),
            gl_renderer,
        })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        self.window.pump_events()?;
        let events = iter::from_fn(|| self.window.pop_event()).filter_map(|event| match event {
            Event::Window(window_event) => {
                if matches!(window_event, WindowEvent::CloseRequested) {
                    self.close_requested = true;
                }
                None
            }
            Event::Pointer(pointer_event) => Some(input::Event::Pointer(pointer_event)),
            Event::Keyboard(keyboard_event) => Some(input::Event::Keyboard(keyboard_event)),
        });
        self.input.handle_events(events);

        self.draw_buffer.clear();
        self.font_service
            .remove_unused_font_instances(&mut self.texture_service);

        let raw_window_handle = self
            .window
            .window_handle()
            .context("window handle is unavailable")?
            .as_raw();

        let logical_size = sx::U32Vec2::from(self.window.logical_size()).as_vec2();
        let scale_factor = self.window.scale_factor() as f32;
        let physical_size = logical_size * scale_factor;

        self.gl_context.make_window_current(
            raw_window_handle,
            physical_size.x as u32,
            physical_size.y as u32,
        )?;

        unsafe { self.gl_context.api.clear_color(0.0, 0.0, 0.4, 1.0) };
        unsafe { self.gl_context.api.clear(gl::COLOR_BUFFER_BIT) };

        let font_instance = self.font_service.get_or_create_font_instance(
            self.default_font_handle,
            16.0,
            scale_factor,
        );
        draw_text(
            "hello sailor!",
            font_instance,
            sx::Rgba::WHITE,
            sx::Vec2::splat(24.0),
            &mut self.texture_service,
            &mut self.draw_buffer,
        );

        self.gl_renderer
            .handle_texture_commands(self.texture_service.drain_comands(), &self.gl_context.api)
            .context("could not update textures")?;
        self.gl_renderer
            .render(
                logical_size,
                scale_factor,
                &mut self.draw_buffer,
                &self.gl_context.api,
            )
            .context("could not render")?;

        self.gl_context.swap_window_buffers(raw_window_handle)?;

        Ok(())
    }
}

#[cfg(target_family = "wasm")]
fn request_animation_frame_loop<F>(mut f: F)
where
    F: FnMut() + 'static,
{
    use std::rc::Rc;

    let request_animation_frame = js::GLOBAL.get("requestAnimationFrame");

    let cb = Rc::<js::Closure<dyn FnMut()>>::new_uninit();
    let closure = js::Closure::new({
        let request_animation_frame = request_animation_frame.clone();
        let cb = unsafe { Rc::clone(&cb).assume_init() };
        Box::new(move || {
            f();
            request_animation_frame
                .call(&[js::Value::from_closure(&cb)])
                .expect("could not request animation frame");
        })
    });
    let cb = unsafe {
        let ptr = Rc::as_ptr(&cb) as *mut js::Closure<dyn FnMut()>;
        ptr.write(closure);
        cb.assume_init()
    };

    request_animation_frame
        .call(&[js::Value::from_closure(cb.as_ref())])
        .expect("could not request animation frame");
}

fn main() {
    #[cfg(target_family = "wasm")]
    std::panic::set_hook(Box::new(|panic_info| {
        js::throw_str(&panic_info.to_string());
    }));

    Logger::init();

    let mut ctx = Context::new().expect("could not create app");

    #[cfg(not(target_family = "wasm"))]
    while !ctx.close_requested {
        ctx.iterate().expect("iteration failure");
    }

    #[cfg(target_family = "wasm")]
    request_animation_frame_loop(move || {
        ctx.iterate().expect("iteration failure");
    });
}
