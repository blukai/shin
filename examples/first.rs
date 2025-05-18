use graphics::egl::{EglConfig, EglContext, EglSurface};
use graphics::{gl, libegl};
use raw_window_handle::{self as rwh, HasDisplayHandle as _, HasWindowHandle as _};
use window::{Window, WindowAttrs, WindowEvent};

struct InitializedGraphicsContext {
    egl: libegl::Lib,
    egl_context: EglContext,
    egl_surface: EglSurface,

    gl: gl::sys::Api,
}

impl InitializedGraphicsContext {
    #[inline]
    fn resize(&mut self, logical_size: (u32, u32)) {
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
        logical_size: (u32, u32),
    ) -> anyhow::Result<()> {
        assert!(matches!(self, Self::Uninitialized));

        let egl = libegl::Lib::load()?;
        let egl_context = EglContext::new(
            &egl,
            display_handle,
            EglConfig {
                min_swap_interval: Some(0),
                ..EglConfig::default()
            },
        )?;
        let egl_surface = EglSurface::new(&egl, &egl_context, window_handle, logical_size)?;

        // TODO: figure out an okay way to include vsync toggle.
        // egl_context.make_current(&egl, egl_surface.as_ptr())?;
        // egl_context.set_swap_interval(&egl, 0)?;

        let gl = unsafe {
            gl::sys::Api::load_with(|procname| (egl.eglGetProcAddress)(procname as _) as _)
        };
        // log::info!("initialized gl version {:?}", gl.version());

        *self = Self::Initialized(InitializedGraphicsContext {
            egl,
            egl_context,
            egl_surface,

            gl,
        });

        Ok(())
    }
}

struct Context {
    window: Box<dyn Window>,
    graphics_context: GraphicsContext,
    close_requested: bool,
}

impl Context {
    fn new() -> anyhow::Result<Self> {
        let window = window::create_window(WindowAttrs::default())?;
        let graphics_context = GraphicsContext::Uninitialized;

        Ok(Self {
            window,
            graphics_context,
            close_requested: false,
        })
    }

    fn iterate(&mut self) -> anyhow::Result<()> {
        self.window.pump_events()?;

        while let Some(event) = self.window.pop_event() {
            log::debug!("event: {event:?}");

            match event {
                WindowEvent::Configure { logical_size } => match self.graphics_context {
                    GraphicsContext::Uninitialized => self.graphics_context.init(
                        self.window.display_handle()?,
                        self.window.window_handle()?,
                        logical_size,
                    )?,
                    GraphicsContext::Initialized(ref mut igc) => {
                        igc.resize(logical_size);
                    }
                },
                WindowEvent::CloseRequested => {
                    self.close_requested = true;
                    return Ok(());
                }
            }
        }

        if let GraphicsContext::Initialized(ref igc) = self.graphics_context {
            unsafe {
                igc.egl_context
                    .make_current(&igc.egl, igc.egl_surface.as_ptr())?;

                igc.gl.ClearColor(1.0, 0.0, 0.0, 1.0);
                igc.gl.Clear(gl::sys::COLOR_BUFFER_BIT);

                igc.egl_context
                    .swap_buffers(&igc.egl, igc.egl_surface.as_ptr())?;
            }
        }

        Ok(())
    }
}

fn main() {
    env_logger::init();

    let mut ctx = Context::new().expect("could not create context");
    while !ctx.close_requested {
        ctx.iterate().expect("iteration failure");
    }
}
