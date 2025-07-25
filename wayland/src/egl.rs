#![allow(non_camel_case_types)]

use std::{
    ffi::{c_int, c_void},
    marker,
};

use dynlib::DynLib;

#[repr(C)]
pub struct wl_egl_window {
    _data: (),
    _marker: marker::PhantomData<(*mut u8, marker::PhantomPinned)>,
}

pub struct EglApi {
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

impl EglApi {
    pub fn load() -> Result<Self, dynlib::Error> {
        let dynlib =
            DynLib::load(c"libwayland-egl.so").or_else(|_| DynLib::load(c"libwayland-egl.so.1"))?;

        Ok(Self {
            wl_egl_window_create: dynlib.lookup(c"wl_egl_window_create")?,
            wl_egl_window_destroy: dynlib.lookup(c"wl_egl_window_destroy")?,
            wl_egl_window_resize: dynlib.lookup(c"wl_egl_window_resize")?,

            _dynlib: dynlib,
        })
    }
}
