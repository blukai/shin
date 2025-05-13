use std::collections::VecDeque;

use raw_window_handle as rwh;

use crate::{Window, WindowAttrs, WindowEvent, DEFAULT_LOGICAL_SIZE};

unsafe extern "C" {
    fn resize_canvas(width: i32, height: i32);
}

pub struct WebBackend {
    attrs: WindowAttrs,

    events: VecDeque<WindowEvent>,
}

impl WebBackend {
    pub fn new_boxed(attrs: WindowAttrs) -> anyhow::Result<Box<Self>> {
        let (width, height) = attrs.logical_size.unwrap_or(DEFAULT_LOGICAL_SIZE);
        unsafe { resize_canvas(width as i32, height as i32) };

        let mut events = VecDeque::new();
        events.push_back(WindowEvent::Configure {
            logical_size: attrs.logical_size.unwrap_or(DEFAULT_LOGICAL_SIZE),
        });

        let boxed = Box::new(Self { attrs, events });

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
