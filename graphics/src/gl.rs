#[cfg(not(target_arch = "wasm32"))]
#[path = "gl_native.rs"]
mod gl_native;
#[cfg(not(target_arch = "wasm32"))]
pub use gl_native::*;

#[cfg(target_arch = "wasm32")]
#[path = "gl_web.rs"]
mod gl_web;
#[cfg(target_arch = "wasm32")]
pub use gl_web::*;
