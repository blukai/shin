use anyhow::Context as _;
use fontdue::layout::{Layout as TextLayout, TextStyle};
use glam::Vec2;
use gpu::{
    egl,
    gl::{self, GlContexter},
};
use raw_window_handle as rwh;
use std::{
    collections::{HashMap, hash_map},
    ffi::c_void,
    ptr::null,
};
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
    ftc: uhi::FontTextureCache,
    ftc_textures: HashMap<uhi::FontTextureCachePageHandle, gl::Texture>,
    font_handle: uhi::FontTextureCacheFontHandle,
    text_layout: TextLayout,
    renderer: uhi::GlRenderer,

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
    ) -> anyhow::Result<&mut InitializedGraphicsContext> {
        assert!(matches!(self, Self::Uninit));

        let context = egl::Context::new(
            display_handle,
            egl::Config {
                min_swap_interval: Some(0),
                ..egl::Config::default()
            },
        )?;
        let surface = egl::Surface::new(&context, window_handle)?;

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

        let mut ftc = uhi::FontTextureCache::default();
        let ftc_textures = HashMap::default();

        let font_handle = ftc
            .create_font(FONT, 14.0)
            .context("could not create font")?;
        let text_layout =
            fontdue::layout::Layout::new(fontdue::layout::CoordinateSystem::PositiveYDown);

        let renderer = uhi::GlRenderer::new(&gl)?;

        *self = Self::Initialized(InitializedGraphicsContext {
            draw_buffer,
            ftc,
            ftc_textures,
            font_handle,
            text_layout,
            renderer,

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
    gpu_context: GraphicsContext,
    close_requested: bool,
}

impl Context {
    fn new() -> anyhow::Result<Self> {
        let window = window::create_window(WindowAttrs::default())?;
        let gpu_context = GraphicsContext::new_uninit();

        Ok(Self {
            window,
            gpu_context,
            close_requested: false,
        })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        self.window.pump_events()?;

        while let Some(event) = self.window.pop_event() {
            log::debug!("event: {event:?}");

            match event {
                WindowEvent::Configure { logical_size } => match self.gpu_context {
                    GraphicsContext::Uninit => {
                        let igc = self
                            .gpu_context
                            .init(self.window.display_handle()?, self.window.window_handle()?)?;

                        igc.surface.resize(logical_size.0, logical_size.1)?;
                    }
                    GraphicsContext::Initialized(ref mut igc) => {
                        igc.surface.resize(logical_size.0, logical_size.1)?;
                    }
                },
                WindowEvent::CloseRequested => {
                    self.close_requested = true;
                    return Ok(());
                }
            }
        }

        if let GraphicsContext::Initialized(InitializedGraphicsContext {
            ref mut draw_buffer,
            ref mut ftc,
            ref mut ftc_textures,
            font_handle,
            ref mut text_layout,
            ref renderer,

            ref context,
            ref surface,
            ref gl,
        }) = self.gpu_context
        {
            draw_buffer.push_rect(uhi::RectShape::with_fill(
                uhi::Rect::from_center_size(Vec2::new(800.0 / 2.0, 600.0 / 2.0), 100.0),
                uhi::Fill::with_color(uhi::Rgba8::FUCHSIA),
            ));

            text_layout.append(
                &[ftc.get_fontdue_font(font_handle)],
                &TextStyle::new("hello, sailor!", 14.0, 0),
            );
            let glyphs = text_layout.glyphs();

            // rasterize glyphs in cache (that were not yet rasterized)
            for glyph in glyphs.iter() {
                if !ftc.contains_char(font_handle, glyph.parent) {
                    ftc.allocate_char(font_handle, glyph.parent);
                }
            }

            // create textures (if not yet created) and upload bitmaps (if not yet uploaded)
            let (tex_width, tex_height) = ftc.get_page_texture_size();
            for (page_handle, page) in ftc.iter_dirty_pages_mut() {
                let tex_entry = match ftc_textures.entry(page_handle) {
                    hash_map::Entry::Occupied(occupied) => occupied,
                    hash_map::Entry::Vacant(vacant) => vacant.insert_entry(unsafe {
                        let texture = gl
                            .create_texture()
                            .context("could not create ftc page tex")?;
                        gl.bind_texture(gl::TEXTURE_2D, Some(texture));

                        // NOTE: without those params you can't see shit in this mist
                        gl.tex_parameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
                        gl.tex_parameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);

                        // NOTE: this fixes tilting when rendering bitmaps. see
                        // https://stackoverflow.com/questions/15983607/opengl-texture-tilted.
                        gl.pixel_storei(gl::UNPACK_ALIGNMENT, 1);

                        // NOTE: this makes so that in the shader colors look like rgba 0 0 0 red,
                        // instead of just red. see
                        // https://www.khronos.org/opengl/wiki/Texture#Swizzle_mask
                        gl.tex_parameteriv(
                            gl::TEXTURE_2D,
                            gl::TEXTURE_SWIZZLE_RGBA,
                            [
                                gl::ONE as gl::GLint,
                                gl::ONE as gl::GLint,
                                gl::ONE as gl::GLint,
                                gl::RED as gl::GLint,
                            ]
                            .as_ptr(),
                        );

                        gl.tex_image_2d(
                            gl::TEXTURE_2D,
                            0,
                            gl::R8 as gl::GLint,
                            tex_width as gl::GLint,
                            tex_height as gl::GLint,
                            0,
                            gl::RED,
                            gl::UNSIGNED_BYTE,
                            null(),
                        );

                        texture
                    }),
                };

                for (rect, bitmap) in page.drain_bitmaps() {
                    unsafe {
                        gl.bind_texture(gl::TEXTURE_2D, Some(*tex_entry.get()));
                        gl.tex_sub_image_2d(
                            gl::TEXTURE_2D,
                            0,
                            rect.min.x as gl::GLint,
                            rect.min.y as gl::GLint,
                            rect.width() as gl::GLsizei,
                            rect.height() as gl::GLsizei,
                            gl::RED,
                            gl::UNSIGNED_BYTE,
                            bitmap.as_ptr() as *const c_void,
                        );
                    }
                }
            }

            // finally ready to put chars on the screen xd
            for glyph in glyphs.iter() {
                let (page_handle, tex_coords) = ftc.get_texture_for_char(font_handle, glyph.parent);
                let tex = ftc_textures.get(&page_handle).expect("page texture");

                let min = Vec2::new(glyph.x, glyph.y);
                let size = Vec2::new(glyph.width as f32, glyph.height as f32);
                draw_buffer.push_rect(uhi::RectShape::with_fill(
                    uhi::Rect::new(min, min + size),
                    uhi::Fill::new(
                        uhi::Rgba8::ORANGE,
                        uhi::FillTexture {
                            handle: *tex,
                            coords: tex_coords,
                        },
                    ),
                ));
            }

            unsafe {
                context.make_current(surface.as_ptr())?;

                gl.clear_color(0.0, 0.0, 0.3, 1.0);
                gl.clear(gl::COLOR_BUFFER_BIT);

                renderer.render(gl, (800, 600), draw_buffer);

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
