use std::env;

use anyhow::anyhow;
use raw_window_handle as rwh;

#[cfg(unix)]
pub mod libwayland_client;
#[cfg(unix)]
pub mod libwayland_cursor;
#[cfg(unix)]
pub mod libxkbcommon;

#[cfg(unix)]
pub mod xkb;

#[cfg(unix)]
mod backend_wayland;

#[cfg(feature = "winit")]
mod backend_winit;

#[cfg(target_family = "wasm")]
mod backend_web;
#[cfg(target_family = "wasm")]
pub use backend_web::js_sys;

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

#[derive(Debug)]
pub enum WindowEvent {
    Configure { logical_size: (u32, u32) },
    Resized { physical_size: (u32, u32) },
    ScaleFactorChanged { scale_factor: f64 },
    CloseRequested,
}

#[derive(Debug)]
pub enum Event {
    Window(WindowEvent),
    Pointer(input::PointerEvent),
    Keyboard(input::KeyboardEvent),
}

pub trait Window: rwh::HasDisplayHandle + rwh::HasWindowHandle {
    fn pump_events(&mut self) -> anyhow::Result<()>;
    fn pop_event(&mut self) -> Option<Event>;
    fn set_cursor_shape(&mut self, cursor_shape: input::CursorShape) -> anyhow::Result<()>;
    fn scale_factor(&self) -> f64;
    fn size(&self) -> (u32, u32);
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
