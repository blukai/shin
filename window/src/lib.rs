use raw_window_handle as rwh;

pub mod libwayland_client;
pub mod libxkbcommon;
mod platform_wayland;
#[cfg(feature = "winit")]
mod platform_winit;

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
    match platform_wayland::WaylandWindow::new_boxed(config.clone()) {
        Ok(wayland_window) => Ok(wayland_window),
        #[cfg(not(feature = "winit"))]
        Err(err) => Err(err),
        #[cfg(feature = "winit")]
        Err(err) => {
            log::error!("could not create wayland window (err: {err:?}), trying winit..");
            let winit_window = platform_winit::WinitWindow::new(config)?;
            Ok(Box::new(winit_window))
        }
    }
}
