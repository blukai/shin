use super::Contexter;

#[allow(non_snake_case)]
#[allow(dead_code)]
mod api {
    use crate::gl::types::*;

    include!(concat!(env!("OUT_DIR"), "/gl_api.rs"));
}

pub struct Context {
    api: api::Api,
}

impl Context {
    pub unsafe fn load_with<F>(get_proc_address: F) -> Self
    where
        F: FnMut(*const std::ffi::c_char) -> *mut std::ffi::c_void,
    {
        Self {
            api: unsafe { api::Api::load_with(get_proc_address) },
        }
    }
}

impl Contexter for Context {
    #[inline]
    unsafe fn clear_color(&self, red: f32, green: f32, blue: f32, alpha: f32) {
        unsafe { self.api.ClearColor(red, green, blue, alpha) }
    }

    #[inline]
    unsafe fn clear(&self, mask: u32) {
        unsafe { self.api.Clear(mask) }
    }
}
