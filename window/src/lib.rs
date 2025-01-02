use anyhow::anyhow;
use raw_window_handle as rwh;

#[cfg(target_os = "linux")]
pub mod libwayland_client;
#[cfg(target_os = "linux")]
pub mod libxkbcommon;

#[cfg(target_os = "linux")]
mod backend_wayland;

#[cfg(feature = "winit")]
mod backend_winit;

#[cfg(target_family = "wasm")]
mod backend_web;

pub const DEFAULT_LOGICAL_SIZE: Size = Size::new(640, 480);

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// rounds away from zero
    pub fn to_physical(&self, scale_factor: f64) -> Self {
        Self {
            width: ((self.width as f64) * scale_factor).round() as u32,
            height: ((self.height as f64) * scale_factor).round() as u32,
        }
    }

    /// rounds away from zero
    pub fn to_logical(&self, scale_factor: f64) -> Self {
        Self {
            width: ((self.width as f64) / scale_factor).round() as u32,
            height: ((self.height as f64) / scale_factor).round() as u32,
        }
    }

    pub fn as_tuple(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[derive(Debug, Default, Clone)]
pub struct WindowConfig {
    logical_size: Option<Size>,
}

// TODO: rename to window event
#[derive(Debug)]
pub enum Event {
    Configure { logical_size: Size },
    CloseRequested,
}

pub trait Window: rwh::HasDisplayHandle + rwh::HasWindowHandle {
    fn update(&mut self) -> anyhow::Result<()>;
    fn pop_event(&mut self) -> Option<Event>;
}

pub fn create_window(config: WindowConfig) -> anyhow::Result<Box<dyn Window>> {
    let mut errors: Vec<anyhow::Error> = Vec::new();

    #[cfg(target_os = "linux")]
    match backend_wayland::WaylandWindow::new_boxed(config.clone()) {
        Ok(wayland_window) => return Ok(wayland_window),
        Err(err) => errors.push(err),
    }

    #[cfg(feature = "winit")]
    match backend_winit::WinitWindow::new(config.clone()) {
        Ok(winit_window) => return Ok(Box::new(winit_window)),
        Err(err) => errors.push(err),
    }

    #[cfg(not(any(target_os = "linux", feature = "winit", target_family = "wasm")))]
    compile_error!("all window backend are disabled");

    #[cfg(target_family = "wasm")]
    unimplemented!("wasm window");

    Err(anyhow!("{errors:?}"))
}
