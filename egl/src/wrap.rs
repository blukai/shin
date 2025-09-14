use std::ffi::{c_int, c_void};
use std::ptr::null;
use std::{array, error, fmt, mem, ops};

use crate::libegl::*;

// NOTE: the idea here is that Connection will hand-out handles to resources that it creates that
// need cleanup/deinitialization and you'll operate on those handles; and Connection will be
// responsible for performing cleanup.

// ----
// display

pub enum Display {
    /// eglGetPlatformDisplay
    Khr(EGLDisplay),
    /// eglGetPlatformDisplayEXT
    /// - https://registry.khronos.org/EGL/extensions/EXT/EGL_EXT_platform_base.txt
    Ext(EGLDisplay),
    /// eglGetDisplay
    /// > the set of platforms to which display_id is permitted to belong, as well as the actual
    /// type of display_id, are implementation-specific.
    /// - https://registry.khronos.org/EGL/sdk/docs/man/html/eglGetDisplay.xhtml
    Old(EGLDisplay),
}

impl ops::Deref for Display {
    type Target = EGLDisplay;

    fn deref(&self) -> &Self::Target {
        let (Self::Khr(dpy) | Self::Ext(dpy) | Self::Old(dpy)) = self;
        dpy
    }
}

impl Display {
    fn get_platform_display(
        api: &Api,
        platform: EGLenum,
        native_display: *mut c_void,
        attribs: Option<&[EGLAttrib]>,
    ) -> Option<EGLDisplay> {
        if api.GetPlatformDisplay.as_ptr().is_null() {
            return None;
        }
        let ret = unsafe {
            api.GetPlatformDisplay(
                platform,
                native_display,
                attribs.map_or(null(), |attribs| attribs.as_ptr()),
            )
        };
        if ret == NO_DISPLAY { None } else { Some(ret) }
    }

    fn get_platform_display_ext(
        api: &Api,
        platform: EGLenum,
        native_display: *mut c_void,
        attribs: Option<&[EGLint]>,
    ) -> Option<EGLDisplay> {
        if api.GetPlatformDisplayEXT.as_ptr().is_null() {
            return None;
        }
        let ret = unsafe {
            api.GetPlatformDisplayEXT(
                platform,
                native_display,
                attribs.map_or(null(), |attribs| attribs.as_ptr()),
            )
        };
        if ret == NO_DISPLAY { None } else { Some(ret) }
    }

    fn get_display(api: &Api, native_display: *mut c_void) -> Option<EGLDisplay> {
        let ret = unsafe { api.GetDisplay(native_display) };
        if ret == NO_DISPLAY { None } else { Some(ret) }
    }

    fn from_wayland_display(
        api: &Api,
        wl_display: *mut wayland::wl_display,
        attribs: Option<&[EGLAttrib]>,
    ) -> Option<Self> {
        attribs.inspect(|attribs| assert!(attribs.contains(&(NONE as EGLAttrib))));

        Self::get_platform_display(api, PLATFORM_WAYLAND_KHR, wl_display.cast(), attribs)
            .map(Self::Khr)
            .or_else(|| {
                Self::get_platform_display_ext(
                    api,
                    PLATFORM_WAYLAND_EXT,
                    wl_display.cast(),
                    attribs.map(|attribs| unsafe { mem::transmute(attribs) }),
                )
                .map(Self::Ext)
            })
            .or_else(|| Self::get_display(api, wl_display.cast()).map(Self::Old))
    }
}

// ----
// context

#[derive(Debug)]
pub enum CreateContextError {
    CouldNotBindApi(EGLint),
    CouldNotCreateContext(EGLint),
}

impl error::Error for CreateContextError {}

impl fmt::Display for CreateContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CouldNotBindApi(code) => {
                f.write_fmt(format_args!("could not bind api: {code:#x}"))
            }
            Self::CouldNotCreateContext(code) => {
                f.write_fmt(format_args!("could not create context: {code:#x}"))
            }
        }
    }
}

pub struct Context {
    index: u8,
    pub context: EGLContext,
    pub config: EGLConfig,
}

// ----
// surface

// NOTE: wsi stands for window system integration; it is somewhat modelled after
// https://registry.khronos.org/vulkan/specs/latest/html/vkspec.html#wsi

#[derive(Debug)]
pub enum CreateWaylandWsiError {
    CouldNotLoadWaylandEgl(dynlib::Error),
    CouldNotCreateWlEglWindow,
}

impl error::Error for CreateWaylandWsiError {}

impl fmt::Display for CreateWaylandWsiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CouldNotLoadWaylandEgl(err) => {
                f.write_fmt(format_args!("could not load wayland-egl: {err}"))
            }
            Self::CouldNotCreateWlEglWindow => {
                f.write_str("could not create wl egl window, make sure params are correct")
            }
        }
    }
}

struct WaylandWsi {
    api: wayland::EglApi,
    wl_egl_window: *mut wayland::wl_egl_window,
}

impl WaylandWsi {
    fn new(
        wl_surface: *mut wayland::wl_surface,
        width: u32,
        height: u32,
    ) -> Result<Self, CreateWaylandWsiError> {
        let api = wayland::EglApi::load().map_err(CreateWaylandWsiError::CouldNotLoadWaylandEgl)?;

        let wl_egl_window = unsafe {
            (api.wl_egl_window_create)(wl_surface.cast(), width as c_int, height as c_int)
        };
        if wl_egl_window.is_null() {
            return Err(CreateWaylandWsiError::CouldNotCreateWlEglWindow);
        }

        Ok(Self { api, wl_egl_window })
    }

    fn resize(&self, width: u32, height: u32) {
        unsafe {
            (self.api.wl_egl_window_resize)(
                self.wl_egl_window,
                width as c_int,
                height as c_int,
                0,
                0,
            )
        };
    }
}

impl Drop for WaylandWsi {
    fn drop(&mut self) {
        unsafe { (self.api.wl_egl_window_destroy)(self.wl_egl_window) };
    }
}

enum Wsi {
    Wayland(WaylandWsi),
}

impl Wsi {
    fn from_wayland_surface(
        wl_surface: *mut wayland::wl_surface,
        width: u32,
        height: u32,
    ) -> Result<Self, CreateWaylandWsiError> {
        WaylandWsi::new(wl_surface, width, height).map(Self::Wayland)
    }

    fn as_native_window(&self) -> *mut c_void {
        match self {
            Self::Wayland(wayland) => wayland.wl_egl_window.cast(),
        }
    }

    fn resize(&self, width: u32, height: u32) {
        match self {
            Self::Wayland(wayland) => wayland.resize(width, height),
        }
    }
}

#[derive(Debug)]
pub enum CreateSurfaceError {
    CouldNotCreateWaylandWsi(CreateWaylandWsiError),
    CouldNotCreateSurface(EGLint),
}

impl error::Error for CreateSurfaceError {}

impl fmt::Display for CreateSurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CouldNotCreateWaylandWsi(err) => {
                f.write_fmt(format_args!("could not create wayland wsi: {err}"))
            }
            Self::CouldNotCreateSurface(code) => {
                f.write_fmt(format_args!("could not create surface: {code:#x}"))
            }
        }
    }
}

pub struct Surface {
    index: u8,
    wsi: Wsi,
    // TODO: would it make sense to make a SurfaceKind { Khr, Ext, Old } enum (same as Display)?
    pub surface: EGLSurface,
    pub config: EGLConfig,
}

impl Surface {
    pub fn resize(&self, width: u32, height: u32) {
        self.wsi.resize(width, height);
    }
}

// ----
// connection

#[derive(Debug)]
pub enum CreateConnectionError {
    CouldNotLoadEgl(dynlib::Error),
    CouldNotGetDisplay,
    CouldNotInitializeDisplay(EGLint),
}

impl error::Error for CreateConnectionError {}

impl fmt::Display for CreateConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CouldNotLoadEgl(err) => f.write_fmt(format_args!("could not load egl: {err}")),
            Self::CouldNotGetDisplay => f.write_str("could not get display"),
            Self::CouldNotInitializeDisplay(code) => {
                f.write_fmt(format_args!("could not initialize display: {code:#x}"))
            }
        }
    }
}

// TODO: Connection might need to be Arc'ed.
pub struct Connection {
    pub api: Api,
    pub display: Display,

    // NOTE: would you want more then 16? 16 is prob too excessive?
    contexts: [Option<EGLContext>; 16],
    surfaces: [Option<EGLSurface>; 16],
}

impl Drop for Connection {
    fn drop(&mut self) {
        for maybe_surface in self.surfaces.iter_mut() {
            if let Some(surface) = maybe_surface.take() {
                unsafe { self.api.DestroySurface(*self.display, surface) };
            }
        }

        for maybe_context in self.contexts.iter_mut() {
            if let Some(context) = maybe_context.take() {
                unsafe { self.api.DestroyContext(*self.display, context) };
            }
        }

        unsafe { self.api.Terminate(*self.display) };
    }
}

impl Connection {
    pub fn from_wayland_display(
        wl_display: *mut wayland::wl_display,
        attribs: Option<&[EGLAttrib]>,
    ) -> Result<Self, CreateConnectionError> {
        let api = Api::load().map_err(CreateConnectionError::CouldNotLoadEgl)?;

        let display = Display::from_wayland_display(&api, wl_display, attribs)
            .ok_or(CreateConnectionError::CouldNotGetDisplay)?;

        let mut version = (0, 0);
        if unsafe { api.Initialize(*display, &mut version.0, &mut version.0) } == FALSE {
            let code = unsafe { api.GetError() };
            return Err(CreateConnectionError::CouldNotInitializeDisplay(code));
        }

        Ok(Self {
            api,
            display,
            contexts: array::from_fn(|_| None),
            surfaces: array::from_fn(|_| None),
        })
    }

    /// NOTE: i don't care how you create your EGLConfig. EGLConfig does not need clean up.
    pub fn create_context(
        &mut self,
        api: EGLenum,
        config: EGLConfig,
        share_context: Option<Context>,
        attribs: Option<&[EGLint]>,
    ) -> Result<Context, CreateContextError> {
        attribs.inspect(|attribs| assert!(attribs.contains(&(NONE as EGLint))));

        if unsafe { self.api.BindAPI(api) } == FALSE {
            let code = unsafe { self.api.GetError() };
            return Err(CreateContextError::CouldNotBindApi(code));
        }

        let context = unsafe {
            self.api.CreateContext(
                *self.display,
                config,
                share_context.map_or(NO_CONTEXT, |c| c.context),
                attribs.map_or(null(), |attribs| attribs.as_ptr()),
            )
        };
        if context == NO_CONTEXT {
            let code = unsafe { self.api.GetError() };
            return Err(CreateContextError::CouldNotCreateContext(code));
        }

        let index = self
            .contexts
            .iter()
            .position(|it| it.is_none())
            .expect("exhausted context capacity");
        self.contexts[index] = Some(context);
        Ok(Context {
            index: index as u8,
            context,
            config,
        })
    }

    /// panics if handle is invalid.
    pub fn destroy_context(&mut self, c: Context) {
        let context = self.contexts[c.index as usize]
            .take()
            .expect("invalid context handle");
        unsafe { self.api.DestroyContext(*self.display, context) };
    }

    /// NOTE: i don't care how you create your EGLConfig. EGLConfig does not need clean up.
    pub fn create_wayland_surface(
        &mut self,
        config: EGLConfig,
        wl_surface: *mut wayland::wl_surface,
        width: u32,
        height: u32,
        attribs: Option<&[EGLAttrib]>,
    ) -> Result<Surface, CreateSurfaceError> {
        attribs.inspect(|attribs| assert!(attribs.contains(&(NONE as EGLAttrib))));

        let wsi = Wsi::from_wayland_surface(wl_surface, width, height)
            .map_err(CreateSurfaceError::CouldNotCreateWaylandWsi)?;

        let surface = match self.display {
            Display::Khr(dpy) => unsafe {
                self.api.CreatePlatformWindowSurface(
                    dpy,
                    config,
                    wsi.as_native_window(),
                    attribs.map_or(null(), |attribs| attribs.as_ptr()),
                )
            },
            Display::Ext(dpy) => unsafe {
                self.api.CreatePlatformWindowSurfaceEXT(
                    dpy,
                    config,
                    wsi.as_native_window(),
                    attribs.map_or(null(), |attribs| {
                        mem::transmute::<_, &[EGLint]>(attribs).as_ptr()
                    }),
                )
            },
            Display::Old(dpy) => unsafe {
                self.api.CreateWindowSurface(
                    dpy,
                    config,
                    wsi.as_native_window(),
                    attribs.map_or(null(), |attribs| {
                        mem::transmute::<_, &[EGLint]>(attribs).as_ptr()
                    }),
                )
            },
        };
        if surface == NO_SURFACE {
            let code = unsafe { self.api.GetError() };
            return Err(CreateSurfaceError::CouldNotCreateSurface(code));
        }

        let index = self
            .surfaces
            .iter()
            .position(|it| it.is_none())
            .expect("exhausted surface capacity");
        self.surfaces[index] = Some(surface);
        Ok(Surface {
            index: index as u8,
            wsi,
            surface,
            config,
        })
    }

    /// panics if handle is invalid.
    pub fn destroy_surface(&mut self, s: Surface) {
        let surface = self.surfaces[s.index as usize]
            .take()
            .expect("invalid surface handle");
        unsafe { self.api.DestroySurface(*self.display, surface) };
    }
}
