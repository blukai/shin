use std::ffi::c_void;
use std::ptr::{null, null_mut};

use anyhow::{anyhow, Context as _};
use raw_window_handle as rwh;
use window::{libwayland_client, Size};

use crate::{libegl, libwayland_egl};

pub fn egl_get_error(egl: &libegl::Lib) -> anyhow::Error {
    match unsafe { (egl.eglGetError)() } as libegl::EGLenum {
        libegl::EGL_SUCCESS => unreachable!(),
        code => anyhow!(format!("egl error 0x{:x}", code)),
    }
}

pub struct EglContext {
    config: libegl::EGLConfig,
    context: libegl::EGLContext,
    display: libegl::EGLDisplay,
}

impl EglContext {
    pub fn new(egl: &libegl::Lib, display_handle: rwh::DisplayHandle) -> anyhow::Result<Self> {
        unsafe {
            // TODO: make api configurable
            if (egl.eglBindAPI)(libegl::EGL_OPENGL_API) == libegl::EGL_FALSE {
                return Err(egl_get_error(egl)).context("could not bind api");
            }

            let display_handle_ptr = match display_handle.as_raw() {
                rwh::RawDisplayHandle::Wayland(payload) => payload.display.as_ptr(),
                _ => {
                    return Err(anyhow!(format!(
                        "unsupported window system (display handle: {display_handle:?})"
                    )));
                }
            };
            let display = (egl.eglGetDisplay)(display_handle_ptr);
            if display == libegl::EGL_NO_DISPLAY {
                return Err(egl_get_error(egl)).context("could not get display");
            }

            let (mut major, mut minor) = (0, 0);
            if (egl.eglInitialize)(display, &mut major, &mut minor) == libegl::EGL_FALSE {
                return Err(egl_get_error(egl)).context("could not initialize");
            }
            log::info!("initialized egl version {major}.{minor}");

            // TODO: make config attrs configurable
            let config_attrs = &[
                libegl::EGL_RED_SIZE,
                8,
                libegl::EGL_GREEN_SIZE,
                8,
                libegl::EGL_BLUE_SIZE,
                8,
                // NOTE: it is important to set EGL_ALPHA_SIZE, it enables transparency
                libegl::EGL_ALPHA_SIZE,
                8,
                libegl::EGL_CONFORMANT,
                libegl::EGL_OPENGL_ES3_BIT,
                libegl::EGL_RENDERABLE_TYPE,
                libegl::EGL_OPENGL_ES3_BIT,
                // NOTE: EGL_SAMPLE_BUFFERS + EGL_SAMPLES enables some kind of don't care anti aliasing
                libegl::EGL_SAMPLE_BUFFERS,
                1,
                libegl::EGL_SAMPLES,
                4,
                libegl::EGL_NONE,
            ];
            let mut num_configs = 0;
            if (egl.eglGetConfigs)(display, null_mut(), 0, &mut num_configs) == libegl::EGL_FALSE {
                return Err(egl_get_error(egl)).context("could not get num of available configs");
            }
            let mut configs = vec![std::mem::zeroed(); num_configs as usize];
            if (egl.eglChooseConfig)(
                display,
                config_attrs.as_ptr() as _,
                configs.as_mut_ptr(),
                num_configs,
                &mut num_configs,
            ) == libegl::EGL_FALSE
            {
                return Err(egl_get_error(egl)).context("could not choose config");
            }
            configs.set_len(num_configs as usize);
            if configs.is_empty() {
                return Err(anyhow!("could not choose config (/ no compatible ones)"));
            }
            let config = *configs.first().unwrap();

            let context_attrs = &[libegl::EGL_CONTEXT_MAJOR_VERSION, 3, libegl::EGL_NONE];
            let context = (egl.eglCreateContext)(
                display,
                config,
                libegl::EGL_NO_CONTEXT,
                context_attrs.as_ptr() as _,
            );
            if context == libegl::EGL_NO_CONTEXT {
                return Err(egl_get_error(egl)).context("could not create context");
            }

            Ok(EglContext {
                display,
                config,
                context,
            })
        }
    }

    pub fn make_current(
        &self,
        egl: &libegl::Lib,
        surface: libegl::EGLSurface,
    ) -> anyhow::Result<()> {
        unsafe {
            if (egl.eglMakeCurrent)(self.display, surface, surface, self.context)
                == libegl::EGL_FALSE
            {
                Err(egl_get_error(&egl)).context("could not make current")
            } else {
                Ok(())
            }
        }
    }

    pub fn make_current_surfaceless(&self, egl: &libegl::Lib) -> anyhow::Result<()> {
        self.make_current(egl, libegl::EGL_NO_SURFACE)
    }

    pub fn swap_buffers(
        &self,
        egl: &libegl::Lib,
        surface: libegl::EGLSurface,
    ) -> anyhow::Result<()> {
        unsafe {
            if (egl.eglSwapBuffers)(self.display, surface) == libegl::EGL_FALSE {
                Err(egl_get_error(egl)).context("could not swap buffers")
            } else {
                Ok(())
            }
        }
    }
}

// NOTE: wsi stands for window system integration; it is somewhat modelled after
// https://registry.khronos.org/vulkan/specs/latest/html/vkspec.html#wsi

struct EglWaylandWsi {
    wayland_egl: libwayland_egl::Lib,
    wl_egl_window: *mut libwayland_egl::wl_egl_window,
}

impl EglWaylandWsi {
    fn new(wayland_wh: rwh::WaylandWindowHandle) -> anyhow::Result<Self> {
        let wayland_egl = libwayland_egl::Lib::load()?;
        let wl_egl_window = unsafe {
            (wayland_egl.wl_egl_window_create)(
                wayland_wh.surface.as_ptr() as *mut libwayland_client::wl_surface,
                640,
                480,
            )
        };
        if wl_egl_window.is_null() {
            return Err(anyhow!("could not create wl egl window"));
        }
        Ok(Self {
            wayland_egl,
            wl_egl_window,
        })
    }
}

enum EglWsi {
    Wayland(EglWaylandWsi),
}

impl EglWsi {
    fn new(window_handle: rwh::WindowHandle) -> anyhow::Result<Self> {
        match window_handle.as_raw() {
            rwh::RawWindowHandle::Wayland(wayland_wh) => {
                EglWaylandWsi::new(wayland_wh).map(Self::Wayland)
            }
            _ => {
                return Err(anyhow!(format!(
                    "unsupported window system (window handle: {window_handle:?})"
                )));
            }
        }
    }

    fn as_ptr(&self) -> *mut c_void {
        match self {
            Self::Wayland(payload) => payload.wl_egl_window as *mut c_void,
        }
    }
}

pub struct EglSurface {
    wsi: EglWsi,
    surface: libegl::EGLSurface,
}

impl EglSurface {
    pub fn new(
        egl: &libegl::Lib,
        egl_context: &EglContext,
        window_handle: rwh::WindowHandle,
        logical_size: Size,
    ) -> anyhow::Result<Self> {
        let egl_wsi = EglWsi::new(window_handle)?;
        let egl_surface = unsafe {
            (egl.eglCreateWindowSurface)(
                egl_context.display,
                egl_context.config,
                egl_wsi.as_ptr() as libegl::EGLNativeWindowType,
                null(),
            )
        };
        if egl_surface.is_null() {
            return Err(anyhow!("could not create egl surface"));
        }

        egl_context.make_current_surfaceless(egl)?;

        match egl_wsi {
            EglWsi::Wayland(ref payload) => unsafe {
                (payload.wayland_egl.wl_egl_window_resize)(
                    payload.wl_egl_window,
                    logical_size.width as i32,
                    logical_size.height as i32,
                    0,
                    0,
                );
            },
        };

        Ok(Self {
            wsi: egl_wsi,
            surface: egl_surface,
        })
    }

    pub fn resize(&self, logical_size: Size) {
        match self.wsi {
            EglWsi::Wayland(ref payload) => unsafe {
                (payload.wayland_egl.wl_egl_window_resize)(
                    payload.wl_egl_window,
                    logical_size.width as i32,
                    logical_size.height as i32,
                    0,
                    0,
                );
            },
        }
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.surface
    }
}
