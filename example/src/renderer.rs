use core::fmt;

#[path = "renderer_gl.rs"]
mod renderer_gl;

pub use renderer_gl::*;

pub trait Renderer {
    type TextureHandle: fmt::Debug + Clone;
}
