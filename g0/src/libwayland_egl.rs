#![allow(non_camel_case_types)]

use std::ffi::c_int;

use dynlib::{DynLib, opaque_struct};
use w0::libwayland_client;

opaque_struct!(wl_egl_window);

pub struct Lib {
    pub wl_egl_window_create: unsafe extern "C" fn(
        surface: *mut libwayland_client::wl_surface,
        width: c_int,
        height: c_int,
    ) -> *mut wl_egl_window,
    pub wl_egl_window_destroy: unsafe extern "C" fn(egl_window: *mut wl_egl_window),
    pub wl_egl_window_resize: unsafe extern "C" fn(
        egl_window: *mut wl_egl_window,
        width: c_int,
        height: c_int,
        dx: c_int,
        dy: c_int,
    ),

    _lib: DynLib,
}

unsafe impl Sync for Lib {}
unsafe impl Send for Lib {}

impl Lib {
    pub fn load() -> anyhow::Result<Self> {
        let lib =
            DynLib::open(c"libwayland-egl.so").or_else(|_| DynLib::open(c"libwayland-egl.so.1"))?;

        Ok(Self {
            wl_egl_window_create: lib.lookup(c"wl_egl_window_create")?,
            wl_egl_window_destroy: lib.lookup(c"wl_egl_window_destroy")?,
            wl_egl_window_resize: lib.lookup(c"wl_egl_window_resize")?,

            _lib: lib,
        })
    }
}
