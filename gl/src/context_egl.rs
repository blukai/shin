use std::ffi::{c_char, c_int, c_void};
use std::ptr::{null, null_mut};

use anyhow::{Context as _, anyhow};
use raw_window_handle as rwh;

use crate::egl::*;

pub fn egl_get_error(egl_lib: &EglApi) -> anyhow::Error {
    match unsafe { egl_lib.GetError() } as EGLenum {
        SUCCESS => unreachable!(),
        code => anyhow!(format!("egl error 0x{:x}", code)),
    }
}

#[derive(Default)]
pub struct Config {
    pub min_swap_interval: Option<u16>,
    pub max_swap_interval: Option<u16>,
}

pub struct Context {
    config: EGLConfig,
    context: EGLContext,
    display: EGLDisplay,

    libegl: EglApi,
}

impl Context {
    pub fn new(display_handle: rwh::DisplayHandle, config: Config) -> anyhow::Result<Self> {
        let libegl = EglApi::load()?;

        unsafe {
            // TODO: make api configurable
            if libegl.BindAPI(OPENGL_API) == FALSE {
                return Err(egl_get_error(&libegl)).context("could not bind api");
            }

            let display_handle_ptr = match display_handle.as_raw() {
                rwh::RawDisplayHandle::Wayland(payload) => payload.display.as_ptr(),
                _ => {
                    return Err(anyhow!(format!(
                        "unsupported window system (display handle: {display_handle:?})"
                    )));
                }
            };
            let display = libegl.GetDisplay(display_handle_ptr);
            if display == NO_DISPLAY {
                return Err(egl_get_error(&libegl)).context("could not get display");
            }

            let (mut major, mut minor) = (0, 0);
            if libegl.Initialize(display, &mut major, &mut minor) == FALSE {
                return Err(egl_get_error(&libegl)).context("could not initialize");
            }
            log::info!("initialized egl version {major}.{minor}");

            // 64 seems enough?
            let mut config_attrs = [NONE as EGLint; 64];
            let mut num_config_attrs = 0;
            let mut push_config_attr = |attr: EGLenum, value: EGLint| {
                config_attrs[num_config_attrs] = attr as EGLint;
                num_config_attrs += 1;
                config_attrs[num_config_attrs] = value;
                num_config_attrs += 1;
            };
            push_config_attr(RED_SIZE, 8);
            push_config_attr(GREEN_SIZE, 8);
            push_config_attr(BLUE_SIZE, 8);
            // NOTE: it is important to set EGL_ALPHA_SIZE, it enables transparency
            push_config_attr(ALPHA_SIZE, 8);
            push_config_attr(CONFORMANT, OPENGL_ES3_BIT);
            push_config_attr(RENDERABLE_TYPE, OPENGL_ES3_BIT);
            // NOTE: EGL_SAMPLE_BUFFERS + EGL_SAMPLES enable some kind of don't care anti aliasing
            push_config_attr(SAMPLE_BUFFERS, 1);
            push_config_attr(SAMPLES, 4);
            if let Some(min_swap_interval) = config.min_swap_interval {
                push_config_attr(MIN_SWAP_INTERVAL, min_swap_interval as _);
            }
            if let Some(max_swap_interval) = config.max_swap_interval {
                push_config_attr(MAX_SWAP_INTERVAL, max_swap_interval as _);
            }

            let mut num_configs = 0;
            if libegl.GetConfigs(display, null_mut(), 0, &mut num_configs) == FALSE {
                return Err(egl_get_error(&libegl))
                    .context("could not get num of available configs");
            }
            let mut configs = vec![std::mem::zeroed(); num_configs as usize];
            if libegl.ChooseConfig(
                display,
                config_attrs.as_ptr() as _,
                configs.as_mut_ptr(),
                num_configs,
                &mut num_configs,
            ) == FALSE
            {
                return Err(egl_get_error(&libegl)).context("could not choose config");
            }
            configs.set_len(num_configs as usize);
            if configs.is_empty() {
                return Err(anyhow!("could not choose config (/ no compatible ones)"));
            }
            let config = *configs.first().unwrap();

            let context_attrs = &[CONTEXT_MAJOR_VERSION, 3, NONE];
            let context =
                libegl.CreateContext(display, config, NO_CONTEXT, context_attrs.as_ptr() as _);
            if context == NO_CONTEXT {
                return Err(egl_get_error(&libegl)).context("could not create context");
            }

            Ok(Context {
                display,
                config,
                context,

                libegl,
            })
        }
    }

    // TODO: get rid of this.
    pub fn get_proc_address(
        &self,
        procname: *const c_char,
    ) -> __eglMustCastToProperFunctionPointerType {
        unsafe { self.libegl.GetProcAddress(procname) }
    }

    pub fn make_current(&self, surface: EGLSurface) -> anyhow::Result<()> {
        unsafe {
            if self
                .libegl
                .MakeCurrent(self.display, surface, surface, self.context)
                == FALSE
            {
                Err(egl_get_error(&self.libegl)).context("could not make current")
            } else {
                Ok(())
            }
        }
    }

    pub fn make_current_surfaceless(&self) -> anyhow::Result<()> {
        self.make_current(NO_SURFACE)
    }

    pub fn set_swap_interval(&self, interval: EGLint) -> anyhow::Result<()> {
        unsafe {
            if self.libegl.SwapInterval(self.display, interval) == FALSE {
                Err(egl_get_error(&self.libegl)).context("could not set swap interval")
            } else {
                Ok(())
            }
        }
    }

    pub fn swap_buffers(&self, surface: EGLSurface) -> anyhow::Result<()> {
        unsafe {
            if self.libegl.SwapBuffers(self.display, surface) == FALSE {
                Err(egl_get_error(&self.libegl)).context("could not swap buffers")
            } else {
                Ok(())
            }
        }
    }
}

// NOTE: wsi stands for window system integration; it is somewhat modelled after
// https://registry.khronos.org/vulkan/specs/latest/html/vkspec.html#wsi

struct WaylandWsi {
    libwayland_egl: wayland::EglApi,
    wl_egl_window: *mut wayland::wl_egl_window,
}

impl WaylandWsi {
    fn new(wayland_wh: rwh::WaylandWindowHandle, width: u32, height: u32) -> anyhow::Result<Self> {
        let libwayland_egl = wayland::EglApi::load()?;
        let wl_egl_window = unsafe {
            (libwayland_egl.wl_egl_window_create)(
                wayland_wh.surface.as_ptr(),
                width as c_int,
                height as c_int,
            )
        };
        if wl_egl_window.is_null() {
            return Err(anyhow!("could not create wl egl window"));
        }
        Ok(Self {
            libwayland_egl,
            wl_egl_window,
        })
    }
}

enum Wsi {
    Wayland(WaylandWsi),
}

impl Wsi {
    fn new(window_handle: rwh::WindowHandle, width: u32, height: u32) -> anyhow::Result<Self> {
        match window_handle.as_raw() {
            rwh::RawWindowHandle::Wayland(wayland_wh) => {
                WaylandWsi::new(wayland_wh, width, height).map(Self::Wayland)
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

pub struct Surface {
    wsi: Wsi,
    surface: EGLSurface,
}

impl Surface {
    pub fn new(
        context: &Context,
        window_handle: rwh::WindowHandle,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Self> {
        assert!(width > 0);
        assert!(height > 0);

        let wsi = Wsi::new(window_handle, width, height)?;
        let surface = unsafe {
            context.libegl.CreateWindowSurface(
                context.display,
                context.config,
                wsi.as_ptr() as EGLNativeWindowType,
                null(),
            )
        };
        if surface.is_null() {
            return Err(anyhow!("could not create egl surface"));
        }

        Ok(Self { wsi, surface })
    }

    pub fn as_ptr(&self) -> *mut c_void {
        self.surface
    }

    pub fn resize(&self, width: u32, height: u32) -> anyhow::Result<()> {
        match self.wsi {
            Wsi::Wayland(ref payload) => unsafe {
                (payload.libwayland_egl.wl_egl_window_resize)(
                    payload.wl_egl_window,
                    width as i32,
                    height as i32,
                    0,
                    0,
                );
            },
        }
        Ok(())
    }
}
