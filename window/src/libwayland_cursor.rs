#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_uint};

use dynlib::{DynLib, opaque_struct};

use crate::libwayland_client;

opaque_struct!(wl_cursor_theme);

#[repr(C)]
pub struct wl_cursor_image {
    pub width: u32,
    pub height: u32,
    pub hotspot_x: u32,
    pub hotspot_y: u32,
    pub delay: u32,
}

#[repr(C)]
pub struct wl_cursor {
    pub image_count: c_uint,
    pub images: *mut *mut wl_cursor_image,
    pub name: *const c_char,
}

pub struct Lib {
    pub wl_cursor_theme_load: unsafe extern "C" fn(
        name: *const c_char,
        size: c_int,
        shm: *mut libwayland_client::wl_shm,
    ) -> *mut wl_cursor_theme,
    pub wl_cursor_theme_destroy: unsafe extern "C" fn(theme: *mut wl_cursor_theme),
    pub wl_cursor_theme_get_cursor:
        unsafe extern "C" fn(theme: *mut wl_cursor_theme, name: *const c_char) -> *mut wl_cursor,
    pub wl_cursor_image_get_buffer:
        unsafe extern "C" fn(image: *mut wl_cursor_image) -> *mut libwayland_client::wl_buffer,

    _dynlib: DynLib,
}

unsafe impl Sync for Lib {}
unsafe impl Send for Lib {}

impl Lib {
    pub fn load() -> anyhow::Result<Self> {
        let dynlib = DynLib::open(c"libwayland-cursor.so")
            .or_else(|_| DynLib::open(c"libwayland-cursor.so.0"))?;

        Ok(Self {
            wl_cursor_theme_load: dynlib.lookup(c"wl_cursor_theme_load")?,
            wl_cursor_theme_destroy: dynlib.lookup(c"wl_cursor_theme_destroy")?,
            wl_cursor_theme_get_cursor: dynlib.lookup(c"wl_cursor_theme_get_cursor")?,
            wl_cursor_image_get_buffer: dynlib.lookup(c"wl_cursor_image_get_buffer")?,

            _dynlib: dynlib,
        })
    }
}
