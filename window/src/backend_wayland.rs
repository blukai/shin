use std::collections::{HashMap, VecDeque};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::mem::MaybeUninit;
use std::ptr::{NonNull, null, null_mut};
use std::slice;
use std::time::Duration;

use anyhow::{Context as _, anyhow};
use input::{CursorShape, KeyboardEvent, Keycode, PointerButton, PointerEvent, Scancode};
use raw_window_handle as rwh;

use crate::{
    DEFAULT_LOGICAL_SIZE, Event, Window, WindowAttrs, WindowEvent, libwayland_client,
    libwayland_cursor, xkb,
};

// https://github.com/torvalds/linux/blob/231825b2e1ff6ba799c5eaf396d3ab2354e37c6b/include/uapi/linux/input-event-codes.h#L356
#[inline]
fn map_pointer_button(button: u32) -> Option<PointerButton> {
    match button {
        0x110 => Some(PointerButton::Primary),
        0x111 => Some(PointerButton::Secondary),
        0x112 => Some(PointerButton::Tertiary),
        _ => None,
    }
}

// TODO: hookup cursor shape - https://wayland.app/protocols/cursor-shape-v1;
// maybe don't completely throw away current cursor shaping implementation?

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
fn get_cursor_rounded_scale(scale_factor: f64) -> u32 {
    (scale_factor - CURSOR_SCALE_FLOORING_THRESHOLD).ceil() as u32
}

fn load_cursor_theme(
    libwayland_cursor: &libwayland_cursor::Lib,
    wl_shm: *mut libwayland_client::wl_shm,
    scale_factor: f64,
) -> anyhow::Result<*mut libwayland_cursor::wl_cursor_theme> {
    assert!(!wl_shm.is_null());

    let rounded_scale = get_cursor_rounded_scale(scale_factor);
    let scaled_size = CURSOR_SIZE * rounded_scale;

    let cursor_theme =
        unsafe { (libwayland_cursor.wl_cursor_theme_load)(null(), scaled_size as c_int, wl_shm) };
    if cursor_theme.is_null() {
        return Err(anyhow!("could not load cursor theme"));
    }

    Ok(cursor_theme)
}

// https://gitlab.freedesktop.org/wayland/wayland/-/blob/827d0c30adc4519fafa7a9c725ff355b1d4fa3bd/cursor/cursor-data.h
// reference https://www.freedesktop.org/wiki/Specifications/cursor-spec/
//
// https://wayland.app/protocols/cursor-shape-v1, which is not completely relevant reference
// https://drafts.csswg.org/css-ui/#cursor
fn map_cursor_shape_to_name(shape: CursorShape) -> &'static CStr {
    match shape {
        CursorShape::Default => c"default",
        CursorShape::Pointer => c"pointer",
        CursorShape::Text => c"Text",
    }
}

fn map_cursor_shape_to_enum(shape: CursorShape) -> u32 {
    match shape {
        CursorShape::Default => libwayland_client::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_DEFAULT,
        CursorShape::Pointer => libwayland_client::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_POINTER,
        CursorShape::Text => libwayland_client::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_TEXT,
    }
}

struct Cursor {
    libwayland_cursor: libwayland_cursor::Lib,
    wl_cursor_theme: *mut libwayland_cursor::wl_cursor_theme,
    wl_surface: *mut libwayland_client::wl_surface,
    scale: f64,
}

impl Cursor {
    fn init(
        libwayland_client: &libwayland_client::Lib,
        wl_compositor: *mut libwayland_client::wl_compositor,
        wl_shm: *mut libwayland_client::wl_shm,
        scale: f64,
    ) -> anyhow::Result<Self> {
        assert!(!wl_compositor.is_null());

        let libwayland_cursor = libwayland_cursor::Lib::load()?;

        let wl_surface = unsafe {
            libwayland_client::wl_compositor_create_surface(libwayland_client, wl_compositor)
        };
        if wl_surface.is_null() {
            return Err(anyhow!("could not create wl_surface for cursor"));
        }

        let wl_cursor_theme = load_cursor_theme(&libwayland_cursor, wl_shm, scale)?;

        Ok(Self {
            libwayland_cursor,
            wl_cursor_theme,
            wl_surface,
            scale,
        })
    }

    #[expect(dead_code, reason = "TODO: consider deininting wayland xd")]
    fn deinit(self, libwayland_client: &libwayland_client::Lib) {
        if !self.wl_surface.is_null() {
            unsafe { libwayland_client::wl_surface_destroy(libwayland_client, self.wl_surface) };
        }
        if !self.wl_cursor_theme.is_null() {
            unsafe { (self.libwayland_cursor.wl_cursor_theme_destroy)(self.wl_cursor_theme) };
        }
    }

    /// NOTE: should call set_shape right after. this function will not update shape to scale.
    fn set_scale(
        &mut self,
        wl_shm: *mut libwayland_client::wl_shm,
        scale: f64,
    ) -> anyhow::Result<()> {
        if !self.wl_cursor_theme.is_null() {
            unsafe { (self.libwayland_cursor.wl_cursor_theme_destroy)(self.wl_cursor_theme) };
            self.wl_cursor_theme = null_mut();
        }

        self.wl_cursor_theme = load_cursor_theme(&self.libwayland_cursor, wl_shm, scale)?;
        self.scale = scale;

        Ok(())
    }

    fn set_shape(
        &self,
        libwayland_client: &libwayland_client::Lib,
        wl_pointer: *mut libwayland_client::wl_pointer,
        name: &'static CStr,
        serial: u32,
    ) -> anyhow::Result<()> {
        assert!(!wl_pointer.is_null());
        assert!(serial != 0); // NOTE: pretty certain that 0 is not a valid serial.

        let cursor = unsafe {
            (self.libwayland_cursor.wl_cursor_theme_get_cursor)(self.wl_cursor_theme, name.as_ptr())
        };
        if cursor.is_null() {
            return Err(anyhow!("could not get cursor {name:?}"));
        };

        let cursor = unsafe { &*cursor };
        let images = unsafe { slice::from_raw_parts(cursor.images, cursor.image_count as usize) };

        let image_ptr = images[0];
        let image = unsafe { &*image_ptr };
        let image_buffer =
            unsafe { (self.libwayland_cursor.wl_cursor_image_get_buffer)(image_ptr) };
        if image_buffer.is_null() {
            return Err(anyhow!("could not get image buffer"));
        }

        // TODO: is this correct (correct enough? seems fine on scale_factor = 1.5)?
        let rounded_scale = get_cursor_rounded_scale(self.scale) as f64;
        let hotspot_x = (image.hotspot_x as f64 / rounded_scale).round() as i32;
        let hotspot_y = (image.hotspot_y as f64 / rounded_scale).round() as i32;

        unsafe {
            libwayland_client::wl_surface_set_buffer_scale(
                libwayland_client,
                self.wl_surface,
                rounded_scale as i32,
            );

            libwayland_client::wl_surface_attach(
                libwayland_client,
                self.wl_surface,
                image_buffer,
                0,
                0,
            );

            // NOTE: pre version 4 wl_surface::damage must be used instead.
            let wl_surface_version = (libwayland_client.wl_proxy_get_version)(
                self.wl_surface as *mut libwayland_client::wl_proxy,
            );
            assert!(wl_surface_version >= 4);

            libwayland_client::wl_surface_damage_buffer(
                libwayland_client,
                self.wl_surface,
                0,
                0,
                image.width as i32,
                image.height as i32,
            );

            libwayland_client::wl_pointer_set_cursor(
                libwayland_client,
                wl_pointer,
                serial,
                self.wl_surface,
                hotspot_x,
                hotspot_y,
            );

            libwayland_client::wl_surface_commit(libwayland_client, self.wl_surface);
        }

        Ok(())
    }
}

// https://github.com/torvalds/linux/blob/231825b2e1ff6ba799c5eaf396d3ab2354e37c6b/include/uapi/linux/input-event-codes.h#L76
#[inline]
fn map_keyboard_key(key: u32) -> Option<Scancode> {
    match key {
        0 => Some(Scancode::Reserved),
        1 => Some(Scancode::Esc),
        2 => Some(Scancode::Num1),
        3 => Some(Scancode::Num2),
        4 => Some(Scancode::Num3),
        5 => Some(Scancode::Num4),
        6 => Some(Scancode::Num5),
        7 => Some(Scancode::Num6),
        8 => Some(Scancode::Num7),
        9 => Some(Scancode::Num8),
        10 => Some(Scancode::Num9),
        11 => Some(Scancode::Num0),
        12 => Some(Scancode::Minus),
        13 => Some(Scancode::Equal),
        14 => Some(Scancode::Backspace),
        15 => Some(Scancode::Tab),
        16 => Some(Scancode::Q),
        17 => Some(Scancode::W),
        18 => Some(Scancode::E),
        19 => Some(Scancode::R),
        20 => Some(Scancode::T),
        21 => Some(Scancode::Y),
        22 => Some(Scancode::U),
        23 => Some(Scancode::I),
        24 => Some(Scancode::O),
        25 => Some(Scancode::P),
        26 => Some(Scancode::BraceLeft),
        27 => Some(Scancode::BraceRight),
        28 => Some(Scancode::Enter),
        29 => Some(Scancode::CtrlLeft),
        30 => Some(Scancode::A),
        31 => Some(Scancode::S),
        32 => Some(Scancode::D),
        33 => Some(Scancode::F),
        34 => Some(Scancode::G),
        35 => Some(Scancode::H),
        36 => Some(Scancode::J),
        37 => Some(Scancode::K),
        38 => Some(Scancode::L),
        39 => Some(Scancode::Semicolon),
        40 => Some(Scancode::Apostrophe),
        41 => Some(Scancode::Grave),
        42 => Some(Scancode::ShiftLeft),
        43 => Some(Scancode::Backslash),
        44 => Some(Scancode::Z),
        45 => Some(Scancode::X),
        46 => Some(Scancode::C),
        47 => Some(Scancode::V),
        48 => Some(Scancode::B),
        49 => Some(Scancode::N),
        50 => Some(Scancode::M),
        51 => Some(Scancode::Comma),
        52 => Some(Scancode::Dot),
        53 => Some(Scancode::Slash),
        54 => Some(Scancode::ShiftRight),
        // 55 => Some(Scancode::KPAsterisk),
        56 => Some(Scancode::AltLeft),
        57 => Some(Scancode::Space),
        58 => Some(Scancode::CapsLock),
        // 59 => Some(Scancode::F1),
        // 60 => Some(Scancode::F2),
        // 61 => Some(Scancode::F3),
        // 62 => Some(Scancode::F4),
        // 63 => Some(Scancode::F5),
        // 64 => Some(Scancode::F6),
        // 65 => Some(Scancode::F7),
        // 66 => Some(Scancode::F8),
        // 67 => Some(Scancode::F9),
        // 68 => Some(Scancode::F10),
        69 => Some(Scancode::NumLock),
        70 => Some(Scancode::ScrollLock),
        // 71 => Some(Scancode::KP7),
        // 72 => Some(Scancode::KP8),
        // 73 => Some(Scancode::KP9),
        // 74 => Some(Scancode::KPMinus),
        // 75 => Some(Scancode::KP4),
        // 76 => Some(Scancode::KP5),
        // 77 => Some(Scancode::KP6),
        // 78 => Some(Scancode::KPPlus),
        // 79 => Some(Scancode::KP1),
        // 80 => Some(Scancode::KP2),
        // 81 => Some(Scancode::KP3),
        // 82 => Some(Scancode::KP0),
        // 83 => Some(Scancode::KPDOT),
        // 84 => Some(Scancode::_),
        // 85 => Some(Scancode::ZENKAKUHANKAKU),
        // 86 => Some(Scancode::102ND),
        // 87 => Some(Scancode::F11),
        // 88 => Some(Scancode::F12),
        // 89 => Some(Scancode::RO),
        // 90 => Some(Scancode::KATAKANA),
        // 91 => Some(Scancode::HIRAGANA),
        // 92 => Some(Scancode::HENKAN),
        // 93 => Some(Scancode::KATAKANAHIRAGANA),
        // 94 => Some(Scancode::MUHENKAN),
        // 95 => Some(Scancode::KPJPCOMMA),
        // 96 => Some(Scancode::KPEnter),
        97 => Some(Scancode::CtrlRight),
        // 98 => Some(Scancode::KPSlash),
        // 99 => Some(Scancode::SYSRQ),
        100 => Some(Scancode::AltRight),
        // 101 => Some(Scancode::LINEFEED),
        102 => Some(Scancode::Home),
        103 => Some(Scancode::ArrowUp),
        104 => Some(Scancode::PageUp),
        105 => Some(Scancode::ArrowLeft),
        106 => Some(Scancode::ArrowRight),
        107 => Some(Scancode::End),
        108 => Some(Scancode::ArrowDown),
        109 => Some(Scancode::PageDown),
        110 => Some(Scancode::Insert),
        111 => Some(Scancode::Delete),
        // 112 => Some(Scancode::MACRO),
        // 113 => Some(Scancode::MUTE),
        // 114 => Some(Scancode::VOLUMEDOWN),
        // 115 => Some(Scancode::VOLUMEUP),
        // 116 => Some(Scancode::POWER),
        // 117 => Some(Scancode::KPEqual),
        // 118 => Some(Scancode::KPPLUSMINUS),
        // 119 => Some(Scancode::PAUSE),
        // 120 => Some(Scancode::SCALE),
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

struct KeyRepeatInfo {
    rate: Duration,
    delay: Duration,
}

struct TimerFD(c_int);

impl TimerFD {
    unsafe fn new(clockid: libc::clockid_t, flags: c_int) -> anyhow::Result<Self> {
        let ret = unsafe { libc::timerfd_create(clockid, flags) };
        if ret == -1 {
            let errno = unsafe { *libc::__errno_location() };
            Err(anyhow!("could not create timerfd: 0x:{errno:x}"))
        } else {
            Ok(Self(ret))
        }
    }

    unsafe fn arm(&self, it_interval: Duration, it_value: Duration) -> anyhow::Result<()> {
        let timerspec = libc::itimerspec {
            it_interval: libc::timespec {
                tv_sec: it_interval.as_secs() as _,
                tv_nsec: it_interval.as_nanos() as _,
            },
            it_value: libc::timespec {
                tv_sec: it_value.as_secs() as _,
                tv_nsec: it_value.as_nanos() as _,
            },
        };
        let ret = unsafe { libc::timerfd_settime(self.0, 0, &timerspec, null_mut()) };
        if ret == -1 {
            let errno = unsafe { *libc::__errno_location() };
            Err(anyhow!("could not set timerfd time: 0x:{errno:x}"))
        } else {
            Ok(())
        }
    }

    unsafe fn disarm(&self) -> anyhow::Result<()> {
        unsafe { self.arm(Duration::ZERO, Duration::ZERO) }
    }

    unsafe fn read<T>(&self) -> anyhow::Result<T> {
        let mut value = MaybeUninit::uninit();
        let ret = unsafe { libc::read(self.0, value.as_mut_ptr() as *mut c_void, size_of::<T>()) };
        if ret != size_of::<T>() as libc::ssize_t {
            let errno = unsafe { *libc::__errno_location() };
            Err(anyhow!("could not read timerfd: 0x:{errno:x}"))
        } else {
            Ok(unsafe { value.assume_init() })
        }
    }
}

pub struct WaylandBackend {
    libwayland_client: libwayland_client::Lib,

    wl_display: NonNull<libwayland_client::wl_display>,

    // interfaces
    wl_compositor: *mut libwayland_client::wl_compositor,
    wl_seat: *mut libwayland_client::wl_seat,
    wl_shm: *mut libwayland_client::wl_shm,
    wp_cursor_shape_manager_v1: *mut libwayland_client::wp_cursor_shape_manager_v1,
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
    wl_pointer: *mut libwayland_client::wl_pointer,
    // NOTE: currently i care only about movement and button press/release events. but other kinds
    // of events will most likely require to store different kind of frame data that PointerEvent
    // would not be capable of describing?
    pointer_frame_events: VecDeque<PointerEvent>,
    cursor: Option<Cursor>,
    // NOTE: cursor_shape is stored here so that it can be set back to what was requested when
    // pointer re-enders the surface.
    cursor_shape: Option<CursorShape>,
    wp_cursor_shape_device_v1: *mut libwayland_client::wp_cursor_shape_device_v1,

    // keyboard
    wl_keyboard: *mut libwayland_client::wl_keyboard,
    xkb_context: Option<xkb::Context>,
    key_repeat_timerfd: TimerFD,
    key_repeat_info: Option<KeyRepeatInfo>,
    key_repeat: Option<(Scancode, Keycode)>,

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
            "wp_cursor_shape_manager_v1" => {
                evl.wp_cursor_shape_manager_v1 = libwayland_client::wl_registry_bind(
                    &evl.libwayland_client,
                    wl_registry,
                    name,
                    &libwayland_client::wp_cursor_shape_manager_v1_interface,
                    1.min(version),
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
    surface_x: libwayland_client::wl_fixed,
    surface_y: libwayland_client::wl_fixed,
) {
    log::debug!("recv wl_pointer_enter");

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };

    evl.serial_tracker
        .update_serial(SerialType::PointerEnter, serial);

    let cursor_shape = evl.cursor_shape.unwrap_or(CursorShape::Default);
    if let Err(err) = evl.set_cursor_shape(cursor_shape) {
        log::error!("could not set cursor shape (pointer enter): {err:?}");
    }

    let position = (
        libwayland_client::wl_fixed_to_f64(surface_x),
        libwayland_client::wl_fixed_to_f64(surface_y),
    );
    // NOTE: pushing motion event on enter is somewhat weird? idk yet how correct this is, but i
    // don't think it's worth introducing enter/leave pointer events (for what reason?).
    // ultimately doing this helps to: compute correct deltas; dispatch press with correct delta
    // when window spawns right under the cursor.
    evl.pointer_frame_events
        .push_back(PointerEvent::Motion { position });
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

    match state {
        libwayland_client::WL_POINTER_BUTTON_STATE_PRESSED => {
            let pointer_event = PointerEvent::Press { button };
            evl.pointer_frame_events.push_back(pointer_event);
        }
        libwayland_client::WL_POINTER_BUTTON_STATE_RELEASED => {
            let pointer_event = PointerEvent::Release { button };
            evl.pointer_frame_events.push_back(pointer_event);
        }
        other => log::warn!("unknown pointer button state: {other}"),
    }
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

    let key = key + EVDEV_OFFSET;
    let sym = unsafe { (xkb_context.lib.xkb_state_key_get_one_sym)(xkb_context.state, key) };
    let utf32 = unsafe { (xkb_context.lib.xkb_keysym_to_utf32)(sym) };
    let keycode = char::from_u32(utf32).map_or_else(|| Keycode::Unhandled, Keycode::Char);

    match state {
        libwayland_client::WL_KEYBOARD_KEY_STATE_PRESSED => {
            let keyboard_event = KeyboardEvent::Press {
                scancode,
                keycode,
                repeat: false,
            };
            evl.events.push_back(Event::Keyboard(keyboard_event));

            if let Some(KeyRepeatInfo { rate, delay }) = evl.key_repeat_info {
                assert!(!xkb_context.keymap.is_null());
                if unsafe { (xkb_context.lib.xkb_keymap_key_repeats)(xkb_context.keymap, key) } == 1
                {
                    evl.key_repeat = Some((scancode, keycode));
                    if let Err(err) = unsafe { evl.key_repeat_timerfd.arm(rate, delay) } {
                        log::error!("could not arm key repeat: {err}");
                    }
                }
            }
        }
        libwayland_client::WL_KEYBOARD_KEY_STATE_RELEASED => {
            let keyboard_event = KeyboardEvent::Release { scancode, keycode };
            evl.events.push_back(Event::Keyboard(keyboard_event));

            evl.key_repeat = None;
            if let Err(err) = unsafe { evl.key_repeat_timerfd.disarm() } {
                log::error!("could not disarm key repeat: {err}");
            }
        }
        libwayland_client::WL_KEYBOARD_KEY_STATE_REPEATED => {
            // NOTE: key repetition is handled with repeat info timer ^.
        }
        other => log::warn!("unknown keyboard key state: {other}"),
    }
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

unsafe extern "C" fn handle_wl_keyboard_repeat_info(
    data: *mut c_void,
    _wl_keyboard: *mut libwayland_client::wl_keyboard,
    rate: i32,
    delay: i32,
) {
    // QUOTE: negative values for either rate or delay are illegal.
    assert!(rate >= 0 && delay >= 0);

    let evl = unsafe { &mut *(data as *mut WaylandBackend) };
    // NOTE: a rate of zero disables any repeating, regardless of the delay's value.
    evl.key_repeat_info = if rate == 0 {
        None
    } else {
        Some(KeyRepeatInfo {
            // QUOTE: rate of repeating keys in characters per second
            rate: Duration::from_nanos(1_000_000_000 / rate as u64),
            // QUOTE: delay in milliseconds since key down until repeating starts
            delay: Duration::from_millis(delay as u64),
        })
    };
}

const WL_KEYBOARD_LISTENER: libwayland_client::wl_keyboard_listener =
    libwayland_client::wl_keyboard_listener {
        keymap: handle_wl_keyboard_keymap,
        enter: handle_wl_keyboard_enter,
        leave: handle_wl_keyboard_leave,
        key: handle_wl_keyboard_key,
        modifiers: handle_wl_keyboard_modifiers,
        repeat_info: handle_wl_keyboard_repeat_info,
    };

impl WaylandBackend {
    pub fn new_boxed(attrs: WindowAttrs) -> anyhow::Result<Box<Self>> {
        let libwayland_client = libwayland_client::Lib::load()?;

        let wl_display =
            NonNull::new(unsafe { (libwayland_client.wl_display_connect)(null_mut()) })
                .context("could not connect to wayland display")?;

        let Ok(key_repeat_timerfd) = (unsafe {
            TimerFD::new(
                libc::CLOCK_MONOTONIC,
                libc::TFD_CLOEXEC | libc::TFD_NONBLOCK,
            )
        }) else {
            unsafe { (libwayland_client.wl_display_disconnect)(wl_display.as_ptr()) };

            return Err(anyhow!("could not create key repeat timer fd"));
        };

        let mut boxed = Box::new(WaylandBackend {
            libwayland_client,

            wl_display,

            wl_compositor: null_mut(),
            wl_seat: null_mut(),
            wl_shm: null_mut(),
            wp_cursor_shape_manager_v1: null_mut(),
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

            wl_pointer: null_mut(),
            pointer_frame_events: VecDeque::new(),
            cursor: None,
            cursor_shape: None,
            wp_cursor_shape_device_v1: null_mut(),

            wl_keyboard: null_mut(),
            xkb_context: None,
            key_repeat_timerfd,
            key_repeat_info: None,
            key_repeat: None,

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

        if !boxed.wp_cursor_shape_manager_v1.is_null() {
            assert!(!boxed.wl_pointer.is_null());
            boxed.wp_cursor_shape_device_v1 = unsafe {
                libwayland_client::wp_cursor_shape_manager_v1_get_pointer(
                    &boxed.libwayland_client,
                    boxed.wp_cursor_shape_manager_v1,
                    boxed.wl_pointer,
                )
            };
            if boxed.wp_cursor_shape_device_v1.is_null() {
                return Err(anyhow!("could not get cursor shape device"));
            }
        } else {
            boxed.cursor = Cursor::init(
                &boxed.libwayland_client,
                boxed.wl_compositor,
                boxed.wl_shm,
                boxed.scale_factor(),
            )
            .map(Some)
            .context("could not init cursor")?;
        }

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

    fn maybe_resize(&mut self, logical_size: Option<(u32, u32)>, scale_factor: Option<f64>) {
        assert!(logical_size.is_some() || scale_factor.is_some());

        let mut logical_size_changed = false;
        if let Some(logical_size) = logical_size {
            logical_size_changed = self.logical_size != Some(logical_size);
            if logical_size_changed {
                self.logical_size = Some(logical_size);

                if !self.wp_viewporter.is_null() {
                    unsafe {
                        libwayland_client::wp_viewport_set_destination(
                            &self.libwayland_client,
                            self.wp_viewport,
                            logical_size.0 as i32,
                            logical_size.1 as i32,
                        )
                    };
                }
            }
        }

        let mut scale_factor_changed = false;
        if let Some(scale_factor) = scale_factor {
            scale_factor_changed = self.scale_factor != Some(scale_factor);
            if scale_factor_changed {
                self.scale_factor = Some(scale_factor);

                // NOTE: if we're using old cursor stuff (not wp_cursor_shape_manager_v1) - cursor
                // needs to be re-scaled.
                if let Some(ref mut cursor) = self.cursor {
                    match cursor.set_scale(self.wl_shm, scale_factor) {
                        Err(err) => {
                            log::error!("could not set cursor scale (during rescale): {err:?}");
                        }
                        Ok(_) => {
                            // NOTE: cursor needs to be updated after re-scaling.
                            let shape = self.cursor_shape.unwrap_or(CursorShape::Default);
                            if let Err(err) = self.set_cursor_shape(shape) {
                                log::error!("could not set cursor shape (during rescale): {err:?}");
                            }
                        }
                    }
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
        // https://wayland.freedesktop.org/docs/html/apb.html#Client-classwl__display_1a40039c1169b153269a3dc0796a54ddb0
        // https://gitlab.freedesktop.org/wayland/weston/-/blob/5a48cedc7b8421d8342dd6a943705955217b0fd1/clients/window.c#L7180

        let client = &self.libwayland_client;
        let display = self.wl_display.as_ptr();

        // QUOTE: returns 0 on success or -1 if event queue was not empty
        while unsafe { (client.wl_display_prepare_read)(display) } == -1 {
            let ret = unsafe { (client.wl_display_dispatch_pending)(display) };
            if ret == -1 {
                return Err(anyhow!("wl_display_dispatch_pending failed"));
            }
        }

        let ret = unsafe { (client.wl_display_flush)(display) };
        if ret == -1 {
            // TODO: handle wl_display_flush's EAGAIN errno
            return Err(anyhow!("wl_display_flush failed"));
        }

        let mut fds = [
            libc::pollfd {
                fd: unsafe { (client.wl_display_get_fd)(display) },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: self.key_repeat_timerfd.0,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        // QUOTE: If the value of timeout is 0, poll() shall return immediately. If the value of
        // timeout is -1, poll() shall block until a requested event occurs or until the call is
        // interrupted.
        let ret = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 0) };
        match ret {
            -1 => {
                unsafe { (client.wl_display_cancel_read)(display) };
                let errno = unsafe { *libc::__errno_location() };
                return Err(anyhow!("could not poll fds: 0x:{errno:x}"));
            }
            0 => {
                unsafe { (client.wl_display_cancel_read)(display) };
            }
            1.. => {
                if fds[0].revents & libc::POLLIN == libc::POLLIN {
                    let ret = unsafe { (client.wl_display_read_events)(display) };
                    if ret == -1 {
                        return Err(anyhow!("wl_display_read_events failed"));
                    }

                    // TODO: is this dispatch really needed here?
                    let ret = unsafe { (client.wl_display_dispatch_pending)(display) };
                    if ret == -1 {
                        return Err(anyhow!("wl_display_dispatch_pending failed"));
                    }
                } else {
                    unsafe { (client.wl_display_cancel_read)(display) };
                }

                if fds[1].revents & libc::POLLIN == libc::POLLIN {
                    if let Some((scancode, keycode)) = self.key_repeat {
                        let exp: u64 = unsafe { self.key_repeat_timerfd.read() }?;
                        for _ in 0..exp {
                            let keyboard_event = KeyboardEvent::Press {
                                scancode,
                                keycode,
                                repeat: true,
                            };
                            self.events.push_back(Event::Keyboard(keyboard_event));
                        }
                    }
                }
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    fn pop_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    fn set_cursor_shape(&mut self, shape: CursorShape) -> anyhow::Result<()> {
        // NOTE: immediate mode ui and shit can may want to set cursor every frame, and most of the
        // time it would be the same (which would not constitute a change).
        //
        // TODO: i am not 100% sure i really need this check here, but it wouldn't hurt i guess?
        if self.cursor_shape == Some(shape) {
            return Ok(());
        }

        let Some(serial) = self.serial_tracker.get_serial(SerialType::PointerEnter) else {
            log::warn!("no pointer enter serial found");
            return Ok(());
        };

        if !self.wp_cursor_shape_device_v1.is_null() {
            unsafe {
                libwayland_client::wp_cursor_shape_device_v1_set_shape(
                    &self.libwayland_client,
                    self.wp_cursor_shape_device_v1,
                    serial,
                    map_cursor_shape_to_enum(shape),
                )
            };
        } else if let Some(ref cursor) = self.cursor {
            cursor.set_shape(
                &self.libwayland_client,
                self.wl_pointer,
                map_cursor_shape_to_name(shape),
                serial,
            )?;
        } else {
            return Err(anyhow!(
                "cursor shape protocol is unavailable and libwayland_cursor thing is uninitialized (why?)"
            ));
        }

        self.cursor_shape = Some(shape);
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
