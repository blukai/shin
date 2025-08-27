use std::collections::VecDeque;
use std::ffi::CString;

use anyhow::anyhow;
use input::CursorShape;
use raw_window_handle as rwh;

use crate::{ClipboardDataProvider, DEFAULT_LOGICAL_SIZE, Event, Window, WindowAttrs, WindowEvent};

pub mod js_sys {
    use std::ffi::{c_char, c_void};

    unsafe extern "C" {
        pub fn panic(msg: *const c_char);

        pub fn console_log(msg: *const c_char);

        pub fn request_animation_frame_loop(
            f: unsafe extern "C" fn(*mut c_void) -> bool,
            ctx: *mut c_void,
        );

        pub(super) fn canvas_get_by_id(id: *const c_char) -> u32;
        pub(super) fn canvas_get_size(extern_ref: u32, width: *mut i32, height: *mut i32);
        pub(super) fn canvas_set_size(extern_ref: u32, width: i32, height: i32);
    }
}

pub struct WebBackend {
    attrs: WindowAttrs,

    canvas: u32,

    events: VecDeque<Event>,
}

impl WebBackend {
    pub fn new_boxed(attrs: WindowAttrs) -> anyhow::Result<Box<Self>> {
        let mut events = VecDeque::new();

        let canvas_id = attrs.canvas_id.as_ref().map_or_else(
            || CString::new("canvas"),
            |payload| CString::new(payload.as_ref()),
        )?;
        let canvas = unsafe { js_sys::canvas_get_by_id(canvas_id.as_ptr()) };
        if canvas == 0 {
            return Err(anyhow!("could not get canvas"));
        }

        let (width, height) = attrs.logical_size.unwrap_or(DEFAULT_LOGICAL_SIZE);
        unsafe { js_sys::canvas_set_size(canvas.clone(), width as i32, height as i32) };
        let (mut width, mut height) = (0_i32, 0_i32);
        unsafe { js_sys::canvas_get_size(canvas.clone(), &mut width, &mut height) };
        events.push_back(Event::Window(WindowEvent::Configure {
            logical_size: (width as u32, height as u32),
        }));
        // TODO: scale factor (/ pixel ratio)

        let boxed = Box::new(Self {
            attrs,

            canvas,

            events,
        });

        Ok(boxed)
    }
}

impl rwh::HasDisplayHandle for WebBackend {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        let web = rwh::WebDisplayHandle::new();
        let raw = rwh::RawDisplayHandle::Web(web);
        Ok(unsafe { rwh::DisplayHandle::borrow_raw(raw) })
    }
}

impl rwh::HasWindowHandle for WebBackend {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        assert!(self.canvas != 0);
        let web = rwh::WebWindowHandle::new(self.canvas);
        let raw = rwh::RawWindowHandle::Web(web);
        Ok(unsafe { rwh::WindowHandle::borrow_raw(raw) })
    }
}

impl Window for WebBackend {
    fn pump_events(&mut self) -> anyhow::Result<()> {
        // TODO
        Ok(())
    }

    fn pop_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    fn set_cursor_shape(&mut self, _cursor_shape: CursorShape) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn read_clipboard(&mut self, mime_type: &str, buf: &mut Vec<u8>) -> anyhow::Result<usize> {
        unimplemented!()
    }

    fn provide_clipboard_data(
        &mut self,
        data_provider: Box<dyn ClipboardDataProvider>,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn physical_size(&self) -> (u32, u32) {
        unimplemented!()
    }

    fn scale_factor(&self) -> f64 {
        unimplemented!()
    }
}
