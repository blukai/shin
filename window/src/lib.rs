use anyhow::anyhow;
use raw_window_handle as rwh;

#[cfg(unix)]
pub mod libwayland_client;
#[cfg(unix)]
pub mod libxkbcommon;

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
    canvas_id: Option<Box<str>>,
    logical_size: Option<(u32, u32)>,
}

#[derive(Debug)]
pub enum WindowEvent {
    Configure { logical_size: (u32, u32) },
    CloseRequested,
}

pub trait Window: rwh::HasDisplayHandle + rwh::HasWindowHandle {
    fn pump_events(&mut self) -> anyhow::Result<()>;
    fn pop_event(&mut self) -> Option<WindowEvent>;
}

pub fn create_window(window_attrs: WindowAttrs) -> anyhow::Result<Box<dyn Window>> {
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
