use std::ffi::{c_int, c_void};
use std::ptr::null;
use std::{array, error, fmt, mem, ops};

use crate::libegl as egl;

/// contains code from `eglGetError`.
pub struct RawError(egl::EGLint);

impl error::Error for RawError {}

impl fmt::Display for RawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:#x}", self.0))
    }
}

// NOTE: Debug trait is not derived, but implemented manually because i specifically want to show
// error codes formatted in hex.
impl fmt::Debug for RawError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:#x}", self.0))
    }
}

impl RawError {
    pub fn code(&self) -> egl::EGLint {
        self.0
    }
}

// NOTE: the idea here is that Connection will hand-out handles to resources that it creates that
// need cleanup/deinitialization and you'll operate on those handles; and Connection will be
// responsible for performing cleanup.

// ----
// display

pub enum Display {
    /// eglGetPlatformDisplay
    Khr(egl::EGLDisplay),
    /// eglGetPlatformDisplayEXT
    /// - https://registry.khronos.org/EGL/extensions/EXT/EGL_EXT_platform_base.txt
    Ext(egl::EGLDisplay),
    /// eglGetDisplay
    /// > the set of platforms to which display_id is permitted to belong, as well as the actual
    /// type of display_id, are implementation-specific.
    /// - https://registry.khronos.org/EGL/sdk/docs/man/html/eglGetDisplay.xhtml
    Old(egl::EGLDisplay),
}

impl ops::Deref for Display {
    type Target = egl::EGLDisplay;

    fn deref(&self) -> &Self::Target {
        let (Self::Khr(dpy) | Self::Ext(dpy) | Self::Old(dpy)) = self;
        dpy
    }
}

impl Display {
    fn get_platform_display(
        api: &egl::Api,
        platform: egl::EGLenum,
        native_display: *mut c_void,
        attribs: Option<&[egl::EGLAttrib]>,
    ) -> Option<egl::EGLDisplay> {
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
        if ret == egl::NO_DISPLAY {
            None
        } else {
            Some(ret)
        }
    }

    fn get_platform_display_ext(
        api: &egl::Api,
        platform: egl::EGLenum,
        native_display: *mut c_void,
        attribs: Option<&[egl::EGLint]>,
    ) -> Option<egl::EGLDisplay> {
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
        if ret == egl::NO_DISPLAY {
            None
        } else {
            Some(ret)
        }
    }

    fn get_display(api: &egl::Api, native_display: *mut c_void) -> Option<egl::EGLDisplay> {
        let ret = unsafe { api.GetDisplay(native_display) };
        if ret == egl::NO_DISPLAY {
            None
        } else {
            Some(ret)
        }
    }

    fn from_wayland_display(
        api: &egl::Api,
        wl_display: *mut wayland::wl_display,
        attribs: Option<&[egl::EGLAttrib]>,
    ) -> Option<Self> {
        attribs.inspect(|attribs| assert!(attribs.contains(&(egl::NONE as egl::EGLAttrib))));

        Self::get_platform_display(api, egl::PLATFORM_WAYLAND_KHR, wl_display.cast(), attribs)
            .map(Self::Khr)
            .or_else(|| {
                Self::get_platform_display_ext(
                    api,
                    egl::PLATFORM_WAYLAND_EXT,
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
    CouldNotBindApi(RawError),
    CouldNotCreateContext(RawError),
}

impl error::Error for CreateContextError {}

impl fmt::Display for CreateContextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CouldNotBindApi(re) => f.write_fmt(format_args!("could not bind api: {re}")),
            Self::CouldNotCreateContext(re) => {
                f.write_fmt(format_args!("could not create context: {re}"))
            }
        }
    }
}

pub struct Context {
    index: u8,
    // TODO: don't expose context as is? but instead expose an `as_raw` method?
    pub context: egl::EGLContext,
    // TODO: do i need config here?
    pub config: egl::EGLConfig,
}

// ----
// window surface

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

pub struct WaylandWsi {
    api: wayland::EglApi,
    wl_egl_window: *mut wayland::wl_egl_window,
    width: u32,
    height: u32,
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

        Ok(Self {
            api,
            wl_egl_window,
            width,
            height,
        })
    }

    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        unsafe {
            (self.api.wl_egl_window_resize)(
                self.wl_egl_window,
                width as c_int,
                height as c_int,
                0,
                0,
            )
        };
        self.width = width;
        self.height = height;
    }
}

impl Drop for WaylandWsi {
    fn drop(&mut self) {
        unsafe { (self.api.wl_egl_window_destroy)(self.wl_egl_window) };
    }
}

pub enum Wsi {
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
}

#[derive(Debug)]
pub enum CreateWindowSurfaceError {
    CouldNotCreateWaylandWsi(CreateWaylandWsiError),
    CouldNotCreateSurface(RawError),
}

impl error::Error for CreateWindowSurfaceError {}

impl fmt::Display for CreateWindowSurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CouldNotCreateWaylandWsi(err) => {
                f.write_fmt(format_args!("could not create wayland wsi: {err}"))
            }
            Self::CouldNotCreateSurface(re) => {
                f.write_fmt(format_args!("could not create surface: {re}"))
            }
        }
    }
}

pub struct WindowSurface {
    index: u8,
    pub wsi: Wsi,
    // TODO: would it make sense to make a SurfaceKind { Khr, Ext, Old } enum (same as Display)?
    // TODO: don't expose surface as is? but instead expose an `as_raw` method?
    pub surface: egl::EGLSurface,
    // TODO: do i need config here?
    pub config: egl::EGLConfig,
}

// ----
// connection

#[derive(Debug)]
pub enum CreateConnectionError {
    CouldNotLoadEgl(dynlib::Error),
    CouldNotGetDisplay,
    CouldNotInitializeDisplay(RawError),
}

impl error::Error for CreateConnectionError {}

impl fmt::Display for CreateConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CouldNotLoadEgl(err) => f.write_fmt(format_args!("could not load egl: {err}")),
            Self::CouldNotGetDisplay => f.write_str("could not get display"),
            Self::CouldNotInitializeDisplay(re) => {
                f.write_fmt(format_args!("could not initialize display: {re}"))
            }
        }
    }
}

// TODO: Connection might need to be Arc'ed.
pub struct Connection {
    pub api: egl::Api,
    pub display: Display,

    // NOTE: would you want more then 16? 16 is prob too excessive?
    contexts: [Option<egl::EGLContext>; 16],
    surfaces: [Option<egl::EGLSurface>; 16],
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
        attribs: Option<&[egl::EGLAttrib]>,
    ) -> Result<Self, CreateConnectionError> {
        let api = egl::Api::load().map_err(CreateConnectionError::CouldNotLoadEgl)?;
        let display = Display::from_wayland_display(&api, wl_display, attribs)
            .ok_or(CreateConnectionError::CouldNotGetDisplay)?;
        let this = Self {
            api,
            display,
            contexts: array::from_fn(|_| None),
            surfaces: array::from_fn(|_| None),
        };

        let mut version = (0, 0);
        let ok = unsafe {
            this.api
                .Initialize(*this.display, &mut version.0, &mut version.0)
        };
        if ok == egl::FALSE {
            return Err(CreateConnectionError::CouldNotInitializeDisplay(
                this.unwrap_err(),
            ));
        }

        Ok(this)
    }

    /// NOTE: i don't care how you create your EGLConfig. EGLConfig does not need clean up.
    pub fn create_context(
        &mut self,
        api: egl::EGLenum,
        config: egl::EGLConfig,
        share_context: Option<Context>,
        attribs: Option<&[egl::EGLint]>,
    ) -> Result<Context, CreateContextError> {
        attribs.inspect(|attribs| assert!(attribs.contains(&(egl::NONE as egl::EGLint))));

        let ok = unsafe { self.api.BindAPI(api) };
        if ok == egl::FALSE {
            return Err(CreateContextError::CouldNotBindApi(self.unwrap_err()));
        }

        let context = unsafe {
            self.api.CreateContext(
                *self.display,
                config,
                share_context.map_or(egl::NO_CONTEXT, |c| c.context),
                attribs.map_or(null(), |attribs| attribs.as_ptr()),
            )
        };
        if context == egl::NO_CONTEXT {
            return Err(CreateContextError::CouldNotCreateContext(self.unwrap_err()));
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
    pub fn create_wayland_window_surface(
        &mut self,
        config: egl::EGLConfig,
        wl_surface: *mut wayland::wl_surface,
        width: u32,
        height: u32,
        attribs: Option<&[egl::EGLAttrib]>,
    ) -> Result<WindowSurface, CreateWindowSurfaceError> {
        attribs.inspect(|attribs| assert!(attribs.contains(&(egl::NONE as egl::EGLAttrib))));

        let wsi = Wsi::from_wayland_surface(wl_surface, width, height)
            .map_err(CreateWindowSurfaceError::CouldNotCreateWaylandWsi)?;

        // TODO: make this into a separate function (create_surface?)?
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
                        mem::transmute::<_, &[egl::EGLint]>(attribs).as_ptr()
                    }),
                )
            },
            Display::Old(dpy) => unsafe {
                self.api.CreateWindowSurface(
                    dpy,
                    config,
                    wsi.as_native_window(),
                    attribs.map_or(null(), |attribs| {
                        mem::transmute::<_, &[egl::EGLint]>(attribs).as_ptr()
                    }),
                )
            },
        };
        if surface == egl::NO_SURFACE {
            return Err(CreateWindowSurfaceError::CouldNotCreateSurface(
                self.unwrap_err(),
            ));
        }

        let index = self
            .surfaces
            .iter()
            .position(|it| it.is_none())
            .expect("exhausted surface capacity");
        self.surfaces[index] = Some(surface);
        Ok(WindowSurface {
            index: index as u8,
            wsi,
            surface,
            config,
        })
    }

    /// panics if handle is invalid.
    pub fn destroy_window_surface(&mut self, s: WindowSurface) {
        let surface = self.surfaces[s.index as usize]
            .take()
            .expect("invalid surface handle");
        unsafe { self.api.DestroySurface(*self.display, surface) };
    }

    /// panics if the last function succeeded without error.
    ///
    /// if you're using anyhow - you can provide context by wrapping RawError into Err like so:
    /// `Err(connection.unwrap_err()).context("bla bla ..")`
    pub fn unwrap_err(&self) -> RawError {
        let code = unsafe { self.api.GetError() };
        if code == egl::SUCCESS as egl::EGLint {
            panic!("attempt to unwrap error, but the last function succeeded");
        } else {
            RawError(code)
        }
    }
}
