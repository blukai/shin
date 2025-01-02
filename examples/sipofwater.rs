use glow::HasContext as _;
use graphics::egl::{EglContext, EglSurface};
use graphics::libegl;
use raw_window_handle::{self as rwh, HasDisplayHandle as _, HasWindowHandle as _};
use window::{Event, Size, WindowConfig};

struct InitializedGraphicsContext {
    egl: libegl::Lib,
    egl_context: EglContext,
    egl_surface: EglSurface,

    gl: glow::Context,
}

impl InitializedGraphicsContext {
    #[inline]
    fn resize(&mut self, logical_size: Size) {
        self.egl_surface.resize(logical_size)
    }
}

enum GraphicsContext {
    Initialized(InitializedGraphicsContext),
    Uninitialized,
}

impl GraphicsContext {
    fn init(
        &mut self,
        display_handle: rwh::DisplayHandle,
        window_handle: rwh::WindowHandle,
        logical_size: Size,
    ) -> anyhow::Result<()> {
        assert!(matches!(self, Self::Uninitialized));

        let egl = libegl::Lib::load()?;
        let egl_context = EglContext::new(&egl, display_handle)?;
        let egl_surface = EglSurface::new(&egl, &egl_context, window_handle, logical_size)?;

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|cstr| {
                (egl.eglGetProcAddress)(cstr.as_ptr() as _) as _
            })
        };

        *self = Self::Initialized(InitializedGraphicsContext {
            egl,
            egl_context,
            egl_surface,

            gl,
        });

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut window = window::create_window(WindowConfig::default())?;
    let mut graphics_context = GraphicsContext::Uninitialized;

    'update_loop: while let Ok(_) = window.update() {
        while let Some(event) = window.pop_event() {
            log::debug!("event: {event:?}");

            match event {
                Event::Configure { logical_size } => match graphics_context {
                    GraphicsContext::Uninitialized => graphics_context.init(
                        window.display_handle()?,
                        window.window_handle()?,
                        logical_size,
                    )?,
                    GraphicsContext::Initialized(ref mut igc) => {
                        igc.resize(logical_size);
                    }
                },
                Event::CloseRequested => break 'update_loop,
            }
        }

        if let GraphicsContext::Initialized(ref igc) = graphics_context {
            unsafe {
                igc.egl_context
                    .make_current(&igc.egl, igc.egl_surface.as_ptr())?;

                igc.gl.clear_color(1.0, 0.0, 0.0, 1.0);
                igc.gl.clear(glow::COLOR_BUFFER_BIT);

                igc.egl_context
                    .swap_buffers(&igc.egl, igc.egl_surface.as_ptr())?;
            }
        }
    }

    Ok(())
}
