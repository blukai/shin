#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_int, c_uint};
use std::marker;

use dynlib::DynLib;

use crate::libwayland_client;

#[repr(C)]
pub struct wl_cursor_theme {
    _data: (),
    _marker: marker::PhantomData<(*mut u8, marker::PhantomPinned)>,
}

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

// TODO: consider naming this LibwaylandCursor.
pub struct CursorApi {
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

impl CursorApi {
    pub fn load() -> Result<Self, dynlib::Error> {
        let dynlib = DynLib::load(c"libwayland-cursor.so")
            .or_else(|_| DynLib::load(c"libwayland-cursor.so.0"))?;

        Ok(Self {
            wl_cursor_theme_load: dynlib.lookup(c"wl_cursor_theme_load")?,
            wl_cursor_theme_destroy: dynlib.lookup(c"wl_cursor_theme_destroy")?,
            wl_cursor_theme_get_cursor: dynlib.lookup(c"wl_cursor_theme_get_cursor")?,
            wl_cursor_image_get_buffer: dynlib.lookup(c"wl_cursor_image_get_buffer")?,

            _dynlib: dynlib,
        })
    }
}
