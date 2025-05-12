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

pub const DEFAULT_LOGICAL_SIZE: (u32, u32) = (640, 480);

#[derive(Debug, Default, Clone)]
pub struct WindowConfig {
    logical_size: Option<(u32, u32)>,
}

#[derive(Debug)]
pub enum WindowEvent {
    Configure { logical_size: (u32, u32) },
    CloseRequested,
}

pub trait Window: rwh::HasDisplayHandle + rwh::HasWindowHandle {
    fn update(&mut self) -> anyhow::Result<()>;
    fn pop_event(&mut self) -> Option<WindowEvent>;
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

    #[cfg(target_family = "wasm")]
    unimplemented!("wasm window");

    #[cfg(not(any(target_os = "linux", feature = "winit", target_family = "wasm")))]
    compile_error!("all window backend are disabled");

    Err(anyhow!("{errors:?}"))
}
