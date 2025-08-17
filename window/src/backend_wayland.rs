use std::collections::VecDeque;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::hash::{Hash, Hasher};
use std::io::{PipeReader, PipeWriter, Read as _};
use std::mem::{self, MaybeUninit};
use std::os::fd::FromRawFd as _;
use std::ptr::{null, null_mut, NonNull};
use std::slice;
use std::time::Duration;

use anyhow::{anyhow, Context as _};
use input::{
    Button, ButtonState, CursorShape, GesturePhase, KeyState, KeyboardEvent, Keycode, PointerEvent,
    RawKey, Scancode,
};
use nohash::{NoHash, NoHashMap};
use raw_window_handle as rwh;

use crate::{
    xkb, ClipboardDataProvider, Event, Window, WindowAttrs, WindowEvent, DEFAULT_LOGICAL_SIZE,
};

// TODO: (xd) consider checking return of wl_proxy_add_listener (xd).

// TODO: can't paste data copied from this event loop into other app when this app is not visible.
// the issue is in eglSwapBuffers.
// the solution might lie in frame callback which would allow to communicate when to perform a
// draw.
// also note that eglSwapBuffers will stop being and issue with eglSwapInterval of 0.

unsafe extern "C" fn noop_listener() {}
const NOOP_LISTENER: unsafe extern "C" fn() = noop_listener;
macro_rules! noop_listener {
    () => {
        unsafe {
            #[expect(clippy::missing_transmute_annotations)]
            mem::transmute(NOOP_LISTENER)
        }
    };
}

// https://github.com/torvalds/linux/blob/231825b2e1ff6ba799c5eaf396d3ab2354e37c6b/include/uapi/linux/input-event-codes.h#L356
#[inline]
fn try_map_pointer_button(button: u32) -> Option<Button> {
    match button {
        0x110 => Some(Button::Primary),
        0x111 => Some(Button::Secondary),
        0x112 => Some(Button::Tertiary),
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
fn get_cursor_rounded_scale(scale_factor: f64) -> u32 {
    (scale_factor - CURSOR_SCALE_FLOORING_THRESHOLD).ceil() as u32
}

fn load_cursor_theme(
    libwayland_cursor: &wayland::CursorApi,
    wl_shm: *mut wayland::wl_shm,
    scale_factor: f64,
) -> anyhow::Result<*mut wayland::wl_cursor_theme> {
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
        CursorShape::Text => c"text",
        CursorShape::Crosshair => c"crosshair",
        CursorShape::Move => c"move",
        CursorShape::NwResize => c"nw-resize",
        CursorShape::NeResize => c"ne-resize",
        CursorShape::SeResize => c"se-resize",
        CursorShape::SwResize => c"sw-resize",
    }
}

fn map_cursor_shape_to_enum(shape: CursorShape) -> u32 {
    match shape {
        CursorShape::Default => wayland::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_DEFAULT,
        CursorShape::Pointer => wayland::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_POINTER,
        CursorShape::Text => wayland::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_TEXT,
        CursorShape::Crosshair => wayland::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_CROSSHAIR,
        CursorShape::Move => wayland::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_MOVE,
        CursorShape::NwResize => wayland::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_NW_RESIZE,
        CursorShape::NeResize => wayland::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_NE_RESIZE,
        CursorShape::SeResize => wayland::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_SE_RESIZE,
        CursorShape::SwResize => wayland::WP_CURSOR_SHAPE_DEVICE_V1_SHAPE_SW_RESIZE,
    }
}

struct Cursor {
    libwayland_cursor: wayland::CursorApi,
    wl_cursor_theme: *mut wayland::wl_cursor_theme,
    wl_surface: *mut wayland::wl_surface,
    scale: f64,
}

impl Cursor {
    fn init(
        libwayland_client: &wayland::ClientApi,
        wl_compositor: *mut wayland::wl_compositor,
        wl_shm: *mut wayland::wl_shm,
        scale: f64,
    ) -> anyhow::Result<Self> {
        assert!(!wl_compositor.is_null());

        let libwayland_cursor = wayland::CursorApi::load()?;

        let wl_surface =
            unsafe { wayland::wl_compositor_create_surface(libwayland_client, wl_compositor) };
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
    fn deinit(self, libwayland_client: &wayland::ClientApi) {
        if !self.wl_surface.is_null() {
            unsafe { wayland::wl_surface_destroy(libwayland_client, self.wl_surface) };
        }
        if !self.wl_cursor_theme.is_null() {
            unsafe { (self.libwayland_cursor.wl_cursor_theme_destroy)(self.wl_cursor_theme) };
        }
    }

    /// NOTE: should call set_shape right after. this function will not update shape to scale.
    fn set_scale(&mut self, wl_shm: *mut wayland::wl_shm, scale: f64) -> anyhow::Result<()> {
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
        libwayland_client: &wayland::ClientApi,
        wl_pointer: *mut wayland::wl_pointer,
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
            wayland::wl_surface_set_buffer_scale(
                libwayland_client,
                self.wl_surface,
                rounded_scale as i32,
            );

            wayland::wl_surface_attach(libwayland_client, self.wl_surface, image_buffer, 0, 0);

            // NOTE: pre version 4 wl_surface::damage must be used instead.
            let wl_surface_version =
                (libwayland_client.wl_proxy_get_version)(self.wl_surface as *mut wayland::wl_proxy);
            assert!(wl_surface_version >= 4);

            wayland::wl_surface_damage_buffer(
                libwayland_client,
                self.wl_surface,
                0,
                0,
                image.width as i32,
                image.height as i32,
            );

            wayland::wl_pointer_set_cursor(
                libwayland_client,
                wl_pointer,
                serial,
                self.wl_surface,
                hotspot_x,
                hotspot_y,
            );

            wayland::wl_surface_commit(libwayland_client, self.wl_surface);
        }

        Ok(())
    }
}

// https://github.com/torvalds/linux/blob/231825b2e1ff6ba799c5eaf396d3ab2354e37c6b/include/uapi/linux/input-event-codes.h#L76
#[inline]
fn map_keyboard_key(key: u32) -> Scancode {
    match key {
        0 => Scancode::Reserved,
        1 => Scancode::Esc,
        2 => Scancode::Num1,
        3 => Scancode::Num2,
        4 => Scancode::Num3,
        5 => Scancode::Num4,
        6 => Scancode::Num5,
        7 => Scancode::Num6,
        8 => Scancode::Num7,
        9 => Scancode::Num8,
        10 => Scancode::Num9,
        11 => Scancode::Num0,
        12 => Scancode::Minus,
        13 => Scancode::Equal,
        14 => Scancode::Backspace,
        15 => Scancode::Tab,
        16 => Scancode::Q,
        17 => Scancode::W,
        18 => Scancode::E,
        19 => Scancode::R,
        20 => Scancode::T,
        21 => Scancode::Y,
        22 => Scancode::U,
        23 => Scancode::I,
        24 => Scancode::O,
        25 => Scancode::P,
        26 => Scancode::BraceLeft,
        27 => Scancode::BraceRight,
        28 => Scancode::Enter,
        29 => Scancode::CtrlLeft,
        30 => Scancode::A,
        31 => Scancode::S,
        32 => Scancode::D,
        33 => Scancode::F,
        34 => Scancode::G,
        35 => Scancode::H,
        36 => Scancode::J,
        37 => Scancode::K,
        38 => Scancode::L,
        39 => Scancode::Semicolon,
        40 => Scancode::Apostrophe,
        41 => Scancode::Grave,
        42 => Scancode::ShiftLeft,
        43 => Scancode::Backslash,
        44 => Scancode::Z,
        45 => Scancode::X,
        46 => Scancode::C,
        47 => Scancode::V,
        48 => Scancode::B,
        49 => Scancode::N,
        50 => Scancode::M,
        51 => Scancode::Comma,
        52 => Scancode::Dot,
        53 => Scancode::Slash,
        54 => Scancode::ShiftRight,
        // 55 => Scancode::KPAsterisk,
        56 => Scancode::AltLeft,
        57 => Scancode::Space,
        58 => Scancode::CapsLock,
        // 59 => Scancode::F1,
        // 60 => Scancode::F2,
        // 61 => Scancode::F3,
        // 62 => Scancode::F4,
        // 63 => Scancode::F5,
        // 64 => Scancode::F6,
        // 65 => Scancode::F7,
        // 66 => Scancode::F8,
        // 67 => Scancode::F9,
        // 68 => Scancode::F10,
        69 => Scancode::NumLock,
        70 => Scancode::ScrollLock,
        // 71 => Scancode::KP7,
        // 72 => Scancode::KP8,
        // 73 => Scancode::KP9,
        // 74 => Scancode::KPMinus,
        // 75 => Scancode::KP4,
        // 76 => Scancode::KP5,
        // 77 => Scancode::KP6,
        // 78 => Scancode::KPPlus,
        // 79 => Scancode::KP1,
        // 80 => Scancode::KP2,
        // 81 => Scancode::KP3,
        // 82 => Scancode::KP0,
        // 83 => Scancode::KPDOT,
        // 84 => Scancode::_,
        // 85 => Scancode::ZENKAKUHANKAKU,
        // 86 => Scancode::102ND,
        // 87 => Scancode::F11,
        // 88 => Scancode::F12,
        // 89 => Scancode::RO,
        // 90 => Scancode::KATAKANA,
        // 91 => Scancode::HIRAGANA,
        // 92 => Scancode::HENKAN,
        // 93 => Scancode::KATAKANAHIRAGANA,
        // 94 => Scancode::MUHENKAN,
        // 95 => Scancode::KPJPCOMMA,
        // 96 => Scancode::KPEnter,
        97 => Scancode::CtrlRight,
        // 98 => Scancode::KPSlash,
        // 99 => Scancode::SYSRQ,
        100 => Scancode::AltRight,
        // 101 => Scancode::LINEFEED,
        102 => Scancode::Home,
        103 => Scancode::ArrowUp,
        104 => Scancode::PageUp,
        105 => Scancode::ArrowLeft,
        106 => Scancode::ArrowRight,
        107 => Scancode::End,
        108 => Scancode::ArrowDown,
        109 => Scancode::PageDown,
        110 => Scancode::Insert,
        111 => Scancode::Delete,
        // 112 => Scancode::MACRO,
        // 113 => Scancode::MUTE,
        // 114 => Scancode::VOLUMEDOWN,
        // 115 => Scancode::VOLUMEUP,
        // 116 => Scancode::POWER,
        // 117 => Scancode::KPEqual,
        // 118 => Scancode::KPPLUSMINUS,
        // 119 => Scancode::PAUSE,
        // 120 => Scancode::SCALE,
        other => Scancode::Unidentified(RawKey::Unix(other)),
    }
}

/// > Offset between evdev keycodes (where KEY_ESCAPE is 1), and the evdev XKB keycode set (where
/// ESC is 9). */
/// - https://github.com/xkbcommon/libxkbcommon/pull/359
/// - https://github.com/xkbcommon/libxkbcommon/blob/eb0a1457f4ada160d03f6d938fa31f6b049cb403/doc/keymap-format-text-v1.md
const EVDEV_OFFSET: u32 = 8;

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum SerialType {
    PointerEnter,
    KeyboardEnter,
}

impl Hash for SerialType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u8(*self as u8);
    }
}

impl NoHash for SerialType {}

#[derive(Default)]
struct SerialTracker {
    map: NoHashMap<SerialType, u32>,
}

impl SerialTracker {
    fn update_serial(&mut self, ty: SerialType, serial: u32) {
        self.map.insert(ty, serial);
    }

    fn reset_serial(&mut self, ty: SerialType) {
        self.map.remove(&ty);
    }

    fn get_serial(&self, ty: SerialType) -> Option<u32> {
        self.map.get(&ty).cloned()
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

// NOTE: i use re-use this for converting &str into *const c_char when calling ffi funcs.
// better solution prob would be to have a proper temp allocator for this and any other kinds of
// stuff.
struct TempCStr {
    buf: Vec<u8>,
}

impl TempCStr {
    fn new_with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
        }
    }

    fn from_str(&mut self, str: &str) -> &CStr {
        assert!(self.buf.is_empty());
        self.buf.extend_from_slice(str.as_bytes());
        self.buf.push(0);
        unsafe { CStr::from_bytes_with_nul_unchecked(self.buf.as_ref()) }
    }

    fn clear(&mut self) {
        self.buf.clear()
    }
}

// NOTE: this does not support multiple surfaces.
pub struct WaylandBackend {
    libwayland_client: wayland::ClientApi,
    wl_display: NonNull<wayland::wl_display>,

    // interfaces
    wl_compositor: *mut wayland::wl_compositor,
    wl_data_device_manager: *mut wayland::wl_data_device_manager,
    wl_seat: *mut wayland::wl_seat,
    wl_shm: *mut wayland::wl_shm,
    wp_cursor_shape_manager_v1: *mut wayland::wp_cursor_shape_manager_v1,
    wp_fractional_scale_manager_v1: *mut wayland::wp_fractional_scale_manager_v1,
    wp_viewporter: *mut wayland::wp_viewporter,
    xdg_wm_base: *mut wayland::xdg_wm_base,
    zwp_pointer_gestures_v1: *mut wayland::zwp_pointer_gestures_v1,

    // window
    attrs: WindowAttrs,
    wl_surface: *mut wayland::wl_surface,
    xdg_surface: *mut wayland::xdg_surface,
    xdg_toplevel: *mut wayland::xdg_toplevel,
    acked_first_xdg_surface_ack_configure: bool,

    // dpi
    wp_fractional_scale_v1: *mut wayland::wp_fractional_scale_v1,
    wp_viewport: *mut wayland::wp_viewport,
    logical_size: Option<(u32, u32)>,
    scale_factor: Option<f64>,

    // pointer
    wl_pointer: *mut wayland::wl_pointer,
    // NOTE: cursor_shape is stored here so that it can be set back to what was requested when
    // pointer re-enders the surface.
    cursor_shape: Option<CursorShape>,
    wp_cursor_shape_device_v1: *mut wayland::wp_cursor_shape_device_v1,
    cursor: Option<Cursor>,
    // NOTE: only one of `axis_discrete`, `axis_value120` and `axis` values will be used if any is
    // present.
    // index 0 is vertical scroll (wayland::WL_POINTER_AXIS_VERTICAL_SCROLL),
    // 1 - horizontal (WL_POINTER_AXIS_HORIZONTAL_SCROLL).
    axis: Option<[wayland::wl_fixed; 2]>,
    axis_discrete: Option<[i32; 2]>,
    axis_value120: Option<[i32; 2]>,
    zwp_pointer_gesture_swipe_v1: *mut wayland::zwp_pointer_gesture_swipe_v1,
    swipe_fingers: Option<u8>,
    zwp_pointer_gesture_pinch_v1: *mut wayland::zwp_pointer_gesture_pinch_v1,
    // NOTE: pinch_scale is set to Some on begin event and back to None on end event.
    // wayland (libinput) reports scale relative to the initial finger position. we want to report
    // delta.
    pinch_scale: Option<f64>,
    pinch_fingers: Option<u8>,

    // keyboard
    wl_keyboard: *mut wayland::wl_keyboard,
    xkb_context: Option<xkb::Context>,
    key_repeat_timerfd: TimerFD,
    key_repeat_info: Option<KeyRepeatInfo>,
    key_repeat: Option<(Scancode, Keycode)>,

    // clipboard
    wl_data_device: *mut wayland::wl_data_device,
    wl_data_offer: *mut wayland::wl_data_offer,
    // NOTE: on cancel this needs to be cleaned up and destroyed.
    clipboard_data: Option<(Box<dyn ClipboardDataProvider>, *mut wayland::wl_data_source)>,

    serial_tracker: SerialTracker,
    events: VecDeque<Event>,

    // NOTE: temp_cstr is used for temporary allocations. hand-offs.
    temp_cstr: TempCStr,
}

unsafe extern "C" fn handle_wl_registry_global(
    data: *mut c_void,
    wl_registry: *mut wayland::wl_registry,
    name: u32,
    interface: *const c_char,
    version: u32,
) {
    unsafe {
        let this = &mut *(data as *mut WaylandBackend);

        let interface = CStr::from_ptr(interface)
            .to_str()
            .expect("invalid interface string");

        match interface {
            "wl_compositor" => {
                this.wl_compositor = wayland::wl_registry_bind(
                    &this.libwayland_client,
                    wl_registry,
                    name,
                    &wayland::wl_compositor_interface,
                    6.min(version),
                ) as _;
            }
            "wl_data_device_manager" => {
                this.wl_data_device_manager = wayland::wl_registry_bind(
                    &this.libwayland_client,
                    wl_registry,
                    name,
                    &wayland::wl_data_device_manager_interface,
                    3.min(version),
                ) as _;
            }
            "wl_seat" => {
                this.wl_seat = wayland::wl_registry_bind(
                    &this.libwayland_client,
                    wl_registry,
                    name,
                    &wayland::wl_seat_interface,
                    9.min(version),
                ) as _;
            }
            "wl_shm" => {
                this.wl_shm = wayland::wl_registry_bind(
                    &this.libwayland_client,
                    wl_registry,
                    name,
                    &wayland::wl_shm_interface,
                    2.min(version),
                ) as _;
            }
            "wp_cursor_shape_manager_v1" => {
                this.wp_cursor_shape_manager_v1 = wayland::wl_registry_bind(
                    &this.libwayland_client,
                    wl_registry,
                    name,
                    &wayland::wp_cursor_shape_manager_v1_interface,
                    1.min(version),
                ) as _;
            }
            "wp_fractional_scale_manager_v1" => {
                this.wp_fractional_scale_manager_v1 = wayland::wl_registry_bind(
                    &this.libwayland_client,
                    wl_registry,
                    name,
                    &wayland::wp_fractional_scale_manager_v1_interface,
                    1.min(version),
                ) as _;
            }
            "wp_viewporter" => {
                this.wp_viewporter = wayland::wl_registry_bind(
                    &this.libwayland_client,
                    wl_registry,
                    name,
                    &wayland::wp_viewporter_interface,
                    1.min(version),
                ) as _;
            }
            "xdg_wm_base" => {
                this.xdg_wm_base = wayland::wl_registry_bind(
                    &this.libwayland_client,
                    wl_registry,
                    name,
                    &wayland::xdg_wm_base_interface,
                    6.min(version),
                ) as _;
            }
            "zwp_pointer_gestures_v1" => {
                this.zwp_pointer_gestures_v1 = wayland::wl_registry_bind(
                    &this.libwayland_client,
                    wl_registry,
                    name,
                    &wayland::zwp_pointer_gestures_v1_interface,
                    3.min(version),
                ) as _;
            }
            _ => {
                log::debug!("unused interface: {interface}");
            }
        }
    }
}

const WL_REGISTRY_LISTENER: wayland::wl_registry_listener = wayland::wl_registry_listener {
    global: handle_wl_registry_global,
    global_remove: noop_listener!(),
};

unsafe extern "C" fn handle_wl_seat_capabilities(
    data: *mut c_void,
    _wl_seat: *mut wayland::wl_seat,
    capabilities: u32,
) {
    log::debug!("recv wl_seat_capabilities (capabilities: {capabilities})");

    let _this = unsafe { &mut *(data as *mut WaylandBackend) };

    if capabilities & wayland::WL_SEAT_CAPABILITY_POINTER == wayland::WL_SEAT_CAPABILITY_POINTER {
        // TODO: init pointer from here
    }

    if capabilities & wayland::WL_SEAT_CAPABILITY_KEYBOARD == wayland::WL_SEAT_CAPABILITY_KEYBOARD {
        // TODO: init keyboard from here
    }

    // NOTE: there's also a touch capability.
}

const WL_SEAT_LISTENER: wayland::wl_seat_listener = wayland::wl_seat_listener {
    capabilities: handle_wl_seat_capabilities,
    name: noop_listener!(),
};

unsafe extern "C" fn handle_xdg_wm_base_ping(
    data: *mut c_void,
    xdg_wm_base: *mut wayland::xdg_wm_base,
    serial: u32,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    unsafe { wayland::xdg_wm_base_pong(&this.libwayland_client, xdg_wm_base, serial) };
}

const XDG_WM_BASE_LISTENER: wayland::xdg_wm_base_listener = wayland::xdg_wm_base_listener {
    ping: handle_xdg_wm_base_ping,
};

unsafe extern "C" fn handle_xdg_surface_configure(
    data: *mut c_void,
    xdg_surface: *mut wayland::xdg_surface,
    serial: u32,
) {
    log::debug!("recv xdg_surface_configure");

    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    unsafe { wayland::xdg_surface_ack_configure(&this.libwayland_client, xdg_surface, serial) };
    this.acked_first_xdg_surface_ack_configure = true;
}

const XDG_SURFACE_LISTENER: wayland::xdg_surface_listener = wayland::xdg_surface_listener {
    configure: handle_xdg_surface_configure,
};

unsafe extern "C" fn handle_xdg_toplevel_configure(
    data: *mut c_void,
    _xdg_toplevel: *mut wayland::xdg_toplevel,
    width: i32,
    height: i32,
    _states: *mut wayland::wl_array,
) {
    log::debug!("recv xdg_toplevel_configure");

    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    // NOTE: if the width or height arguments are zero, it means the client should decide its own
    // window dimension.
    assert!(width >= 0 && height >= 0);
    let logical_size = (width > 0 || height > 0)
        .then_some((width as u32, height as u32))
        .or(this.logical_size)
        .unwrap_or(DEFAULT_LOGICAL_SIZE);

    this.maybe_resize(Some(logical_size), None);
}

unsafe extern "C" fn handle_xdg_toplevel_close(
    data: *mut c_void,
    _xdg_toplevel: *mut wayland::xdg_toplevel,
) {
    log::debug!("recv xdg_toplevel_close");

    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    this.events
        .push_back(Event::Window(WindowEvent::CloseRequested));
}

const XDG_TOPLEVEL_LISTENER: wayland::xdg_toplevel_listener = wayland::xdg_toplevel_listener {
    configure: handle_xdg_toplevel_configure,
    close: handle_xdg_toplevel_close,
    wm_capabilities: noop_listener!(),
    configure_bounds: noop_listener!(),
};

unsafe extern "C" fn handle_wp_fractional_scale_v1_preferred_scale(
    data: *mut c_void,
    _wp_fractional_scale_v1: *mut wayland::wp_fractional_scale_v1,
    scale: u32,
) {
    log::debug!("recv wp_fractional_scale_v1_preferred_scale");

    // > The sent scale is the numerator of a fraction with a denominator of 120.
    let scale_factor = scale as f64 / 120.0;

    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    this.maybe_resize(None, Some(scale_factor));
}

const WP_FRACTIONAL_SCALE_MANAGER_V1_LISTENER: wayland::wp_fractional_scale_v1_listener =
    wayland::wp_fractional_scale_v1_listener {
        preferred_scale: handle_wp_fractional_scale_v1_preferred_scale,
    };

unsafe extern "C" fn handle_wl_pointer_enter(
    data: *mut c_void,
    _wl_pointer: *mut wayland::wl_pointer,
    serial: u32,
    _surface: *mut wayland::wl_surface,
    surface_x: wayland::wl_fixed,
    surface_y: wayland::wl_fixed,
) {
    log::debug!("recv wl_pointer_enter");

    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    this.serial_tracker
        .update_serial(SerialType::PointerEnter, serial);

    let cursor_shape = this.cursor_shape.unwrap_or(CursorShape::Default);
    if let Err(err) = this.set_cursor_shape(cursor_shape) {
        log::error!("could not set cursor shape (pointer enter): {err:?}");
    }

    let position = (
        wayland::wl_fixed_to_f64(surface_x),
        wayland::wl_fixed_to_f64(surface_y),
    );
    this.events.push_back(Event::Pointer(PointerEvent::Enter {
        position: Some(position),
    }));
}

unsafe extern "C" fn handle_wl_pointer_leave(
    data: *mut c_void,
    _wl_pointer: *mut wayland::wl_pointer,
    _serial: u32,
    _surface: *mut wayland::wl_surface,
) {
    log::debug!("recv wl_pointer_leave");

    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    this.serial_tracker.reset_serial(SerialType::PointerEnter);
    this.events.push_back(Event::Pointer(PointerEvent::Leave));
}

unsafe extern "C" fn handle_wl_pointer_motion(
    data: *mut c_void,
    _wl_pointer: *mut wayland::wl_pointer,
    _time: u32,
    surface_x: wayland::wl_fixed,
    surface_y: wayland::wl_fixed,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    let position = (
        wayland::wl_fixed_to_f64(surface_x),
        wayland::wl_fixed_to_f64(surface_y),
    );
    // TODO: multiple motion events per frame can be sent. should they be accumulated? probably
    // not?
    this.events
        .push_back(Event::Pointer(PointerEvent::Move { position }));
}

unsafe extern "C" fn handle_wl_pointer_button(
    data: *mut c_void,
    _wl_pointer: *mut wayland::wl_pointer,
    _serial: u32,
    _time: u32,
    button: u32,
    state: u32,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    let Some(button) = try_map_pointer_button(button) else {
        log::warn!("unidentified pointer button: {button}");
        return;
    };
    let state = match state {
        wayland::WL_POINTER_BUTTON_STATE_PRESSED => ButtonState::Pressed,
        wayland::WL_POINTER_BUTTON_STATE_RELEASED => ButtonState::Released,
        other => {
            log::warn!("unknown pointer button state: {other}");
            return;
        }
    };
    this.events
        .push_back(Event::Pointer(PointerEvent::Button { state, button }));
}

unsafe extern "C" fn handle_wl_pointer_axis(
    data: *mut c_void,
    _wl_pointer: *mut wayland::wl_pointer,
    _time: u32,
    axis: u32,
    value: wayland::wl_fixed,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    let dst = this.axis.get_or_insert([0, 0]);
    // NOTE: the spec dos not state that only one axis event may occur per frame, thus
    // accumulating.
    dst[axis as usize] += value;
}

unsafe extern "C" fn handle_wl_pointer_frame(
    data: *mut c_void,
    _wl_pointer: *mut wayland::wl_pointer,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    let axis = this.axis.take();
    let axis_value120 = this.axis_value120.take();
    let axis_discrete = this.axis_discrete.take();
    // NOTE: the order is important. if we have received axis_value120 - ignore others, and so on.
    let scroll_delta: Option<(f64, f64)> = if let Some([y, x]) = axis_value120 {
        const DENOM: f64 = 120.0;
        Some((x as f64 / DENOM, y as f64 / DENOM))
    } else if let Some([y, x]) = axis_discrete {
        Some((x as f64, y as f64))
    } else if let Some([y, x]) = axis {
        // NOTE: the axis value is specified in logical surface coordinate space. most compositors
        // use either 10 (gnome, weston) or 15 (wlroots, same as libinput) as the value.
        //
        // chrome uses 10 (kAxisValueScale in wayland_pointer.cc)
        // sdl uses 10 (WAYLAND_WHEEL_AXIS_UNIT in SDL_waylandevents.c).
        //
        // TODO: since this is logical coords - do i need to apply fractional scaling here? would
        // it make sense to do that? noobody seems to be doing that though.
        const SCALE: f64 = 10.0;
        Some((
            wayland::wl_fixed_to_f64(x) / SCALE,
            wayland::wl_fixed_to_f64(y) / SCALE,
        ))
    } else {
        None
    };
    if let Some(delta) = scroll_delta {
        this.events
            .push_back(Event::Pointer(PointerEvent::Scroll { delta }));
    }
}

// NOTE: wl_pointer_axis_discrete event is deprecated since v8 (and i am seeing wl_seat being v9).
// but i don't see why not to support it. easy enough.
unsafe extern "C" fn handle_wl_pointer_axis_discrete(
    data: *mut c_void,
    _wl_pointer: *mut wayland::wl_pointer,
    axis: u32,
    discrete: i32,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    let dst = this.axis_discrete.get_or_insert([0, 0]);
    // QUOTE: A wl_pointer.frame must not contain more than one axis_discrete event per axis type.
    assert_eq!(dst[axis as usize], 0);
    dst[axis as usize] = discrete;
}

unsafe extern "C" fn handle_wl_pointer_axis_value120(
    data: *mut c_void,
    _wl_pointer: *mut wayland::wl_pointer,
    axis: u32,
    value120: i32,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    let dst = this.axis_value120.get_or_insert([0, 0]);
    // NOTE: i don't see the spec mentioning that only one axis_value120 event may occur per frame,
    // thus accumulating.
    dst[axis as usize] += value120;
}

const WL_POINTER_LISTENER: wayland::wl_pointer_listener = wayland::wl_pointer_listener {
    enter: handle_wl_pointer_enter,
    leave: handle_wl_pointer_leave,
    motion: handle_wl_pointer_motion,
    button: handle_wl_pointer_button,
    axis: handle_wl_pointer_axis,
    frame: handle_wl_pointer_frame,
    axis_source: noop_listener!(),
    axis_stop: noop_listener!(),
    axis_discrete: handle_wl_pointer_axis_discrete,
    axis_value120: handle_wl_pointer_axis_value120,
    axis_relative_direction: noop_listener!(),
};

unsafe extern "C" fn handle_zwp_pointer_gesture_swipe_v1_begin(
    data: *mut c_void,
    _zwp_pointer_gesture_swipe_v1: *mut wayland::zwp_pointer_gesture_swipe_v1,
    _serial: u32,
    _time: u32,
    _surface: *mut wayland::wl_surface,
    fingers: u32,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    // QUOTE:
    // > swipe gestures are executed when three or more fingers are moved synchronously in the same
    // direction.
    // - https://wayland.freedesktop.org/libinput/doc/latest/gestures.html#swipe-gestures
    assert!(fingers >= 3);
    assert!(fingers <= u8::MAX as u32);
    let fingers = fingers as u8;
    // NOTE: swipe fingers must be unset on end.
    assert!(this.swipe_fingers.is_none());
    this.swipe_fingers = Some(fingers);

    // TODO: consider introducing a `Possible` GesturePhase variant and not recognizing pinch_begin
    // as Started for pan + zoom + rotate.
    this.events.push_back(Event::Pointer(PointerEvent::Pan {
        phase: GesturePhase::Started,
        translation_delta: (0.0, 0.0),
        num_touches: fingers,
    }));
}

unsafe extern "C" fn handle_zwp_pointer_gesture_swipe_v1_update(
    data: *mut c_void,
    _zwp_pointer_gesture_swipe_v1: *mut wayland::zwp_pointer_gesture_swipe_v1,
    _time: u32,
    dx: wayland::wl_fixed,
    dy: wayland::wl_fixed,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    let fingers = this.swipe_fingers.expect("set fingers on start");

    this.events.push_back(Event::Pointer(PointerEvent::Pan {
        phase: GesturePhase::Updated,
        // TODO: do i need to scale dx and dy by fractional scale?
        translation_delta: (wayland::wl_fixed_to_f64(dx), wayland::wl_fixed_to_f64(dy)),
        num_touches: fingers,
    }));
}

unsafe extern "C" fn handle_zwp_pointer_gesture_swipe_v1_end(
    data: *mut c_void,
    _zwp_pointer_gesture_swipe_v1: *mut wayland::zwp_pointer_gesture_swipe_v1,
    _serial: u32,
    _time: u32,
    cancelled: i32,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    let phase = if cancelled == 1 {
        GesturePhase::Cancelled
    } else {
        GesturePhase::Finished
    };

    let fingers = this.swipe_fingers.take().expect("set fingers on start");

    this.events.push_back(Event::Pointer(PointerEvent::Pan {
        phase,
        translation_delta: (0.0, 0.0),
        num_touches: fingers,
    }));
}

unsafe extern "C" fn handle_zwp_pointer_gesture_pinch_v1_begin(
    data: *mut c_void,
    _zwp_pointer_gesture_pinch_v1: *mut wayland::zwp_pointer_gesture_pinch_v1,
    _serial: u32,
    _time: u32,
    _surface: *mut wayland::wl_surface,
    fingers: u32,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    let phase = GesturePhase::Started;

    // NOTE: pinch scale must be unset on end.
    assert!(this.pinch_scale.is_none());
    this.pinch_scale = Some(1.0);

    // QUOTE:
    // > pinch gestures are executed when two or more fingers are located on the touchpad
    // - https://wayland.freedesktop.org/libinput/doc/latest/gestures.html#pinch-gestures
    assert!(fingers >= 2);
    assert!(fingers <= u8::MAX as u32);
    let fingers = fingers as u8;
    // NOTE: pinch fingers must be unset on end.
    assert!(this.pinch_fingers.is_none());
    this.pinch_fingers = Some(fingers);

    // TODO: consider introducing a `Possible` GesturePhase variant and not recognizing pinch_begin
    // as Started for pan + zoom + rotate.
    this.events.push_back(Event::Pointer(PointerEvent::Pan {
        phase,
        translation_delta: (0.0, 0.0),
        num_touches: fingers,
    }));
    this.events.push_back(Event::Pointer(PointerEvent::Zoom {
        phase,
        scale_delta: 0.0,
    }));
    this.events.push_back(Event::Pointer(PointerEvent::Rotate {
        phase,
        rotation_delta: 0.0,
    }));
}

unsafe extern "C" fn handle_zwp_pointer_gesture_pinch_v1_update(
    data: *mut c_void,
    _zwp_pointer_gesture_pinch_v1: *mut wayland::zwp_pointer_gesture_pinch_v1,
    _time: u32,
    dx: wayland::wl_fixed,
    dy: wayland::wl_fixed,
    scale: wayland::wl_fixed,
    rotation: wayland::wl_fixed,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    let phase = GesturePhase::Updated;

    let fingers = this.pinch_fingers.expect("set fingers on start");

    // NOTE: scale is relative to the initial finger position. we want to report deltas.
    let next_scale = wayland::wl_fixed_to_f64(scale);
    let prev_scale = this
        .pinch_scale
        .replace(next_scale)
        .expect("set scale on start");
    let scale_delta = next_scale - prev_scale;

    this.events.push_back(Event::Pointer(PointerEvent::Pan {
        phase,
        // TODO: do i need to scale dx and dy by fractional scale?
        translation_delta: (wayland::wl_fixed_to_f64(dx), wayland::wl_fixed_to_f64(dy)),
        num_touches: fingers,
    }));
    this.events
        .push_back(Event::Pointer(PointerEvent::Zoom { phase, scale_delta }));
    this.events.push_back(Event::Pointer(PointerEvent::Rotate {
        phase,
        rotation_delta: wayland::wl_fixed_to_f64(rotation),
    }));
}

unsafe extern "C" fn handle_zwp_pointer_gesture_pinch_v1_end(
    data: *mut c_void,
    _zwp_pointer_gesture_pinch_v1: *mut wayland::zwp_pointer_gesture_pinch_v1,
    _serial: u32,
    _time: u32,
    cancelled: i32,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    let phase = if cancelled == 1 {
        GesturePhase::Cancelled
    } else {
        GesturePhase::Finished
    };

    let _scale = this.pinch_scale.take().expect("set scale on start");
    let fingers = this.pinch_fingers.take().expect("set fingers on start");

    this.events.push_back(Event::Pointer(PointerEvent::Pan {
        phase,
        translation_delta: (0.0, 0.0),
        num_touches: fingers,
    }));
    this.events.push_back(Event::Pointer(PointerEvent::Zoom {
        phase,
        scale_delta: 0.0,
    }));
    this.events.push_back(Event::Pointer(PointerEvent::Rotate {
        phase,
        rotation_delta: 0.0,
    }));
}

const ZWP_POINTER_GESTURE_SWIPE_V1_LISTENER: wayland::zwp_pointer_gesture_swipe_v1_listener =
    wayland::zwp_pointer_gesture_swipe_v1_listener {
        begin: handle_zwp_pointer_gesture_swipe_v1_begin,
        update: handle_zwp_pointer_gesture_swipe_v1_update,
        end: handle_zwp_pointer_gesture_swipe_v1_end,
    };

const ZWP_POINTER_GESTURE_PINCH_V1_LISTENER: wayland::zwp_pointer_gesture_pinch_v1_listener =
    wayland::zwp_pointer_gesture_pinch_v1_listener {
        begin: handle_zwp_pointer_gesture_pinch_v1_begin,
        update: handle_zwp_pointer_gesture_pinch_v1_update,
        end: handle_zwp_pointer_gesture_pinch_v1_end,
    };

unsafe extern "C" fn handle_wl_keyboard_keymap(
    data: *mut c_void,
    _wl_keyboard: *mut wayland::wl_keyboard,
    format: u32,
    fd: i32,
    size: u32,
) {
    log::debug!("recv wl_keyboard_keymap");

    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    match format {
        wayland::WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1 => {
            // TODO: this need to be more robust. panics on sway reload (mod+shift+c).
            assert!(this.xkb_context.is_none());
            let xkb_context =
                unsafe { xkb::Context::from_fd(fd, size) }.expect("could not create xkb context");
            this.xkb_context = Some(xkb_context);
            log::info!("created xkb context");
        }
        other => unreachable!("unknown keymap format: {other}"),
    }

    unsafe { libc::close(fd) };
}

unsafe extern "C" fn handle_wl_keyboard_enter(
    data: *mut c_void,
    _wl_keyboard: *mut wayland::wl_keyboard,
    serial: u32,
    _surface: *mut wayland::wl_surface,
    _keys: *mut wayland::wl_array,
) {
    log::debug!("recv wl_keyboard_enter");

    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    this.serial_tracker
        .update_serial(SerialType::KeyboardEnter, serial);
}

unsafe extern "C" fn handle_wl_keyboard_leave(
    data: *mut c_void,
    _wl_keyboard: *mut wayland::wl_keyboard,
    _serial: u32,
    _surface: *mut wayland::wl_surface,
) {
    log::debug!("recv wl_keyboard_leave");

    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    this.serial_tracker.reset_serial(SerialType::KeyboardEnter);

    // QUOTE: The data_offer is valid until a new data_offer or NULL is received or until the
    // client loses keyboard focus.
    this.wl_data_offer = null_mut();
}

unsafe extern "C" fn handle_wl_keyboard_key(
    data: *mut c_void,
    _wl_keyboard: *mut wayland::wl_keyboard,
    _serial: u32,
    _time: u32,
    key: u32,
    state: u32,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    let xkb_context = this
        .xkb_context
        .as_ref()
        .expect("xkb contex has not been created");

    let scancode = map_keyboard_key(key);

    // NOTE: convert to xkb. for more info see comment above EVDEV_OFFSET.
    let xkb_key = key + EVDEV_OFFSET;
    let xkb_sym =
        unsafe { (xkb_context.libxkbcommon.xkb_state_key_get_one_sym)(xkb_context.state, xkb_key) };
    let utf32 = unsafe { (xkb_context.libxkbcommon.xkb_keysym_to_utf32)(xkb_sym) };
    let keycode = char::from_u32(utf32)
        .map_or_else(|| Keycode::Unidentified(RawKey::Unix(key)), Keycode::Char);

    match state {
        wayland::WL_KEYBOARD_KEY_STATE_PRESSED => {
            this.events.push_back(Event::Keyboard(KeyboardEvent::Key {
                state: KeyState::Pressed,
                scancode,
                keycode,
                repeat: false,
            }));

            if let Some(KeyRepeatInfo { rate, delay }) = this.key_repeat_info {
                assert!(!xkb_context.keymap.is_null());
                if unsafe {
                    (xkb_context.libxkbcommon.xkb_keymap_key_repeats)(xkb_context.keymap, xkb_key)
                } == 1
                {
                    this.key_repeat = Some((scancode, keycode));
                    if let Err(err) = unsafe { this.key_repeat_timerfd.arm(rate, delay) } {
                        log::error!("could not arm key repeat: {err}");
                    }
                }
            }
        }
        wayland::WL_KEYBOARD_KEY_STATE_RELEASED => {
            this.events.push_back(Event::Keyboard(KeyboardEvent::Key {
                state: KeyState::Released,
                scancode,
                keycode,
                repeat: false,
            }));

            this.key_repeat = None;
            if let Err(err) = unsafe { this.key_repeat_timerfd.disarm() } {
                log::error!("could not disarm key repeat: {err}");
            }
        }
        wayland::WL_KEYBOARD_KEY_STATE_REPEATED => {
            // NOTE: key repetition is handled with repeat info timer ^.
        }
        other => {
            log::warn!("unknown keyboard key state: {other}");
        }
    }
}

unsafe extern "C" fn handle_wl_keyboard_modifiers(
    data: *mut c_void,
    _wl_keyboard: *mut wayland::wl_keyboard,
    _serial: u32,
    mods_depressed: u32,
    mods_latched: u32,
    mods_locked: u32,
    group: u32,
) {
    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    let xkb_context = this
        .xkb_context
        .as_ref()
        .expect("xkb contex has not been created");
    unsafe {
        (xkb_context.libxkbcommon.xkb_state_update_mask)(
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
    _wl_keyboard: *mut wayland::wl_keyboard,
    rate: i32,
    delay: i32,
) {
    // QUOTE: negative values for either rate or delay are illegal.
    assert!(rate >= 0 && delay >= 0);

    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    // NOTE: a rate of zero disables any repeating, regardless of the delay's value.
    this.key_repeat_info = if rate == 0 {
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

const WL_KEYBOARD_LISTENER: wayland::wl_keyboard_listener = wayland::wl_keyboard_listener {
    keymap: handle_wl_keyboard_keymap,
    enter: handle_wl_keyboard_enter,
    leave: handle_wl_keyboard_leave,
    key: handle_wl_keyboard_key,
    modifiers: handle_wl_keyboard_modifiers,
    repeat_info: handle_wl_keyboard_repeat_info,
};

unsafe extern "C" fn handle_wl_data_device_selection(
    data: *mut c_void,
    _wl_data_device: *mut wayland::wl_data_device,
    id: *mut wayland::wl_data_offer,
) {
    log::debug!("recv wl_data_device selection");

    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    // QUOTE: The data_offer is valid until a new data_offer or NULL is received or until the
    // client loses keyboard focus.
    this.wl_data_offer = id;
}

const WL_DATA_DEVICE_LISTENER: wayland::wl_data_device_listener =
    wayland::wl_data_device_listener {
        data_offer: noop_listener!(),
        enter: noop_listener!(),
        leave: noop_listener!(),
        motion: noop_listener!(),
        drop: noop_listener!(),
        selection: handle_wl_data_device_selection,
    };

unsafe extern "C" fn handle_wl_data_source_send(
    data: *mut c_void,
    wl_data_source: *mut wayland::wl_data_source,
    mime_type: *const c_char,
    fd: i32,
) {
    log::debug!("recv wl_data_source send");

    let this = unsafe { &mut *(data as *mut WaylandBackend) };

    // NOTE: PipeWriter becomes responsibile for closing fd. i am constructing it early here to not
    // have to manually close fd in each error case.
    let mut writer = unsafe { PipeWriter::from_raw_fd(fd) };

    // NOTE: never will be hit/unreachable (but compositor might be buggy? idk).
    let Some((data_provider, data_source)) = this.clipboard_data.as_ref() else {
        return;
    };

    if *data_source != wl_data_source {
        log::warn!("attempt to send on unknown data source");
        return;
    }

    let c_mime_type = unsafe { CStr::from_ptr(mime_type) };
    let Ok(mime_type) = c_mime_type.to_str() else {
        log::error!("invalid mime type: {c_mime_type:?}");
        return;
    };

    let supported_mime_types = data_provider.supported_mime_types();
    if !supported_mime_types.contains(&mime_type) {
        log::error!("unsupported mime type (got {mime_type}, want {supported_mime_types:?})");
        return;
    }

    if let Err(err) = data_provider.write_as(mime_type, &mut writer) {
        log::error!("could not write data into clipboard: {err:?}");
    }
}

unsafe extern "C" fn handle_wl_data_source_cancelled(
    data: *mut c_void,
    wl_data_source: *mut wayland::wl_data_source,
) {
    log::debug!("recv wl_data_source cancelled");

    let this = unsafe { &mut *(data as *mut WaylandBackend) };
    unsafe { wayland::wl_data_source_destroy(&this.libwayland_client, wl_data_source) };
    // NOTE: should always take. if existing data source != given -> compositor did a fucky wacky?
    this.clipboard_data
        .take_if(|(_, prev)| *prev == wl_data_source);
}

const WL_DATA_SOURCE_LISTENER: wayland::wl_data_source_listener =
    wayland::wl_data_source_listener {
        target: noop_listener!(),
        send: handle_wl_data_source_send,
        cancelled: handle_wl_data_source_cancelled,
        dnd_drop_performed: noop_listener!(),
        dnd_finished: noop_listener!(),
        action: noop_listener!(),
    };

impl WaylandBackend {
    pub fn new_boxed(attrs: WindowAttrs) -> anyhow::Result<Box<Self>> {
        let libwayland_client = wayland::ClientApi::load()?;

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

        let mut this = Box::new(WaylandBackend {
            libwayland_client,
            wl_display,

            wl_compositor: null_mut(),
            wl_data_device_manager: null_mut(),
            wl_seat: null_mut(),
            wl_shm: null_mut(),
            wp_cursor_shape_manager_v1: null_mut(),
            wp_fractional_scale_manager_v1: null_mut(),
            wp_viewporter: null_mut(),
            xdg_wm_base: null_mut(),
            zwp_pointer_gestures_v1: null_mut(),

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
            cursor: None,
            cursor_shape: None,
            wp_cursor_shape_device_v1: null_mut(),
            axis: None,
            axis_discrete: None,
            axis_value120: None,
            zwp_pointer_gesture_swipe_v1: null_mut(),
            zwp_pointer_gesture_pinch_v1: null_mut(),
            swipe_fingers: None,
            pinch_scale: None,
            pinch_fingers: None,

            wl_keyboard: null_mut(),
            xkb_context: None,
            key_repeat_timerfd,
            key_repeat_info: None,
            key_repeat: None,

            wl_data_device: null_mut(),
            wl_data_offer: null_mut(),
            clipboard_data: None,

            serial_tracker: SerialTracker::default(),
            events: VecDeque::new(),

            temp_cstr: TempCStr::new_with_capacity(255),
        });

        // init globals

        let wl_registry: *mut wayland::wl_registry = unsafe {
            wayland::wl_display_get_registry(&this.libwayland_client, this.wl_display.as_ptr())
        };
        if wl_registry.is_null() {
            return Err(anyhow!("could not get registry"));
        }
        unsafe {
            (this.libwayland_client.wl_proxy_add_listener)(
                wl_registry as *mut wayland::wl_proxy,
                &WL_REGISTRY_LISTENER as *const wayland::wl_registry_listener as _,
                this.as_mut() as *mut WaylandBackend as *mut c_void,
            );
            (this.libwayland_client.wl_display_roundtrip)(this.wl_display.as_ptr());
        }

        // TODO: consider handling those somehow more ~gracefully xd. at least provide useful info?
        assert!(!this.wl_compositor.is_null());
        assert!(!this.wl_seat.is_null());
        assert!(!this.wl_shm.is_null());
        assert!(!this.xdg_wm_base.is_null());

        log::info!("initialized window globals");

        unsafe {
            (this.libwayland_client.wl_proxy_add_listener)(
                this.wl_seat as *mut wayland::wl_proxy,
                &WL_SEAT_LISTENER as *const wayland::wl_seat_listener as _,
                this.as_mut() as *mut WaylandBackend as *mut c_void,
            )
        };

        unsafe {
            (this.libwayland_client.wl_proxy_add_listener)(
                this.xdg_wm_base as *mut wayland::wl_proxy,
                &XDG_WM_BASE_LISTENER as *const wayland::xdg_wm_base_listener as _,
                this.as_mut() as *mut WaylandBackend as *mut c_void,
            )
        };

        // init window

        this.wl_surface = unsafe {
            wayland::wl_compositor_create_surface(&this.libwayland_client, this.wl_compositor)
        };
        if this.wl_surface.is_null() {
            return Err(anyhow!("could not create wl surface"));
        }

        this.xdg_surface = unsafe {
            wayland::xdg_wm_base_get_xdg_surface(
                &this.libwayland_client,
                this.xdg_wm_base,
                this.wl_surface,
            )
        };
        if this.xdg_surface.is_null() {
            return Err(anyhow!("could not create xdg surface"));
        }
        unsafe {
            (this.libwayland_client.wl_proxy_add_listener)(
                this.xdg_surface as *mut wayland::wl_proxy,
                &XDG_SURFACE_LISTENER as *const wayland::xdg_surface_listener as _,
                this.as_mut() as *mut WaylandBackend as *mut c_void,
            )
        };

        this.xdg_toplevel =
            unsafe { wayland::xdg_surface_get_toplevel(&this.libwayland_client, this.xdg_surface) };
        if this.xdg_toplevel.is_null() {
            return Err(anyhow!("could not get xdg toplevel"));
        }
        unsafe {
            (this.libwayland_client.wl_proxy_add_listener)(
                this.xdg_toplevel as *mut wayland::wl_proxy,
                &XDG_TOPLEVEL_LISTENER as *const wayland::xdg_toplevel_listener as _,
                this.as_mut() as *mut WaylandBackend as *mut c_void,
            )
        };

        if !this.attrs.resizable {
            let logical_size = this.attrs.logical_size.unwrap_or(DEFAULT_LOGICAL_SIZE);
            unsafe {
                wayland::xdg_toplevel_set_min_size(
                    &this.libwayland_client,
                    this.xdg_toplevel,
                    logical_size.0 as i32,
                    logical_size.1 as i32,
                );
                wayland::xdg_toplevel_set_max_size(
                    &this.libwayland_client,
                    this.xdg_toplevel,
                    logical_size.0 as i32,
                    logical_size.1 as i32,
                )
            };
        }

        // dpi

        if !this.wp_fractional_scale_manager_v1.is_null() {
            this.wp_fractional_scale_v1 = unsafe {
                wayland::wp_fractional_scale_manager_v1_get_fractional_scale(
                    &this.libwayland_client,
                    this.wp_fractional_scale_manager_v1,
                    this.wl_surface,
                )
            };
            if this.wp_fractional_scale_v1.is_null() {
                return Err(anyhow!("could not get fractional scale"));
            }
            unsafe {
                (this.libwayland_client.wl_proxy_add_listener)(
                    this.wp_fractional_scale_v1 as *mut wayland::wl_proxy,
                    &WP_FRACTIONAL_SCALE_MANAGER_V1_LISTENER
                        as *const wayland::wp_fractional_scale_v1_listener as _,
                    this.as_mut() as *mut WaylandBackend as *mut c_void,
                )
            };
        }

        if !this.wp_viewporter.is_null() {
            this.wp_viewport = unsafe {
                wayland::wp_viewporter_get_viewport(
                    &this.libwayland_client,
                    this.wp_viewporter,
                    this.wl_surface,
                )
            };
        }

        // pointer

        this.wl_pointer =
            unsafe { wayland::wl_seat_get_pointer(&this.libwayland_client, this.wl_seat) };
        if this.wl_pointer.is_null() {
            return Err(anyhow!("could not get pointer"));
        }
        unsafe {
            (this.libwayland_client.wl_proxy_add_listener)(
                this.wl_pointer as *mut wayland::wl_proxy,
                &WL_POINTER_LISTENER as *const wayland::wl_pointer_listener as _,
                this.as_mut() as *mut WaylandBackend as *mut c_void,
            )
        };

        if !this.wp_cursor_shape_manager_v1.is_null() {
            this.wp_cursor_shape_device_v1 = unsafe {
                wayland::wp_cursor_shape_manager_v1_get_pointer(
                    &this.libwayland_client,
                    this.wp_cursor_shape_manager_v1,
                    this.wl_pointer,
                )
            };
            if this.wp_cursor_shape_device_v1.is_null() {
                return Err(anyhow!("could not get cursor shape device"));
            }
        } else {
            this.cursor = Cursor::init(
                &this.libwayland_client,
                this.wl_compositor,
                this.wl_shm,
                this.scale_factor(),
            )
            .map(Some)
            .context("could not init cursor")?;
        }

        if !this.zwp_pointer_gestures_v1.is_null() {
            this.zwp_pointer_gesture_swipe_v1 = unsafe {
                wayland::zwp_pointer_gestures_v1_get_swipe_gesture(
                    &this.libwayland_client,
                    this.zwp_pointer_gestures_v1,
                    this.wl_pointer,
                )
            };
            if this.zwp_pointer_gesture_swipe_v1.is_null() {
                return Err(anyhow!("could not get swipe gesture"));
            }
            unsafe {
                (this.libwayland_client.wl_proxy_add_listener)(
                    this.zwp_pointer_gesture_swipe_v1 as *mut wayland::wl_proxy,
                    &ZWP_POINTER_GESTURE_SWIPE_V1_LISTENER
                        as *const wayland::zwp_pointer_gesture_swipe_v1_listener
                        as _,
                    this.as_mut() as *mut WaylandBackend as *mut c_void,
                )
            };

            this.zwp_pointer_gesture_pinch_v1 = unsafe {
                wayland::zwp_pointer_gestures_v1_get_pinch_gesture(
                    &this.libwayland_client,
                    this.zwp_pointer_gestures_v1,
                    this.wl_pointer,
                )
            };
            if this.zwp_pointer_gesture_pinch_v1.is_null() {
                return Err(anyhow!("could not get pinch gesture"));
            }
            unsafe {
                (this.libwayland_client.wl_proxy_add_listener)(
                    this.zwp_pointer_gesture_pinch_v1 as *mut wayland::wl_proxy,
                    &ZWP_POINTER_GESTURE_PINCH_V1_LISTENER
                        as *const wayland::zwp_pointer_gesture_pinch_v1_listener
                        as _,
                    this.as_mut() as *mut WaylandBackend as *mut c_void,
                )
            };
        }

        // keyboard

        this.wl_keyboard =
            unsafe { wayland::wl_seat_get_keyboard(&this.libwayland_client, this.wl_seat) };
        if this.wl_keyboard.is_null() {
            return Err(anyhow!("could not get keyboard"));
        }
        unsafe {
            (this.libwayland_client.wl_proxy_add_listener)(
                this.wl_keyboard as *mut wayland::wl_proxy,
                &WL_KEYBOARD_LISTENER as *const wayland::wl_keyboard_listener as _,
                this.as_mut() as *mut WaylandBackend as *mut c_void,
            )
        };

        // clipboard

        if !this.wl_data_device_manager.is_null() {
            this.wl_data_device = unsafe {
                wayland::wl_data_device_manager_get_data_device(
                    &this.libwayland_client,
                    this.wl_data_device_manager,
                    this.wl_seat,
                )
            };
            if this.wl_data_device.is_null() {
                return Err(anyhow!("could not get data device"));
            }
            unsafe {
                (this.libwayland_client.wl_proxy_add_listener)(
                    this.wl_data_device as *mut wayland::wl_proxy,
                    &WL_DATA_DEVICE_LISTENER as *const wayland::wl_data_device_listener as _,
                    this.as_mut() as *mut WaylandBackend as *mut c_void,
                )
            };
        }

        // finalize

        unsafe { wayland::wl_surface_commit(&this.libwayland_client, this.wl_surface) };
        unsafe { (this.libwayland_client.wl_display_roundtrip)(this.wl_display.as_ptr()) };

        // TODO: consider waiting for fractional scale event (if fractional scale interface exists)
        assert!(this.acked_first_xdg_surface_ack_configure);
        this.events.push_back(Event::Window(WindowEvent::Configure {
            logical_size: this.logical_size.expect("configured logical size"),
        }));

        log::info!("initialized window");

        Ok(this)
    }

    fn set_cursor_shape(&mut self, shape: CursorShape) -> anyhow::Result<()> {
        let Some(serial) = self.serial_tracker.get_serial(SerialType::PointerEnter) else {
            log::warn!("could not set cursor shape (no pointer enter)");
            return Ok(());
        };

        if !self.wp_cursor_shape_device_v1.is_null() {
            unsafe {
                wayland::wp_cursor_shape_device_v1_set_shape(
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

    fn maybe_resize(&mut self, logical_size: Option<(u32, u32)>, scale_factor: Option<f64>) {
        assert!(logical_size.is_some() || scale_factor.is_some());

        let mut logical_size_changed = false;
        if let Some(logical_size) = logical_size {
            logical_size_changed = self.logical_size != Some(logical_size);
            if logical_size_changed {
                self.logical_size = Some(logical_size);

                if !self.wp_viewporter.is_null() {
                    unsafe {
                        wayland::wp_viewport_set_destination(
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
                        Ok(_) => {
                            // NOTE: cursor needs to be updated after re-scaling.
                            let shape = self.cursor_shape.unwrap_or(CursorShape::Default);
                            if let Err(err) = self.set_cursor_shape(shape) {
                                log::error!("could not set cursor shape (during rescale): {err:?}");
                            }
                        }
                        Err(err) => {
                            log::error!("could not set cursor scale (during rescale): {err:?}");
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

    fn get_clipboard_data(&mut self, mime_type: &str, buf: &mut Vec<u8>) -> anyhow::Result<usize> {
        // NOTE: try to read from existing clipboard data because otherwise the whole thing will
        // hang because reads and writes happen on the same thread here.
        if let Some((data_provider, _data_source)) = self.clipboard_data.as_ref() {
            if data_provider.supported_mime_types().contains(&mime_type) {
                return data_provider
                    .write_as(mime_type, buf)
                    .context("failed to write into buf from existing data provier");
            }
            // TODO: is this ok?
            return Ok(0);
        }

        if self.wl_data_offer.is_null() {
            return Err(anyhow!("data device manager is missing"));
        }

        let mut fds = [0 as c_int; 2];
        let ret = unsafe { libc::pipe(fds.as_mut_ptr()) };
        if ret == -1 {
            let errno = unsafe { *libc::__errno_location() };
            return Err(anyhow!("could not pipe: 0x:{errno:x}"));
        }
        let [read_fd, write_fd] = fds;

        let c_mime_type = self.temp_cstr.from_str(mime_type);
        unsafe {
            wayland::wl_data_offer_receive(
                &self.libwayland_client,
                self.wl_data_offer,
                c_mime_type.as_ptr(),
                write_fd,
            )
        };
        self.temp_cstr.clear();

        let ret = unsafe { libc::close(write_fd) };
        if ret == -1 {
            // NOTE: i am hesitant to treat this as critical error, but rather as something that is
            // very unlikely to ever happen and if it'll happen - probably no biggie?
            let errno = unsafe { *libc::__errno_location() };
            log::error!("could not close clipboard writer pipe: 0x:{errno:x}");
        }

        let ret = unsafe { (self.libwayland_client.wl_display_flush)(self.wl_display.as_ptr()) };
        if ret == -1 {
            // TODO: handle wl_display_flush's EAGAIN errno?
            return Err(anyhow!("wl_display_flush failed"));
        }

        // NOTE: PipeReader becomes responsibile for closing fd.
        unsafe { PipeReader::from_raw_fd(read_fd) }
            .read_to_end(buf)
            .context("could not read from pipe")
    }

    fn set_clipboard_data(
        &mut self,
        data_provider: Box<dyn ClipboardDataProvider>,
    ) -> anyhow::Result<()> {
        if let Some((data_provider, data_source)) = self.clipboard_data.take() {
            drop(data_provider);
            unsafe { wayland::wl_data_source_destroy(&self.libwayland_client, data_source) };
        }

        let supported_mime_types = data_provider.supported_mime_types();
        if supported_mime_types.is_empty() {
            return Err(anyhow!("data provider supports nothing huh?"));
        }

        // TODO: consider not bailing-out if there's no serial, but defering the offer until there
        // is?
        let Some(serial) = self
            .serial_tracker
            .get_serial(SerialType::PointerEnter)
            .or_else(|| self.serial_tracker.get_serial(SerialType::KeyboardEnter))
        else {
            return Err(anyhow!("no pointer nor keyboard serial found"));
        };

        if self.wl_data_device_manager.is_null() {
            return Err(anyhow!("data device manager is missing"));
        }

        let data_source = unsafe {
            wayland::wl_data_device_manager_create_data_source(
                &self.libwayland_client,
                self.wl_data_device_manager,
            )
        };
        if data_source.is_null() {
            return Err(anyhow!("failed to create data source"));
        }

        unsafe {
            (self.libwayland_client.wl_proxy_add_listener)(
                data_source as *mut wayland::wl_proxy,
                &WL_DATA_SOURCE_LISTENER as *const wayland::wl_data_source_listener as _,
                self as *mut WaylandBackend as *mut c_void,
            )
        };

        for mime_type in supported_mime_types {
            let c_mime_type = self.temp_cstr.from_str(*mime_type);
            unsafe {
                wayland::wl_data_source_offer(
                    &self.libwayland_client,
                    data_source,
                    c_mime_type.as_ptr(),
                )
            };
            self.temp_cstr.clear();
        }

        unsafe {
            wayland::wl_data_device_set_selection(
                &self.libwayland_client,
                self.wl_data_device,
                data_source,
                serial,
            )
        };

        self.clipboard_data = Some((data_provider, data_source));

        Ok(())
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
                            self.events.push_back(Event::Keyboard(KeyboardEvent::Key {
                                state: KeyState::Pressed,
                                scancode,
                                keycode,
                                repeat: true,
                            }));
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

        self.set_cursor_shape(shape)
    }

    fn read_clipboard(&mut self, mime_type: &str, buf: &mut Vec<u8>) -> anyhow::Result<usize> {
        self.get_clipboard_data(mime_type, buf)
    }

    fn provide_clipboard_data(
        &mut self,
        data_provider: Box<dyn ClipboardDataProvider>,
    ) -> anyhow::Result<()> {
        self.set_clipboard_data(data_provider)
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
