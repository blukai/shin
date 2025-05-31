#![allow(non_camel_case_types)]

use std::ffi::{c_int, c_void};

use dynlib::{DynLib, opaque_struct};

opaque_struct!(wl_egl_window);

pub struct Lib {
    pub wl_egl_window_create: unsafe extern "C" fn(
        surface: *mut c_void,
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

    _dynlib: DynLib,
}

unsafe impl Sync for Lib {}
unsafe impl Send for Lib {}

impl Lib {
    pub fn load() -> anyhow::Result<Self> {
        let dynlib =
            DynLib::open(c"libwayland-egl.so").or_else(|_| DynLib::open(c"libwayland-egl.so.1"))?;

        Ok(Self {
            wl_egl_window_create: dynlib.lookup(c"wl_egl_window_create")?,
            wl_egl_window_destroy: dynlib.lookup(c"wl_egl_window_destroy")?,
            wl_egl_window_resize: dynlib.lookup(c"wl_egl_window_resize")?,

            _dynlib: dynlib,
        })
    }
}
