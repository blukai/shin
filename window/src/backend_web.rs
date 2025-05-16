use std::collections::VecDeque;
use std::ffi::CString;

use anyhow::anyhow;
use raw_window_handle as rwh;

use crate::{Window, WindowAttrs, WindowEvent, DEFAULT_LOGICAL_SIZE};

pub mod js_bindings {
    use std::ffi::{c_char, c_void};

    #[derive(Debug, Clone)]
    #[repr(transparent)]
    pub(super) struct ExternRef {
        idx: u32,
    }

    impl ExternRef {
        pub(super) fn is_nil(&self) -> bool {
            self.idx == 0
        }
    }

    unsafe extern "C" {
        pub fn panic(msg: *const c_char);

        pub fn console_log(msg: *const c_char);

        pub fn request_animation_frame_loop(
            f: unsafe extern "C" fn(*mut c_void) -> bool,
            ctx: *mut c_void,
        );

        pub(super) fn canvas_get_by_id(id: *const c_char) -> ExternRef;
        pub(super) fn canvas_get_size(el: ExternRef, width: *mut i32, height: *mut i32);
        pub(super) fn canvas_set_size(el: ExternRef, width: i32, height: i32);
        pub(super) fn canvas_get_context(el: ExternRef, context_type: *const c_char) -> ExternRef;

        pub(super) fn gl_clear_color(ctx: ExternRef, r: f32, g: f32, b: f32, a: f32);
        pub(super) fn gl_clear(ctx: ExternRef, mask: u32);
    }
}

pub struct WebBackend {
    attrs: WindowAttrs,

    canvas: js_bindings::ExternRef,
    webgl2_context: js_bindings::ExternRef,

    events: VecDeque<WindowEvent>,
}

impl WebBackend {
    pub fn new_boxed(attrs: WindowAttrs) -> anyhow::Result<Box<Self>> {
        let mut events = VecDeque::new();

        let canvas_id = attrs.canvas_id.as_ref().map_or_else(
            || CString::new("canvas"),
            |payload| CString::new(payload.as_ref()),
        )?;
        let canvas = unsafe { js_bindings::canvas_get_by_id(canvas_id.as_ptr()) };
        if canvas.is_nil() {
            return Err(anyhow!("could not get canvas"));
        }

        let (width, height) = attrs.logical_size.unwrap_or(DEFAULT_LOGICAL_SIZE);
        unsafe { js_bindings::canvas_set_size(canvas.clone(), width as i32, height as i32) };
        let (mut width, mut height) = (0_i32, 0_i32);
        unsafe { js_bindings::canvas_get_size(canvas.clone(), &mut width, &mut height) };
        events.push_back(WindowEvent::Configure {
            logical_size: (width as u32, height as u32),
        });
        // TODO: scale factor (/ pixel ratio)

        // TODO: this must noe be happening here. this must be responsibility of graphics crate!
        let webgl2_context =
            unsafe { js_bindings::canvas_get_context(canvas.clone(), c"webgl2".as_ptr()) };
        if webgl2_context.is_nil() {
            return Err(anyhow!("could not get webgl2 context"));
        }
        unsafe {
            js_bindings::gl_clear_color(webgl2_context.clone(), 1.0, 0.0, 0.0, 1.0);
            js_bindings::gl_clear(webgl2_context.clone(), 0x00004000);
        }

        let boxed = Box::new(Self {
            attrs,

            canvas,
            webgl2_context,

            events,
        });

        Ok(boxed)
    }
}

impl rwh::HasDisplayHandle for WebBackend {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        unimplemented!()
    }
}

impl rwh::HasWindowHandle for WebBackend {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        unimplemented!()
    }
}

impl Window for WebBackend {
    fn pump_events(&mut self) -> anyhow::Result<()> {
        unreachable!()
    }

    fn pop_event(&mut self) -> Option<WindowEvent> {
        self.events.pop_back()
    }
}
