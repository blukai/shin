use std::ffi::c_char;

use raw_window_handle as rwh;

unsafe extern "C" {
    fn canvas_get_context(extern_ref: u32, context_type: *const c_char) -> u32;
}

pub struct Surface {
    canvas: u32,
    webgl2: u32,
}

impl Surface {
    pub fn new(window_handle: rwh::WindowHandle) -> Self {
        let rwh::RawWindowHandle::Web(web_window_handle) = window_handle.as_raw() else {
            panic!("unsupported window system (window handle: {window_handle:?})");
        };

        let canvas = web_window_handle.id;
        assert!(canvas != 0);

        let webgl2 = unsafe { canvas_get_context(canvas, c"webgl2".as_ptr()) };
        assert!(webgl2 != 0);

        Self { canvas, webgl2 }
    }

    pub fn as_extern_ref(&self) -> u32 {
        self.webgl2
    }
}
