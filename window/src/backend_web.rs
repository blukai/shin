use std::collections::VecDeque;

use anyhow::Context as _;
use input::CursorShape;
use raw_window_handle as rwh;

use crate::{ClipboardDataProvider, DEFAULT_LOGICAL_SIZE, Event, Window, WindowAttrs};

pub struct WebBackend {
    attrs: WindowAttrs,

    canvas: js::Value,
    canvas_raw_handle: u32,

    events: VecDeque<Event>,
}

impl WebBackend {
    pub fn new_boxed(attrs: WindowAttrs) -> anyhow::Result<Box<Self>> {
        let events = VecDeque::new();

        let document = js::GLOBAL.get("document");
        let canvas = match attrs.canvas_id.as_ref() {
            Some(canvas_id) => document
                .get("getElementById")
                .call(&[js::Value::from_str(canvas_id)])
                .with_context(|| format!("could not get canvas (id {canvas_id})"))?,
            None => {
                let canvas = document
                    .get("createElement")
                    .call(&[js::Value::from_str("canvas")])
                    .context("could not create canvas")?;
                document
                    .get("body")
                    .get("append")
                    .call(&[canvas.clone()])
                    .context("could not append canvas")?;
                canvas
            }
        };

        let random = js::GLOBAL.get("Math").get("random");
        let canvas_raw_handle =
            (random.call(&[]).context("could not random")?.as_f64() * u32::MAX as f64) as u32;

        let dataset = canvas.get("dataset");
        dataset.set("rawHandle", &js::Value::from_f64(canvas_raw_handle as f64));

        let (width, height) = attrs.logical_size.unwrap_or(DEFAULT_LOGICAL_SIZE);
        canvas.set("width", &js::Value::from_f64(width as f64));
        canvas.set("height", &js::Value::from_f64(height as f64));

        // TODO: do i need to check if width and height were set correctly?

        // TODO: handle scale factor (/ pixel ratio)

        let boxed = Box::new(Self {
            attrs,

            canvas,
            canvas_raw_handle,

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
        let web = rwh::WebWindowHandle::new(self.canvas_raw_handle);
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

    fn read_clipboard(&mut self, _mime_type: &str, _buf: &mut Vec<u8>) -> anyhow::Result<usize> {
        unimplemented!()
    }

    fn provide_clipboard_data(
        &mut self,
        _data_provider: Box<dyn ClipboardDataProvider>,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn logical_size(&self) -> (u32, u32) {
        unimplemented!()
    }

    fn scale_factor(&self) -> f64 {
        unimplemented!()
    }
}
