use std::ffi::c_void;
use std::ptr::NonNull;

pub mod libwayland_client;
pub mod libxkbcommon;
mod platform_wayland;

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

pub const DEFAULT_LOGICAL_SIZE: Size = Size::new(800, 600);

#[derive(Debug, Default)]
pub struct WindowConfig {
    logical_size: Option<Size>,
}

#[derive(Debug)]
pub enum Event {
    Configure { logical_size: Size },
    CloseRequested,
}

pub trait EventLoop {
    fn display_handle(&self) -> NonNull<c_void>;
    fn window_handle(&self) -> NonNull<c_void>;
    fn update(&mut self);
    fn pop_event(&mut self) -> Option<Event>;
}

// NOCOMMIT
pub mod platform {
    pub mod wayland {
        pub use crate::platform_wayland::*;
    }
}
