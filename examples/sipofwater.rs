use std::{
    ffi::c_void,
    ptr::{NonNull, null},
    rc::Rc,
};

use anyhow::anyhow;
use g0::{egl::EglContext, libegl, libwayland_egl};
use glow::HasContext;
use w0::{Event, EventLoop as _, Size, WindowConfig, libwayland_client};

struct InitializedGraphicsContext {
    egl: Rc<libegl::Lib>,
    wayland_egl: libwayland_egl::Lib,
    gl: glow::Context,

    egl_context: EglContext,
    wl_egl_window: *mut libwayland_egl::wl_egl_window,
    egl_surface: libegl::EGLSurface,

    logical_size: Size,
}

impl InitializedGraphicsContext {
    fn resize(&mut self, logical_size: Size) {
        unsafe {
            (self.wayland_egl.wl_egl_window_resize)(
                self.wl_egl_window,
                logical_size.width as i32,
                logical_size.height as i32,
                0,
                0,
            );
        }

        self.logical_size = logical_size;
    }
}

enum GraphicsContext {
    Initialized(InitializedGraphicsContext),
    Uninitialized,
}

impl GraphicsContext {
    fn init(
        &mut self,
        display_handle: NonNull<c_void>,
        window_handle: NonNull<c_void>,
        logical_size: Size,
    ) -> anyhow::Result<()> {
        assert!(matches!(self, Self::Uninitialized));

        let egl = Rc::new(libegl::Lib::load()?);
        let wayland_egl = libwayland_egl::Lib::load()?;

        let egl_context = unsafe { EglContext::new(&egl, display_handle.as_ptr())? };

        let wl_egl_window = unsafe {
            (wayland_egl.wl_egl_window_create)(
                window_handle.as_ptr() as *mut libwayland_client::wl_surface,
                logical_size.width as i32,
                logical_size.height as i32,
            )
        };
        if wl_egl_window.is_null() {
            return Err(anyhow!("could not create wl egl window"));
        }

        let egl_surface = unsafe {
            (egl.eglCreateWindowSurface)(
                egl_context.display,
                egl_context.config,
                wl_egl_window as libegl::EGLNativeWindowType,
                null(),
            )
        };
        if egl_surface.is_null() {
            return Err(anyhow!("could not create egl surface"));
        }

        unsafe {
            egl_context.make_current_surfaceless()?;
        }

        let gl = unsafe {
            glow::Context::from_loader_function_cstr(|cstr| {
                (egl.eglGetProcAddress)(cstr.as_ptr() as _) as _
            })
        };

        *self = Self::Initialized(InitializedGraphicsContext {
            egl,
            wayland_egl,
            gl,

            egl_context,
            wl_egl_window,
            egl_surface,

            logical_size,
        });

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let mut event_loop =
        w0::platform::wayland::WaylandEventLoop::new_boxed(WindowConfig::default())?;
    let mut graphics_context = GraphicsContext::Uninitialized;

    'main_loop: loop {
        event_loop.update();
        while let Some(event) = event_loop.pop_event() {
            match event {
                Event::Configure { logical_size } => match graphics_context {
                    GraphicsContext::Uninitialized => graphics_context.init(
                        event_loop.display_handle(),
                        event_loop.window_handle(),
                        logical_size,
                    )?,
                    GraphicsContext::Initialized(ref mut igc) => {
                        igc.resize(logical_size);
                    }
                },
                Event::CloseRequested => break 'main_loop,
            }
        }

        if let GraphicsContext::Initialized(ref igc) = graphics_context {
            unsafe {
                igc.egl_context.make_current(igc.egl_surface)?;

                igc.gl.clear_color(1.0, 0.0, 0.0, 1.0);
                igc.gl.clear(glow::COLOR_BUFFER_BIT);

                igc.egl_context.swap_buffers(igc.egl_surface)?;
            }
        }
    }

    Ok(())
}
