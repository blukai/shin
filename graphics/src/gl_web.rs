use std::ffi::c_char;

use raw_window_handle as rwh;

use super::Contexter;

#[derive(Debug, Clone)]
#[repr(transparent)]
struct ExternRef {
    idx: u32,
}

impl ExternRef {
    pub(super) fn is_nil(&self) -> bool {
        self.idx == 0
    }
}

unsafe extern "C" {
    fn canvas_get_context(this: ExternRef, context_type: *const c_char) -> ExternRef;

    fn gl_clear_color(this: ExternRef, red: f32, green: f32, blue: f32, alpha: f32);
    fn gl_clear(this: ExternRef, mask: u32);
}

pub struct Context {
    webgl2_context: ExternRef,
}

impl Context {
    pub fn from_window_handle(window_handle: rwh::WindowHandle) -> Self {
        let rwh::RawWindowHandle::WebCanvas(canvas) = window_handle.as_raw() else {
            panic!("unsupported window system (window handle: {window_handle:?})");
        };
        let canvas = ExternRef {
            idx: unsafe { *(canvas.obj.as_ptr() as *mut u32) },
        };
        assert!(!canvas.is_nil());

        let webgl2_context = unsafe { canvas_get_context(canvas.clone(), c"webgl2".as_ptr()) };
        assert!(!webgl2_context.is_nil());
        Self { webgl2_context }
    }
}

impl Contexter for Context {
    #[inline]
    unsafe fn clear_color(&self, red: f32, green: f32, blue: f32, alpha: f32) {
        unsafe { gl_clear_color(self.webgl2_context.clone(), red, green, blue, alpha) }
    }

    #[inline]
    unsafe fn clear(&self, mask: u32) {
        unsafe { gl_clear(self.webgl2_context.clone(), mask) }
    }
}
