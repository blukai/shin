use std::collections::VecDeque;
use std::ffi::{c_char, c_void, CStr};
use std::ptr::{null_mut, NonNull};

use anyhow::{anyhow, Context as _};
use raw_window_handle as rwh;

use crate::{
    libwayland_client, libxkbcommon, Event, EventLoop, Size, WindowConfig, DEFAULT_LOGICAL_SIZE,
};

struct WaylandConnection {
    libwayland_client: libwayland_client::Lib,
    libxkbcommon: libxkbcommon::Lib,

    wl_display: NonNull<libwayland_client::wl_display>,

    wl_compositor: *mut libwayland_client::wl_compositor,
    wl_seat: *mut libwayland_client::wl_seat,
    wp_viewporter: *mut libwayland_client::wp_viewporter,
    xdg_wm_base: *mut libwayland_client::xdg_wm_base,
}

struct WaylandWindow {
    config: WindowConfig,

    wl_surface: *mut libwayland_client::wl_surface,
    xdg_surface: *mut libwayland_client::xdg_surface,
    xdg_toplevel: *mut libwayland_client::xdg_toplevel,
}

pub struct WaylandEventLoop {
    conn: WaylandConnection,
    window: WaylandWindow,
    events: VecDeque<Event>,
}

unsafe extern "C" fn handle_wl_registry_global(
    data: *mut c_void,
    wl_registry: *mut libwayland_client::wl_registry,
    name: u32,
    interface: *const c_char,
    version: u32,
) {
    let evl = &mut *(data as *mut WaylandEventLoop);

    let interface = CStr::from_ptr(interface)
        .to_str()
        .expect("invalid interface string");

    match interface {
        "wl_compositor" => {
            evl.conn.wl_compositor = libwayland_client::wl_registry_bind(
                &evl.conn.libwayland_client,
                wl_registry,
                name,
                &libwayland_client::wl_compositor_interface,
                6.min(version),
            ) as _;
        }
        "wl_seat" => {
            evl.conn.wl_seat = libwayland_client::wl_registry_bind(
                &evl.conn.libwayland_client,
                wl_registry,
                name,
                &libwayland_client::wl_seat_interface,
                9.min(version),
            ) as _;
        }
        "wp_viewporter" => {
            evl.conn.wp_viewporter = libwayland_client::wl_registry_bind(
                &evl.conn.libwayland_client,
                wl_registry,
                name,
                &libwayland_client::wp_viewporter_interface,
                1.min(version),
            ) as _;
        }
        "xdg_wm_base" => {
            evl.conn.xdg_wm_base = libwayland_client::wl_registry_bind(
                &evl.conn.libwayland_client,
                wl_registry,
                name,
                &libwayland_client::xdg_wm_base_interface,
                6.min(version),
            ) as _;
        }
        _ => {
            log::debug!("unused interface: {interface}");
        }
    }
}

const WL_REGISTRY_LISTENER: libwayland_client::wl_registry_listener =
    libwayland_client::wl_registry_listener {
        global: handle_wl_registry_global,
        global_remove: libwayland_client::noop_listener!(),
    };

unsafe extern "C" fn handle_xdg_wm_base_ping(
    data: *mut c_void,
    xdg_wm_base: *mut libwayland_client::xdg_wm_base,
    serial: u32,
) {
    let evl = &mut *(data as *mut WaylandEventLoop);
    libwayland_client::xdg_wm_base_pong(&evl.conn.libwayland_client, xdg_wm_base, serial);
}

const XDG_WM_BASE_LISTENER: libwayland_client::xdg_wm_base_listener =
    libwayland_client::xdg_wm_base_listener {
        ping: handle_xdg_wm_base_ping,
    };

unsafe extern "C" fn handle_xdg_surface_configure(
    data: *mut c_void,
    xdg_surface: *mut libwayland_client::xdg_surface,
    serial: u32,
) {
    log::debug!("recv xdg_surface_configure");

    let evl = &mut *(data as *mut WaylandEventLoop);
    libwayland_client::xdg_surface_ack_configure(&evl.conn.libwayland_client, xdg_surface, serial);
}

const XDG_SURFACE_LISTENER: libwayland_client::xdg_surface_listener =
    libwayland_client::xdg_surface_listener {
        configure: handle_xdg_surface_configure,
    };

unsafe extern "C" fn handle_xdg_toplevel_configure(
    data: *mut c_void,
    _xdg_toplevel: *mut libwayland_client::xdg_toplevel,
    width: i32,
    height: i32,
    _states: *mut libwayland_client::wl_array,
) {
    log::debug!("recv xdg_toplevel_configure");

    let evl = &mut *(data as *mut WaylandEventLoop);

    assert!(width >= 0 && height >= 0);
    let logical_size = if width > 0 || height > 0 {
        Some(Size::new(width as u32, height as u32))
    } else {
        evl.window.config.logical_size
    }
    .unwrap_or(DEFAULT_LOGICAL_SIZE);
    log::debug!("logical_size: {logical_size:?}");

    let event = Event::Configure { logical_size };
    evl.events.push_back(event);
}

unsafe extern "C" fn handle_xdg_toplevel_close(
    data: *mut c_void,
    _xdg_toplevel: *mut libwayland_client::xdg_toplevel,
) {
    log::debug!("recv xdg_toplevel_close");

    let evl = &mut *(data as *mut WaylandEventLoop);

    let event = Event::CloseRequested;
    evl.events.push_back(event);
}

const XDG_TOPLEVEL_LISTENER: libwayland_client::xdg_toplevel_listener =
    libwayland_client::xdg_toplevel_listener {
        configure: handle_xdg_toplevel_configure,
        close: handle_xdg_toplevel_close,
        wm_capabilities: libwayland_client::noop_listener!(),
        configure_bounds: libwayland_client::noop_listener!(),
    };

impl WaylandEventLoop {
    pub fn new_boxed(config: WindowConfig) -> anyhow::Result<Box<Self>> {
        let libwayland_client = libwayland_client::Lib::load()?;
        let libxkbcommon = libxkbcommon::Lib::load()?;

        let wl_display =
            NonNull::new(unsafe { (libwayland_client.wl_display_connect)(null_mut()) })
                .context("could not connect to wayland display")?;

        let mut boxed = Box::new(Self {
            conn: WaylandConnection {
                libwayland_client,
                libxkbcommon,

                wl_display,

                wl_compositor: null_mut(),
                wl_seat: null_mut(),
                wp_viewporter: null_mut(),
                xdg_wm_base: null_mut(),
            },
            window: WaylandWindow {
                config,

                wl_surface: null_mut(),
                xdg_surface: null_mut(),
                xdg_toplevel: null_mut(),
            },
            events: VecDeque::new(),
        });

        // init globals

        let wl_registry: *mut libwayland_client::wl_registry = unsafe {
            libwayland_client::wl_display_get_registry(
                &boxed.conn.libwayland_client,
                boxed.conn.wl_display.as_ptr(),
            )
        };
        if wl_registry.is_null() {
            return Err(anyhow!("could not get registry"));
        }
        unsafe {
            (boxed.conn.libwayland_client.wl_proxy_add_listener)(
                wl_registry as *mut libwayland_client::wl_proxy,
                &WL_REGISTRY_LISTENER as *const libwayland_client::wl_registry_listener as _,
                boxed.as_mut() as *mut WaylandEventLoop as *mut c_void,
            );
            (boxed.conn.libwayland_client.wl_display_roundtrip)(boxed.conn.wl_display.as_ptr());
        }
        assert!(!boxed.conn.wl_compositor.is_null());
        assert!(!boxed.conn.wl_seat.is_null());
        assert!(!boxed.conn.xdg_wm_base.is_null());

        unsafe {
            (boxed.conn.libwayland_client.wl_proxy_add_listener)(
                boxed.conn.xdg_wm_base as *mut libwayland_client::wl_proxy,
                &XDG_WM_BASE_LISTENER as *const libwayland_client::xdg_wm_base_listener as _,
                boxed.as_mut() as *mut WaylandEventLoop as *mut c_void,
            );
        }

        log::info!("initialized globals");

        // init window

        boxed.window.wl_surface = unsafe {
            libwayland_client::wl_compositor_create_surface(
                &boxed.conn.libwayland_client,
                boxed.conn.wl_compositor,
            )
        };
        if boxed.window.wl_surface.is_null() {
            return Err(anyhow!("could not create wl surface"));
        }

        boxed.window.xdg_surface = unsafe {
            libwayland_client::xdg_wm_base_get_xdg_surface(
                &boxed.conn.libwayland_client,
                boxed.conn.xdg_wm_base,
                boxed.window.wl_surface,
            )
        };
        if boxed.window.xdg_surface.is_null() {
            return Err(anyhow!("could not create xdg surface"));
        }

        boxed.window.xdg_toplevel = unsafe {
            libwayland_client::xdg_surface_get_toplevel(
                &boxed.conn.libwayland_client,
                boxed.window.xdg_surface,
            )
        };
        if boxed.window.xdg_toplevel.is_null() {
            return Err(anyhow!("could not get xdg toplevel"));
        }

        unsafe {
            (boxed.conn.libwayland_client.wl_proxy_add_listener)(
                boxed.window.xdg_surface as *mut libwayland_client::wl_proxy,
                &XDG_SURFACE_LISTENER as *const libwayland_client::xdg_surface_listener as _,
                boxed.as_mut() as *mut WaylandEventLoop as *mut c_void,
            );
            (boxed.conn.libwayland_client.wl_proxy_add_listener)(
                boxed.window.xdg_toplevel as *mut libwayland_client::wl_proxy,
                &XDG_TOPLEVEL_LISTENER as *const libwayland_client::xdg_toplevel_listener as _,
                boxed.as_mut() as *mut WaylandEventLoop as *mut c_void,
            );
            libwayland_client::wl_surface_commit(
                &boxed.conn.libwayland_client,
                boxed.window.wl_surface,
            );
            (boxed.conn.libwayland_client.wl_display_roundtrip)(boxed.conn.wl_display.as_ptr());
        }

        log::info!("initialized window");

        Ok(boxed)
    }
}

impl rwh::HasDisplayHandle for WaylandEventLoop {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let wayland_display_handle = rwh::WaylandDisplayHandle::new(self.conn.wl_display.cast());
        let raw_display_handle = rwh::RawDisplayHandle::Wayland(wayland_display_handle);
        let display_handle = unsafe { rwh::DisplayHandle::borrow_raw(raw_display_handle) };
        Ok(display_handle)
    }
}

impl rwh::HasWindowHandle for WaylandEventLoop {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let Some(wl_surface) = NonNull::new(self.window.wl_surface) else {
            return Err(rwh::HandleError::Unavailable);
        };
        let wayland_window_handle = rwh::WaylandWindowHandle::new(wl_surface.cast());
        let raw_window_handle = rwh::RawWindowHandle::Wayland(wayland_window_handle);
        let window_handle = unsafe { rwh::WindowHandle::borrow_raw(raw_window_handle) };
        Ok(window_handle)
    }
}

impl EventLoop for WaylandEventLoop {
    fn update(&mut self) -> anyhow::Result<()> {
        let n = unsafe {
            (self.conn.libwayland_client.wl_display_dispatch_pending)(self.conn.wl_display.as_ptr())
        };
        if n == -1 {
            return Err(anyhow!("wl_display_dispatch_pending failed"));
        }
        Ok(())
    }

    fn pop_event(&mut self) -> Option<Event> {
        self.events.pop_back()
    }
}
