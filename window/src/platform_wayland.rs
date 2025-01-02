use std::collections::VecDeque;
use std::ffi::{CStr, c_char, c_void};
use std::ptr::{NonNull, null_mut};

use anyhow::{Context as _, anyhow};
use raw_window_handle as rwh;

use crate::{
    DEFAULT_LOGICAL_SIZE, Event, Size, Window, WindowConfig, libwayland_client, libxkbcommon,
};

pub struct WaylandWindow {
    config: WindowConfig,

    libwayland_client: libwayland_client::Lib,
    libxkbcommon: libxkbcommon::Lib,

    wl_display: NonNull<libwayland_client::wl_display>,

    wl_compositor: *mut libwayland_client::wl_compositor,
    wl_seat: *mut libwayland_client::wl_seat,
    wp_viewporter: *mut libwayland_client::wp_viewporter,
    xdg_wm_base: *mut libwayland_client::xdg_wm_base,

    wl_surface: *mut libwayland_client::wl_surface,
    xdg_surface: *mut libwayland_client::xdg_surface,
    xdg_toplevel: *mut libwayland_client::xdg_toplevel,

    events: VecDeque<Event>,
}

unsafe extern "C" fn handle_wl_registry_global(
    data: *mut c_void,
    wl_registry: *mut libwayland_client::wl_registry,
    name: u32,
    interface: *const c_char,
    version: u32,
) {
    let evl = &mut *(data as *mut WaylandWindow);

    let interface = CStr::from_ptr(interface)
        .to_str()
        .expect("invalid interface string");

    match interface {
        "wl_compositor" => {
            evl.wl_compositor = libwayland_client::wl_registry_bind(
                &evl.libwayland_client,
                wl_registry,
                name,
                &libwayland_client::wl_compositor_interface,
                6.min(version),
            ) as _;
        }
        "wl_seat" => {
            evl.wl_seat = libwayland_client::wl_registry_bind(
                &evl.libwayland_client,
                wl_registry,
                name,
                &libwayland_client::wl_seat_interface,
                9.min(version),
            ) as _;
        }
        "wp_viewporter" => {
            evl.wp_viewporter = libwayland_client::wl_registry_bind(
                &evl.libwayland_client,
                wl_registry,
                name,
                &libwayland_client::wp_viewporter_interface,
                1.min(version),
            ) as _;
        }
        "xdg_wm_base" => {
            evl.xdg_wm_base = libwayland_client::wl_registry_bind(
                &evl.libwayland_client,
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
    let evl = &mut *(data as *mut WaylandWindow);
    libwayland_client::xdg_wm_base_pong(&evl.libwayland_client, xdg_wm_base, serial);
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

    let evl = &mut *(data as *mut WaylandWindow);
    libwayland_client::xdg_surface_ack_configure(&evl.libwayland_client, xdg_surface, serial);
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

    let evl = &mut *(data as *mut WaylandWindow);

    assert!(width >= 0 && height >= 0);
    let logical_size = if width > 0 || height > 0 {
        Some(Size::new(width as u32, height as u32))
    } else {
        evl.config.logical_size
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

    let evl = &mut *(data as *mut WaylandWindow);

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

impl WaylandWindow {
    pub fn new_boxed(config: WindowConfig) -> anyhow::Result<Box<Self>> {
        let libwayland_client = libwayland_client::Lib::load()?;
        let libxkbcommon = libxkbcommon::Lib::load()?;

        let wl_display =
            NonNull::new(unsafe { (libwayland_client.wl_display_connect)(null_mut()) })
                .context("could not connect to wayland display")?;

        let mut boxed = Box::new(WaylandWindow {
            config,

            libwayland_client,
            libxkbcommon,

            wl_display,

            wl_compositor: null_mut(),
            wl_seat: null_mut(),
            wp_viewporter: null_mut(),
            xdg_wm_base: null_mut(),

            wl_surface: null_mut(),
            xdg_surface: null_mut(),
            xdg_toplevel: null_mut(),

            events: VecDeque::new(),
        });

        // init globals

        let wl_registry: *mut libwayland_client::wl_registry = unsafe {
            libwayland_client::wl_display_get_registry(
                &boxed.libwayland_client,
                boxed.wl_display.as_ptr(),
            )
        };
        if wl_registry.is_null() {
            return Err(anyhow!("could not get registry"));
        }
        unsafe {
            (boxed.libwayland_client.wl_proxy_add_listener)(
                wl_registry as *mut libwayland_client::wl_proxy,
                &WL_REGISTRY_LISTENER as *const libwayland_client::wl_registry_listener as _,
                boxed.as_mut() as *mut WaylandWindow as *mut c_void,
            );
            (boxed.libwayland_client.wl_display_roundtrip)(boxed.wl_display.as_ptr());
        }
        assert!(!boxed.wl_compositor.is_null());
        assert!(!boxed.wl_seat.is_null());
        assert!(!boxed.xdg_wm_base.is_null());

        unsafe {
            (boxed.libwayland_client.wl_proxy_add_listener)(
                boxed.xdg_wm_base as *mut libwayland_client::wl_proxy,
                &XDG_WM_BASE_LISTENER as *const libwayland_client::xdg_wm_base_listener as _,
                boxed.as_mut() as *mut WaylandWindow as *mut c_void,
            );
        }

        log::info!("initialized globals");

        // init window

        boxed.wl_surface = unsafe {
            libwayland_client::wl_compositor_create_surface(
                &boxed.libwayland_client,
                boxed.wl_compositor,
            )
        };
        if boxed.wl_surface.is_null() {
            return Err(anyhow!("could not create wl surface"));
        }

        boxed.xdg_surface = unsafe {
            libwayland_client::xdg_wm_base_get_xdg_surface(
                &boxed.libwayland_client,
                boxed.xdg_wm_base,
                boxed.wl_surface,
            )
        };
        if boxed.xdg_surface.is_null() {
            return Err(anyhow!("could not create xdg surface"));
        }

        boxed.xdg_toplevel = unsafe {
            libwayland_client::xdg_surface_get_toplevel(&boxed.libwayland_client, boxed.xdg_surface)
        };
        if boxed.xdg_toplevel.is_null() {
            return Err(anyhow!("could not get xdg toplevel"));
        }

        unsafe {
            (boxed.libwayland_client.wl_proxy_add_listener)(
                boxed.xdg_surface as *mut libwayland_client::wl_proxy,
                &XDG_SURFACE_LISTENER as *const libwayland_client::xdg_surface_listener as _,
                boxed.as_mut() as *mut WaylandWindow as *mut c_void,
            );
            (boxed.libwayland_client.wl_proxy_add_listener)(
                boxed.xdg_toplevel as *mut libwayland_client::wl_proxy,
                &XDG_TOPLEVEL_LISTENER as *const libwayland_client::xdg_toplevel_listener as _,
                boxed.as_mut() as *mut WaylandWindow as *mut c_void,
            );
            libwayland_client::wl_surface_commit(&boxed.libwayland_client, boxed.wl_surface);
            (boxed.libwayland_client.wl_display_roundtrip)(boxed.wl_display.as_ptr());
        }

        log::info!("initialized window");

        Ok(boxed)
    }
}

impl rwh::HasDisplayHandle for WaylandWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let wayland_display_handle = rwh::WaylandDisplayHandle::new(self.wl_display.cast());
        let raw_display_handle = rwh::RawDisplayHandle::Wayland(wayland_display_handle);
        let display_handle = unsafe { rwh::DisplayHandle::borrow_raw(raw_display_handle) };
        Ok(display_handle)
    }
}

impl rwh::HasWindowHandle for WaylandWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let Some(wl_surface) = NonNull::new(self.wl_surface) else {
            return Err(rwh::HandleError::Unavailable);
        };
        let wayland_window_handle = rwh::WaylandWindowHandle::new(wl_surface.cast());
        let raw_window_handle = rwh::RawWindowHandle::Wayland(wayland_window_handle);
        let window_handle = unsafe { rwh::WindowHandle::borrow_raw(raw_window_handle) };
        Ok(window_handle)
    }
}

impl Window for WaylandWindow {
    fn update(&mut self) -> anyhow::Result<()> {
        let n = unsafe {
            (self.libwayland_client.wl_display_dispatch_pending)(self.wl_display.as_ptr())
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
