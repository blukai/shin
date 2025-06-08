use std::collections::{HashMap, VecDeque};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr::{NonNull, null_mut};
use std::slice;

use anyhow::{Context as _, anyhow};
use raw_window_handle as rwh;

use crate::{
    CursorShape, DEFAULT_LOGICAL_SIZE, Event, KeyboardEvent, Keycode, PointerButton, PointerEvent,
    Scancode, Window, WindowAttrs, WindowEvent, libwayland_client, libwayland_cursor, xkb,
};

// https://github.com/torvalds/linux/blob/231825b2e1ff6ba799c5eaf396d3ab2354e37c6b/include/uapi/linux/input-event-codes.h#L356
const BTN_LEFT: u32 = 0x110;
const BTN_RIGHT: u32 = 0x111;
const BTN_MIDDLE: u32 = 0x112;

#[inline]
fn map_pointer_button(button: u32) -> Option<PointerButton> {
    match button {
        BTN_LEFT => Some(PointerButton::Primary),
        BTN_RIGHT => Some(PointerButton::Secondary),
        BTN_MIDDLE => Some(PointerButton::Tertiary),
        _ => None,
    }
}

// NOTE: it seems like people on the internet default to 24.
const CURSOR_SIZE: u32 = 24;

// The threshold for rounding down the final scale of the cursor image that gets sent to the
// Wayland compositor. For instance, if the original cursor image scale is 1.2, we'll downscale it
// to 1.0. On the other hand, if it's something like 1.5 then we'll upscale it to 2.0.
//
// stolen from chrome (wayland_cursor_factory.cc)
const CURSOR_SCALE_FLOORING_THRESHOLD: f64 = 0.2;

// Wayland only supports cursor images with an integer scale, so we must upscale cursor images with
// non-integer scales to integer scaled images so that the cursor is displayed correctly.
//
// stolen from chrome (wayland_cursor_factory.cc)
fn get_cursor_scale(scale_factor: f64) -> u32 {
    (scale_factor - CURSOR_SCALE_FLOORING_THRESHOLD).ceil() as u32
}

// https://gitlab.freedesktop.org/wayland/wayland/-/blob/827d0c30adc4519fafa7a9c725ff355b1d4fa3bd/cursor/cursor-data.h
// reference https://www.freedesktop.org/wiki/Specifications/cursor-spec/
//
// https://wayland.app/protocols/cursor-shape-v1, which is not completely relevant reference
// https://drafts.csswg.org/css-ui/#cursor
#[inline]
fn map_cursor_shape(cursor_shape: CursorShape) -> &'static CStr {
    match cursor_shape {
        CursorShape::Default => c"default",
        CursorShape::Pointer => c"pointer",
    }
}

// https://github.com/torvalds/linux/blob/231825b2e1ff6ba799c5eaf396d3ab2354e37c6b/include/uapi/linux/input-event-codes.h#L76

const KEY_ESC: u32 = 1;
const KEY_W: u32 = 17;
const KEY_A: u32 = 30;
const KEY_S: u32 = 31;
const KEY_D: u32 = 32;

#[inline]
fn map_keyboard_key(key: u32) -> Option<Scancode> {
    match key {
        KEY_ESC => Some(Scancode::Esc),
        KEY_W => Some(Scancode::W),
        KEY_A => Some(Scancode::A),
        KEY_S => Some(Scancode::S),
        KEY_D => Some(Scancode::D),
        _ => None,
    }
}

/// > Offset between evdev keycodes (where KEY_ESCAPE is 1), and the evdev XKB keycode set (where
/// ESC is 9). */
/// - https://github.com/xkbcommon/libxkbcommon/pull/359
/// - https://github.com/xkbcommon/libxkbcommon/blob/eb0a1457f4ada160d03f6d938fa31f6b049cb403/doc/keymap-format-text-v1.md
const EVDEV_OFFSET: u32 = 8;

#[derive(PartialEq, Eq, Hash)]
enum SerialType {
    PointerEnter,
    KeyboardEnter,
}

#[derive(Default)]
struct SerialTracker {
    serial_map: HashMap<SerialType, u32>,
}

impl SerialTracker {
    fn update_serial(&mut self, ty: SerialType, serial: u32) {
        self.serial_map.insert(ty, serial);
    }

    fn reset_serial(&mut self, ty: SerialType) {
        self.serial_map.remove(&ty);
    }

    fn get_serial(&self, ty: SerialType) -> Option<u32> {
        self.serial_map.get(&ty).cloned()
    }
}

pub struct WaylandBackend {
    libwayland_client: libwayland_client::Lib,
    libwayland_cursor: libwayland_cursor::Lib,

    wl_display: NonNull<libwayland_client::wl_display>,

    // interfaces
    wl_compositor: *mut libwayland_client::wl_compositor,
    wl_seat: *mut libwayland_client::wl_seat,
    wl_shm: *mut libwayland_client::wl_shm,
    wp_fractional_scale_manager_v1: *mut libwayland_client::wp_fractional_scale_manager_v1,
    wp_viewporter: *mut libwayland_client::wp_viewporter,
    xdg_wm_base: *mut libwayland_client::xdg_wm_base,

    // window
    attrs: WindowAttrs,
    wl_surface: *mut libwayland_client::wl_surface,
    xdg_surface: *mut libwayland_client::xdg_surface,
    xdg_toplevel: *mut libwayland_client::xdg_toplevel,
    acked_first_xdg_surface_ack_configure: bool,

    // dpi
    wp_fractional_scale_v1: *mut libwayland_client::wp_fractional_scale_v1,
    wp_viewport: *mut libwayland_client::wp_viewport,
    logical_size: Option<(u32, u32)>,
    scale_factor: Option<f64>,

    // pointer
    cursor_shape: Option<CursorShape>,
    // NOTE: currently i care only about movement and button press/release events. but other kinds
    // of events will most likely require to store different kind of frame data that PointerEvent
    // would not be capable of describing?
    pointer_frame_events: VecDeque<PointerEvent>,
    wl_pointer: *mut libwayland_client::wl_pointer,
    cursor_theme: *mut libwayland_cursor::wl_cursor_theme,
    cursor_surface: *mut libwayland_client::wl_surface,

    // keyboard
    wl_keyboard: *mut libwayland_client::wl_keyboard,
    xkb_context: Option<xkb::Context>,

    serial_tracker: SerialTracker,
    events: VecDeque<Event>,
}

unsafe extern "C" fn handle_wl_registry_global(
    data: *mut c_void,
    wl_registry: *mut libwayland_client::wl_registry,
    name: u32,
    interface: *const c_char,
    version: u32,
) {
    unsafe {
        let evl = &mut *(data as *mut WaylandBackend);

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
            "wl_shm" => {
                evl.wl_shm = libwayland_client::wl_registry_bind(
                    &evl.libwayland_client,
                    wl_registry,
                    name,
                    &libwayland_client::wl_shm_interface,
                    2.min(version),
                ) as _;
            }
            "wp_fractional_scale_manager_v1" => {
                evl.wp_fractional_scale_manager_v1 = libwayland_client::wl_registry_bind(
                    &evl.libwayland_client,
                    wl_registry,
                    name,
                    &libwayland_client::wp_fractional_scale_manager_v1_interface,
                    1.min(version),
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
    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    unsafe { libwayland_client::xdg_wm_base_pong(&evl.libwayland_client, xdg_wm_base, serial) };
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

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    unsafe {
        libwayland_client::xdg_surface_ack_configure(&evl.libwayland_client, xdg_surface, serial)
    };
    evl.acked_first_xdg_surface_ack_configure = true;
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

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };

    // NOTE: if the width or height arguments are zero, it means the client should decide its own
    // window dimension.
    assert!(width >= 0 && height >= 0);
    let logical_size = (width > 0 || height > 0)
        .then_some((width as u32, height as u32))
        .or(evl.logical_size)
        .unwrap_or(DEFAULT_LOGICAL_SIZE);

    evl.maybe_resize(Some(logical_size), None);
}

unsafe extern "C" fn handle_xdg_toplevel_close(
    data: *mut c_void,
    _xdg_toplevel: *mut libwayland_client::xdg_toplevel,
) {
    log::debug!("recv xdg_toplevel_close");

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    evl.events
        .push_back(Event::Window(WindowEvent::CloseRequested));
}

const XDG_TOPLEVEL_LISTENER: libwayland_client::xdg_toplevel_listener =
    libwayland_client::xdg_toplevel_listener {
        configure: handle_xdg_toplevel_configure,
        close: handle_xdg_toplevel_close,
        wm_capabilities: libwayland_client::noop_listener!(),
        configure_bounds: libwayland_client::noop_listener!(),
    };

unsafe extern "C" fn handle_wp_fractional_scale_v1_preferred_scale(
    data: *mut c_void,
    _wp_fractional_scale_v1: *mut libwayland_client::wp_fractional_scale_v1,
    scale: u32,
) {
    log::debug!("recv wp_fractional_scale_v1_preferred_scale");

    // > The sent scale is the numerator of a fraction with a denominator of 120.
    let scale_factor = scale as f64 / 120.0;

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    evl.maybe_resize(None, Some(scale_factor));
}

const WP_FRACTIONAL_SCALE_MANAGER_V1_LISTENER: libwayland_client::wp_fractional_scale_v1_listener =
    libwayland_client::wp_fractional_scale_v1_listener {
        preferred_scale: handle_wp_fractional_scale_v1_preferred_scale,
    };

unsafe extern "C" fn handle_wl_pointer_motion(
    data: *mut c_void,
    _wl_pointer: *mut libwayland_client::wl_pointer,
    _time: u32,
    surface_x: libwayland_client::wl_fixed,
    surface_y: libwayland_client::wl_fixed,
) {
    let evl = unsafe { &mut *(data as *mut WaylandBackend) };

    let position = (
        libwayland_client::wl_fixed_to_f64(surface_x),
        libwayland_client::wl_fixed_to_f64(surface_y),
    );
    evl.pointer_frame_events
        .push_back(PointerEvent::Motion { position });
}

unsafe extern "C" fn handle_wl_pointer_enter(
    data: *mut c_void,
    _wl_pointer: *mut libwayland_client::wl_pointer,
    serial: u32,
    _surface: *mut libwayland_client::wl_surface,
    _surface_x: libwayland_client::wl_fixed,
    _surface_y: libwayland_client::wl_fixed,
) {
    log::debug!("recv wl_pointer_enter");

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    evl.serial_tracker
        .update_serial(SerialType::PointerEnter, serial);
    if let Err(err) = evl.set_cursor_shape(evl.cursor_shape.unwrap_or(CursorShape::Default)) {
        log::error!("could not set cursor shape (pointer enter): {err:?}");
    }
}

unsafe extern "C" fn handle_wl_pointer_leave(
    data: *mut c_void,
    _wl_pointer: *mut libwayland_client::wl_pointer,
    _serial: u32,
    _surface: *mut libwayland_client::wl_surface,
) {
    log::debug!("recv wl_pointer_leave");

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    evl.serial_tracker.reset_serial(SerialType::PointerEnter);
    evl.cursor_shape = None;
}

unsafe extern "C" fn handle_wl_pointer_button(
    data: *mut c_void,
    _wl_pointer: *mut libwayland_client::wl_pointer,
    _serial: u32,
    _time: u32,
    button: u32,
    state: u32,
) {
    let Some(button) = map_pointer_button(button) else {
        log::debug!("unidentified pointer button: {button}");
        return;
    };

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };

    let pressed = state == libwayland_client::WL_POINTER_BUTTON_STATE_PRESSED;
    evl.pointer_frame_events.push_back(if pressed {
        PointerEvent::Press { button }
    } else {
        PointerEvent::Release { button }
    });
}

unsafe extern "C" fn handle_wl_pointer_frame(
    data: *mut c_void,
    _wl_pointer: *mut libwayland_client::wl_pointer,
) {
    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    evl.events
        .extend(evl.pointer_frame_events.drain(..).map(Event::Pointer));
}

const WL_POINTER_LISTENER: libwayland_client::wl_pointer_listener =
    libwayland_client::wl_pointer_listener {
        enter: handle_wl_pointer_enter,
        leave: handle_wl_pointer_leave,
        motion: handle_wl_pointer_motion,
        button: handle_wl_pointer_button,
        frame: handle_wl_pointer_frame,
        axis: libwayland_client::noop_listener!(),
        axis_source: libwayland_client::noop_listener!(),
        axis_stop: libwayland_client::noop_listener!(),
        axis_discrete: libwayland_client::noop_listener!(),
        axis_value120: libwayland_client::noop_listener!(),
        axis_relative_direction: libwayland_client::noop_listener!(),
    };

// TODO: will need this to be able to map scancodes to keycodes with libxkbcommon.
unsafe extern "C" fn handle_wl_keyboard_keymap(
    data: *mut c_void,
    _wl_keyboard: *mut libwayland_client::wl_keyboard,
    format: u32,
    fd: i32,
    size: u32,
) {
    log::debug!("recv wl_keyboard_keymap");

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };

    match format {
        libwayland_client::WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1 => {
            assert!(evl.xkb_context.is_none());
            let xkb_context =
                unsafe { xkb::Context::from_fd(fd, size) }.expect("could not create xkb context");
            evl.xkb_context = Some(xkb_context);
            log::info!("created xkb context");
        }
        other => unreachable!("unknown keymap format: {other}"),
    }

    unsafe { libc::close(fd) };
}

unsafe extern "C" fn handle_wl_keyboard_enter(
    data: *mut c_void,
    _wl_keyboard: *mut libwayland_client::wl_keyboard,
    serial: u32,
    _surface: *mut libwayland_client::wl_surface,
    _keys: *mut libwayland_client::wl_array,
) {
    log::debug!("recv wl_keyboard_enter");

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    evl.serial_tracker
        .update_serial(SerialType::KeyboardEnter, serial);
}

unsafe extern "C" fn handle_wl_keyboard_leave(
    data: *mut c_void,
    _wl_keyboard: *mut libwayland_client::wl_keyboard,
    _serial: u32,
    _surface: *mut libwayland_client::wl_surface,
) {
    log::debug!("recv wl_keyboard_leave");

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    evl.serial_tracker.reset_serial(SerialType::KeyboardEnter);
}

unsafe extern "C" fn handle_wl_keyboard_key(
    data: *mut c_void,
    _wl_keyboard: *mut libwayland_client::wl_keyboard,
    _serial: u32,
    _time: u32,
    key: u32,
    state: u32,
) {
    let Some(scancode) = map_keyboard_key(key) else {
        log::debug!("unidentified keyboard key: {key}");
        return;
    };

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    let xkb_context = evl
        .xkb_context
        .as_ref()
        .expect("xkb contex has not been created");

    let sym = unsafe {
        (xkb_context.lib.xkb_state_key_get_one_sym)(xkb_context.state, key + EVDEV_OFFSET)
    };
    let utf32 = unsafe { (xkb_context.lib.xkb_keysym_to_utf32)(sym) };
    let keycode = char::from_u32(utf32).map_or_else(|| Keycode::Unhandled, Keycode::Char);

    let pressed = state == libwayland_client::WL_KEYBOARD_KEY_STATE_PRESSED;
    evl.events.push_back(Event::Keyboard(if pressed {
        KeyboardEvent::Press { scancode, keycode }
    } else {
        KeyboardEvent::Release { scancode, keycode }
    }));
}

unsafe extern "C" fn handle_wl_keyboard_modifiers(
    data: *mut c_void,
    _wl_keyboard: *mut libwayland_client::wl_keyboard,
    _serial: u32,
    mods_depressed: u32,
    mods_latched: u32,
    mods_locked: u32,
    group: u32,
) {
    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    let xkb_context = evl
        .xkb_context
        .as_ref()
        .expect("xkb contex has not been created");
    unsafe {
        (xkb_context.lib.xkb_state_update_mask)(
            xkb_context.state,
            mods_depressed,
            mods_latched,
            mods_locked,
            0,
            0,
            group,
        )
    };
}

const WL_KEYBOARD_LISTENER: libwayland_client::wl_keyboard_listener =
    libwayland_client::wl_keyboard_listener {
        keymap: handle_wl_keyboard_keymap,
        enter: handle_wl_keyboard_enter,
        leave: handle_wl_keyboard_leave,
        key: handle_wl_keyboard_key,
        modifiers: handle_wl_keyboard_modifiers,
        repeat_info: libwayland_client::noop_listener!(),
    };

impl WaylandBackend {
    pub fn new_boxed(attrs: WindowAttrs) -> anyhow::Result<Box<Self>> {
        let libwayland_client = libwayland_client::Lib::load()?;
        let libwayland_cursor = libwayland_cursor::Lib::load()?;

        let wl_display =
            NonNull::new(unsafe { (libwayland_client.wl_display_connect)(null_mut()) })
                .context("could not connect to wayland display")?;

        let mut boxed = Box::new(WaylandBackend {
            libwayland_client,
            libwayland_cursor,

            wl_display,

            wl_compositor: null_mut(),
            wl_seat: null_mut(),
            wl_shm: null_mut(),
            wp_fractional_scale_manager_v1: null_mut(),
            wp_viewporter: null_mut(),
            xdg_wm_base: null_mut(),

            attrs,
            wl_surface: null_mut(),
            xdg_surface: null_mut(),
            xdg_toplevel: null_mut(),
            acked_first_xdg_surface_ack_configure: false,

            wp_fractional_scale_v1: null_mut(),
            wp_viewport: null_mut(),
            logical_size: None,
            scale_factor: None,

            cursor_shape: None,
            pointer_frame_events: VecDeque::new(),
            wl_pointer: null_mut(),
            cursor_theme: null_mut(),
            cursor_surface: null_mut(),

            wl_keyboard: null_mut(),
            xkb_context: None,

            serial_tracker: SerialTracker::default(),
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
                boxed.as_mut() as *mut WaylandBackend as *mut c_void,
            );
            (boxed.libwayland_client.wl_display_roundtrip)(boxed.wl_display.as_ptr());
        }

        assert!(!boxed.wl_compositor.is_null());
        assert!(!boxed.wl_seat.is_null());
        assert!(!boxed.wl_shm.is_null());
        assert!(!boxed.xdg_wm_base.is_null());

        log::info!("initialized globals");

        unsafe {
            (boxed.libwayland_client.wl_proxy_add_listener)(
                boxed.xdg_wm_base as *mut libwayland_client::wl_proxy,
                &XDG_WM_BASE_LISTENER as *const libwayland_client::xdg_wm_base_listener as _,
                boxed.as_mut() as *mut WaylandBackend as *mut c_void,
            )
        };

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
        unsafe {
            (boxed.libwayland_client.wl_proxy_add_listener)(
                boxed.xdg_surface as *mut libwayland_client::wl_proxy,
                &XDG_SURFACE_LISTENER as *const libwayland_client::xdg_surface_listener as _,
                boxed.as_mut() as *mut WaylandBackend as *mut c_void,
            )
        };

        boxed.xdg_toplevel = unsafe {
            libwayland_client::xdg_surface_get_toplevel(&boxed.libwayland_client, boxed.xdg_surface)
        };
        if boxed.xdg_toplevel.is_null() {
            return Err(anyhow!("could not get xdg toplevel"));
        }
        unsafe {
            (boxed.libwayland_client.wl_proxy_add_listener)(
                boxed.xdg_toplevel as *mut libwayland_client::wl_proxy,
                &XDG_TOPLEVEL_LISTENER as *const libwayland_client::xdg_toplevel_listener as _,
                boxed.as_mut() as *mut WaylandBackend as *mut c_void,
            )
        };

        if !boxed.attrs.resizable {
            let logical_size = boxed.attrs.logical_size.unwrap_or(DEFAULT_LOGICAL_SIZE);
            unsafe {
                libwayland_client::xdg_toplevel_set_min_size(
                    &boxed.libwayland_client,
                    boxed.xdg_toplevel,
                    logical_size.0 as i32,
                    logical_size.1 as i32,
                );
                libwayland_client::xdg_toplevel_set_max_size(
                    &boxed.libwayland_client,
                    boxed.xdg_toplevel,
                    logical_size.0 as i32,
                    logical_size.1 as i32,
                )
            };
        }

        // dpi

        if !boxed.wp_fractional_scale_manager_v1.is_null() {
            boxed.wp_fractional_scale_v1 = unsafe {
                libwayland_client::wp_fractional_scale_manager_v1_get_fractional_scale(
                    &boxed.libwayland_client,
                    boxed.wp_fractional_scale_manager_v1,
                    boxed.wl_surface,
                )
            };
            if boxed.wp_fractional_scale_v1.is_null() {
                return Err(anyhow!("could not get fractional scale"));
            }
            unsafe {
                (boxed.libwayland_client.wl_proxy_add_listener)(
                    boxed.wp_fractional_scale_v1 as *mut libwayland_client::wl_proxy,
                    &WP_FRACTIONAL_SCALE_MANAGER_V1_LISTENER
                        as *const libwayland_client::wp_fractional_scale_v1_listener
                        as _,
                    boxed.as_mut() as *mut WaylandBackend as *mut c_void,
                )
            };
        }

        if !boxed.wp_viewporter.is_null() {
            boxed.wp_viewport = unsafe {
                libwayland_client::wp_viewporter_get_viewport(
                    &boxed.libwayland_client,
                    boxed.wp_viewporter,
                    boxed.wl_surface,
                )
            };
        }

        // pointer

        boxed.wl_pointer = unsafe {
            libwayland_client::wl_seat_get_pointer(&boxed.libwayland_client, boxed.wl_seat)
        };
        if boxed.wl_pointer.is_null() {
            return Err(anyhow!("could not get pointer"));
        }
        unsafe {
            (boxed.libwayland_client.wl_proxy_add_listener)(
                boxed.wl_pointer as *mut libwayland_client::wl_proxy,
                &WL_POINTER_LISTENER as *const libwayland_client::wl_pointer_listener as _,
                boxed.as_mut() as *mut WaylandBackend as *mut c_void,
            )
        };

        boxed.cursor_surface = unsafe {
            libwayland_client::wl_compositor_create_surface(
                &boxed.libwayland_client,
                boxed.wl_compositor,
            )
        };
        assert!(!boxed.cursor_surface.is_null());
        assert!(boxed.scale_factor.is_none());
        boxed.load_cursor_theme_for_scale(1.0);

        // keyboard

        boxed.wl_keyboard = unsafe {
            libwayland_client::wl_seat_get_keyboard(&boxed.libwayland_client, boxed.wl_seat)
        };
        if boxed.wl_keyboard.is_null() {
            return Err(anyhow!("could not get keyboard"));
        }
        unsafe {
            (boxed.libwayland_client.wl_proxy_add_listener)(
                boxed.wl_keyboard as *mut libwayland_client::wl_proxy,
                &WL_KEYBOARD_LISTENER as *const libwayland_client::wl_keyboard_listener as _,
                boxed.as_mut() as *mut WaylandBackend as *mut c_void,
            )
        };

        // finalize

        unsafe { libwayland_client::wl_surface_commit(&boxed.libwayland_client, boxed.wl_surface) };
        unsafe { (boxed.libwayland_client.wl_display_roundtrip)(boxed.wl_display.as_ptr()) };

        // TODO: consider waiting for fractional scale event (if fractional scale interface exists)
        assert!(boxed.acked_first_xdg_surface_ack_configure);
        boxed
            .events
            .push_back(Event::Window(WindowEvent::Configure {
                logical_size: boxed.logical_size.expect("configured logical size"),
            }));

        log::info!("initialized window");

        Ok(boxed)
    }

    fn maybe_resize_viewport(&mut self, logical_size: (u32, u32)) {
        if self.wp_viewport.is_null() {
            return;
        }
        unsafe {
            libwayland_client::wp_viewport_set_destination(
                &self.libwayland_client,
                self.wp_viewport,
                logical_size.0 as i32,
                logical_size.1 as i32,
            );
            assert!(!self.wl_surface.is_null());
            libwayland_client::wl_surface_commit(&self.libwayland_client, self.wl_surface);
        }
    }

    fn load_cursor_theme_for_scale(&mut self, scale_factor: f64) {
        let cursor_scale = get_cursor_scale(scale_factor);
        self.cursor_theme = unsafe {
            assert!(!self.wl_shm.is_null());
            (self.libwayland_cursor.wl_cursor_theme_load)(
                map_cursor_shape(CursorShape::Default).as_ptr(),
                (CURSOR_SIZE * cursor_scale) as c_int,
                self.wl_shm,
            )
        };
        assert!(!self.cursor_theme.is_null());
        unsafe {
            libwayland_client::wl_surface_set_buffer_scale(
                &self.libwayland_client,
                self.cursor_surface,
                cursor_scale as i32,
            );
            libwayland_client::wl_surface_commit(&self.libwayland_client, self.cursor_surface);
        }
    }

    fn maybe_resize(&mut self, logical_size: Option<(u32, u32)>, scale_factor: Option<f64>) {
        assert!(logical_size.is_some() || scale_factor.is_some());

        let mut logical_size_changed = false;
        if let Some(logical_size) = logical_size {
            logical_size_changed = self.logical_size != Some(logical_size);
            if logical_size_changed {
                self.logical_size = Some(logical_size);

                self.maybe_resize_viewport(logical_size);
            }
        }

        let mut scale_factor_changed = false;
        if let Some(scale_factor) = scale_factor {
            scale_factor_changed = self.scale_factor != Some(scale_factor);
            if scale_factor_changed {
                self.scale_factor = Some(scale_factor);

                self.load_cursor_theme_for_scale(scale_factor);
                if let Err(err) =
                    self.set_cursor_shape(self.cursor_shape.unwrap_or(CursorShape::Default))
                {
                    log::error!("could not set cursor shape (rescaled): {err:?}");
                }
            }
        }

        if !logical_size_changed && !scale_factor_changed {
            return;
        }

        self.events.push_back(Event::Window(WindowEvent::Resized {
            physical_size: self.size(),
        }));

        if scale_factor_changed {
            self.events
                .push_back(Event::Window(WindowEvent::ScaleFactorChanged {
                    scale_factor: self.scale_factor(),
                }));
        }
    }
}

impl rwh::HasDisplayHandle for WaylandBackend {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let wayland = rwh::WaylandDisplayHandle::new(self.wl_display.cast());
        let raw = rwh::RawDisplayHandle::Wayland(wayland);
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(raw) })
    }
}

impl rwh::HasWindowHandle for WaylandBackend {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        let Some(wl_surface) = NonNull::new(self.wl_surface) else {
            return Err(rwh::HandleError::Unavailable);
        };
        let wayland = rwh::WaylandWindowHandle::new(wl_surface.cast());
        let raw = rwh::RawWindowHandle::Wayland(wayland);
        Ok(unsafe { rwh::WindowHandle::borrow_raw(raw) })
    }
}

impl Window for WaylandBackend {
    fn pump_events(&mut self) -> anyhow::Result<()> {
        let n = unsafe {
            (self.libwayland_client.wl_display_dispatch_pending)(self.wl_display.as_ptr())
        };
        if n == -1 {
            return Err(anyhow!("wl_display_dispatch_pending failed"));
        }
        Ok(())
    }

    fn pop_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    fn set_cursor_shape(&mut self, cursor_shape: CursorShape) -> anyhow::Result<()> {
        assert!(!self.cursor_theme.is_null());
        assert!(!self.cursor_surface.is_null());

        if self.cursor_shape.is_some_and(|cs| cs == cursor_shape) {
            return Ok(());
        }

        let Some(serial) = self.serial_tracker.get_serial(SerialType::PointerEnter) else {
            log::warn!("no pointer enter serial found");
            return Ok(());
        };

        let cursor_name = map_cursor_shape(cursor_shape);
        let cursor = unsafe {
            (self.libwayland_cursor.wl_cursor_theme_get_cursor)(
                self.cursor_theme,
                cursor_name.as_ptr(),
            )
        };
        if cursor.is_null() {
            log::warn!("could not find {cursor_name:?} cursor");
            return Ok(());
        };
        let cursor = unsafe { &*cursor };

        let cursor_images =
            unsafe { slice::from_raw_parts(cursor.images, cursor.image_count as usize) };
        let cursor_image_ptr = cursor_images[0];
        let cursor_image = unsafe { &*cursor_image_ptr };

        let cursor_image_buffer =
            unsafe { (self.libwayland_cursor.wl_cursor_image_get_buffer)(cursor_image_ptr) };
        if cursor_image_buffer.is_null() {
            return Err(anyhow!("could not get cursor image buffer"));
        }

        unsafe {
            libwayland_client::wl_surface_attach(
                &self.libwayland_client,
                self.cursor_surface,
                cursor_image_buffer,
                0,
                0,
            );

            // NOTE: pre version 4 wl_surface::damage must be used instead.
            let wl_surface_version = (self.libwayland_client.wl_proxy_get_version)(
                self.cursor_surface as *mut libwayland_client::wl_proxy,
            );
            assert!(wl_surface_version >= 4);
            libwayland_client::wl_surface_damage_buffer(
                &self.libwayland_client,
                self.cursor_surface,
                0,
                0,
                cursor_image.width as i32,
                cursor_image.height as i32,
            );
            libwayland_client::wl_surface_commit(&self.libwayland_client, self.cursor_surface);

            libwayland_client::wl_pointer_set_cursor(
                &self.libwayland_client,
                self.wl_pointer,
                serial,
                self.cursor_surface,
                cursor_image.hotspot_x as i32,
                cursor_image.hotspot_y as i32,
            );
        }

        self.cursor_shape = Some(cursor_shape);
        Ok(())
    }

    fn scale_factor(&self) -> f64 {
        self.scale_factor.unwrap_or(1.0)
    }

    fn size(&self) -> (u32, u32) {
        let logical_size = self.logical_size.expect("logical size");
        let scale_factor = self.scale_factor.unwrap_or(1.0);
        (
            (logical_size.0 as f64 * scale_factor) as u32,
            (logical_size.1 as f64 * scale_factor) as u32,
        )
    }
}
