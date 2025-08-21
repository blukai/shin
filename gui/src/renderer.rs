use core::fmt;

// #[cfg(feature = "renderer_gl")]
#[path = "renderer_gl.rs"]
mod renderer_gl;
pub use renderer_gl::GlRenderer;

pub trait Renderer {
    type TextureHandle: fmt::Debug + Clone;
}
