#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_void};

use dynlib::{DynLib, opaque_struct};

pub const WL_MARSHAL_FLAG_DESTROY: u32 = 1 << 0;

opaque_struct!(wl_proxy);

#[repr(C)]
#[derive(Debug, Clone)]
pub struct wl_message {
    pub name: *const c_char,
    pub signature: *const c_char,
    pub types: *const *const wl_interface,
}

unsafe impl Sync for wl_message {}
unsafe impl Send for wl_message {}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct wl_interface {
    pub name: *const c_char,
    pub version: c_int,
    pub method_count: c_int,
    pub methods: *const wl_message,
    pub event_count: c_int,
    pub events: *const wl_message,
}

unsafe impl Sync for wl_interface {}
unsafe impl Send for wl_interface {}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct wl_array {
    pub size: usize,
    pub alloc: usize,
    pub data: *mut c_void,
}

pub type wl_fixed = i32;

#[inline]
pub fn wl_fixed_to_f32(f: wl_fixed) -> f32 {
    (f as f32) / 256.0
}

pub struct Lib {
    pub wl_display_connect: unsafe extern "C" fn(name: *const c_char) -> *mut wl_display,
    pub wl_display_disconnect: unsafe extern "C" fn(display: *mut wl_display) -> *mut c_void,
    pub wl_display_dispatch: unsafe extern "C" fn(display: *mut wl_display) -> c_int,
    pub wl_display_dispatch_pending: unsafe extern "C" fn(display: *mut wl_display) -> c_int,
    pub wl_display_roundtrip: unsafe extern "C" fn(display: *mut wl_display) -> c_int,
    pub wl_display_flush: unsafe extern "C" fn(display: *mut wl_display) -> c_int,

    pub wl_proxy_add_listener: unsafe extern "C" fn(
        proxy: *mut wl_proxy,
        implementation: *mut unsafe extern "C" fn(),
        data: *mut c_void,
    ) -> c_int,
    pub wl_proxy_destroy: unsafe extern "C" fn(proxy: *mut wl_proxy),
    pub wl_proxy_get_version: unsafe extern "C" fn(proxy: *mut wl_proxy) -> u32,
    pub wl_proxy_marshal_flags: unsafe extern "C" fn(
        proxy: *mut wl_proxy,
        opcode: u32,
        interface: *const wl_interface,
        version: u32,
        flags: u32,
        ...
    ) -> *mut wl_proxy,

    _dynlib: DynLib,
}

impl Lib {
    pub fn load() -> anyhow::Result<Self> {
        let dynlib = DynLib::open(c"libwayland-client.so")
            .or_else(|_| DynLib::open(c"libwayland-client.so.0"))?;

        Ok(Self {
            wl_display_connect: dynlib.lookup(c"wl_display_connect")?,
            wl_display_disconnect: dynlib.lookup(c"wl_display_disconnect")?,
            wl_display_dispatch: dynlib.lookup(c"wl_display_dispatch")?,
            wl_display_dispatch_pending: dynlib.lookup(c"wl_display_dispatch_pending")?,
            wl_display_roundtrip: dynlib.lookup(c"wl_display_roundtrip")?,
            wl_display_flush: dynlib.lookup(c"wl_display_flush")?,

            wl_proxy_add_listener: dynlib.lookup(c"wl_proxy_add_listener")?,
            wl_proxy_destroy: dynlib.lookup(c"wl_proxy_destroy")?,
            wl_proxy_get_version: dynlib.lookup(c"wl_proxy_get_version")?,
            wl_proxy_marshal_flags: dynlib.lookup(c"wl_proxy_marshal_flags")?,

            _dynlib: dynlib,
        })
    }
}

#[allow(non_upper_case_globals)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/wayland_generated.rs"));
}
pub use generated::*;

unsafe extern "C" fn __noop_listener() {}
pub(crate) const __NOOP_LISTENER: unsafe extern "C" fn() = __noop_listener;
macro_rules! noop_listener {
    () => {
        unsafe {
            #[expect(clippy::missing_transmute_annotations)]
            std::mem::transmute(crate::libwayland_client::__NOOP_LISTENER)
        }
    };
}
pub(crate) use noop_listener;
