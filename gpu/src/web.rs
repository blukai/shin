use std::ffi::c_char;

use raw_window_handle as rwh;

unsafe extern "C" {
    fn canvas_get_context(extern_ref: usize, context_type: *const c_char) -> usize;
}

pub struct Surface {
    canvas: usize,
    webgl2: usize,
}

impl Surface {
    pub fn new(window_handle: rwh::WindowHandle) -> Self {
        let rwh::RawWindowHandle::WebCanvas(canvas) = window_handle.as_raw() else {
            panic!("unsupported window system (window handle: {window_handle:?})");
        };

        let canvas = unsafe { *(canvas.obj.as_ptr() as *mut usize) };
        assert!(canvas != 0);

        let webgl2 = unsafe { canvas_get_context(canvas, c"webgl2".as_ptr()) };
        assert!(webgl2 != 0);

        Self { canvas, webgl2 }
    }

    pub fn as_extern_ref(&self) -> usize {
        self.webgl2
    }
}
