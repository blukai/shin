use core::fmt;

// #[cfg(feature = "renderer_shingl")]
#[path = "renderer_shingl.rs"]
mod renderer_shingl;
pub use renderer_shingl::GlRenderer;

pub trait Renderer {
    type TextureHandle: fmt::Debug + Clone;
}
