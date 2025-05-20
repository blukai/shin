use std::ffi::c_char;

use raw_window_handle as rwh;

use super::GlContexter;

unsafe extern "C" {
    fn gl_clear_color(extern_ref: usize, red: f32, green: f32, blue: f32, alpha: f32);
    fn gl_clear(extern_ref: usize, mask: u32);
}

pub struct Context {
    extern_ref: usize,
}

impl Context {
    pub fn from_extern_ref(extern_ref: usize) -> Self {
        Self { extern_ref }
    }
}

impl GlContexter for Context {
    #[inline]
    unsafe fn clear_color(&self, red: f32, green: f32, blue: f32, alpha: f32) {
        unsafe { gl_clear_color(self.extern_ref, red, green, blue, alpha) }
    }

    #[inline]
    unsafe fn clear(&self, mask: u32) {
        unsafe { gl_clear(self.extern_ref, mask) }
    }
}
