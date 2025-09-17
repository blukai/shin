use std::env;

use anyhow::{Context as _, anyhow};
use raw_window_handle as rwh;

#[cfg(unix)]
mod backend_wayland;

#[cfg(feature = "winit")]
mod backend_winit;

#[cfg(target_family = "wasm")]
mod backend_web;

// TODO: consider separating wayland clipboard stuff out of wayland window backend into a separate
// wayland clipboard thing. because winit does not provide any clipboard apis and separate wayland
// clipboard can be used with winit backend when winit is backed by wayland.

pub const DEFAULT_LOGICAL_SIZE: (u32, u32) = (640, 480);

#[derive(Debug, Default, Clone)]
pub struct WindowAttrs {
    /// defaults to `canvas`.
    #[cfg(target_family = "wasm")]
    pub canvas_id: Option<Box<str>>,
    /// if not specified - [DEFAULT_LOGICAL_SIZE] will be used.
    pub logical_size: Option<(u32, u32)>,
    pub resizable: bool,
}

// TODO: i don't want this ti be called window event. maybe it does not even need to be separated
// from Event? cosider either renaming WindowEvent to SurfaceEvent or moving all variants into
// Event? or maybe you'll get better ideas?
#[derive(Debug)]
pub enum WindowEvent {
    Resized { logical_size: (u32, u32) },
    ScaleFactorChanged { scale_factor: f64 },
    CloseRequested,
}

// TODO: event probably needs to be split into Event and EventKind where Event will contain
// additional info such as surface id and possibly device id? it must be possible to ~route input
// events per-surface and possibly per-device?
#[derive(Debug)]
pub enum Event {
    Window(WindowEvent),
    Pointer(input::PointerEvent),
    Keyboard(input::KeyboardEvent),
}

// TODO: maybe clipboard needs not to be a part of window crate? nor input. but a separate crate
// that would allow to create clipboard thing from a rwh? maybe not.

pub const MIME_TYPE_TEXT: &str = "text/plain;charset=utf-8";

// NOTE: ClipboardDataProvider allows you to dictate how to and where to store and access your
// data. you may choose to grant ownership of data that you're putting into clipboard to the thing
// that implements the ClipboardDataProvider; you may choose to reference it via smart pointer or
// something, etc..
// it allows you to not have to store your clipboard data in many formats, but advertise many
// formats and convert it to the one that was requested (if any) on demand.
pub trait ClipboardDataProvider {
    fn supported_mime_types(&self) -> &[&str];

    // NOTE: the event loop probably should always call this method  in a separate thread to
    // prevent ui from being blocked. why? that is because for example in case of images most
    // likely you are working with raw pixels and it's not super cheap and quick to turn those into
    // png if we're thinking about large images.
    //
    // TODO: is there any value in having `write_as` return result or anything at all?
    //
    // ----
    //
    // if successful, this function must return the total number of bytes written.
    //
    // the implementer will never ask for a mime type that was told by the
    // [`Self::supported_mime_types`].
    fn write_as(&self, mime_type: &str, w: &mut dyn std::io::Write) -> anyhow::Result<usize>;
}

pub struct ClipboardTextProvider {
    text: String,
}

impl ClipboardTextProvider {
    pub fn new(text: String) -> Self {
        Self { text }
    }
}

impl ClipboardDataProvider for ClipboardTextProvider {
    fn supported_mime_types(&self) -> &[&str] {
        &[MIME_TYPE_TEXT]
    }

    fn write_as(&self, mime_type: &str, w: &mut dyn std::io::Write) -> anyhow::Result<usize> {
        assert!(self.supported_mime_types().contains(&mime_type));
        w.write_all(self.text.as_bytes())
            .context("could not write all")?;
        Ok(self.text.len())
    }
}

// TODO: rename this into EventLoop. but note that scale_factor and size methods most likely will
// need to be moved into a separate thing that will probably retain the name Window (or maybe it
// would make more sense to call it surface?).
pub trait Window: rwh::HasDisplayHandle + rwh::HasWindowHandle {
    // TODO: add timeout: Option<Duration>.
    // timeout limits how long it may block waiting for new events. a timeout of
    // Some(Duration::ZERO) = don't block; None means that it may wait indefinitely.
    //
    // TODO: as a replacement for pump_events + pop_event consider adding non-blocking poll_event
    // and wait_event(timeout).
    fn pump_events(&mut self) -> anyhow::Result<()>;
    fn pop_event(&mut self) -> Option<Event>;

    fn set_cursor_shape(&mut self, cursor_shape: input::CursorShape) -> anyhow::Result<()>;

    // NOTE: it is okay for read_clipboard and provide_clipboard_data methods to fail silently in
    // if clipboard is not-available.

    // if successful, this function will return the total number of bytes read (might be 0).
    fn read_clipboard(&mut self, mime_type: &str, buf: &mut Vec<u8>) -> anyhow::Result<usize>;
    // TODO: consider changing provider from being all boxed and ugly to an enum that would support
    // most common mime types as well as allow for providing manual/custom boxed providers.
    fn provide_clipboard_data(
        &mut self,
        data_provider: Box<dyn ClipboardDataProvider>,
    ) -> anyhow::Result<()>;
    // TODO: might need to introduce a method that would allow to list mime types that are
    // currently available for read from the clipboard.

    fn logical_size(&self) -> (u32, u32);
    fn scale_factor(&self) -> f64;
}

pub fn create_window(window_attrs: WindowAttrs) -> anyhow::Result<Box<dyn Window>> {
    let backend_hint = env::var("SHIN_WINDOW_BACKEND");
    match backend_hint.as_ref().map(|string| string.as_str()) {
        #[cfg(unix)]
        Ok("wayland") => return Ok(backend_wayland::WaylandBackend::new_boxed(window_attrs)?),
        #[cfg(feature = "winit")]
        Ok("winit") => return Ok(Box::new(backend_winit::WinitBackend::new(window_attrs)?)),
        _ => {}
    }

    let mut errors: Vec<anyhow::Error> = Vec::new();

    #[cfg(unix)]
    match backend_wayland::WaylandBackend::new_boxed(window_attrs.clone()) {
        Ok(wayland_window) => return Ok(wayland_window),
        Err(err) => errors.push(err),
    }

    #[cfg(target_family = "wasm")]
    match backend_web::WebBackend::new_boxed(window_attrs.clone()) {
        Ok(web_window) => return Ok(web_window),
        Err(err) => errors.push(err),
    }

    #[cfg(feature = "winit")]
    match backend_winit::WinitBackend::new(window_attrs.clone()) {
        Ok(winit_window) => return Ok(Box::new(winit_window)),
        Err(err) => errors.push(err),
    }

    #[cfg(not(any(unix, feature = "winit", target_family = "wasm")))]
    compile_error!("all window backend are disabled");

    Err(anyhow!("{errors:?}"))
}
