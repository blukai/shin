pub(crate) mod types {
    include!(concat!(env!("OUT_DIR"), "/gl_types.rs"));
}

#[allow(non_upper_case_globals)]
#[allow(dead_code)]
pub(crate) mod enums {
    use super::types::*;

    include!(concat!(env!("OUT_DIR"), "/gl_enums.rs"));
}

#[cfg(not(target_family = "wasm"))]
#[path = "gl_native.rs"]
mod gl_native;

#[cfg(target_family = "wasm")]
#[path = "gl_web.rs"]
mod gl_web;

pub use enums::*;
pub use types::*;

#[cfg(not(target_family = "wasm"))]
pub use gl_native::*;

#[cfg(target_family = "wasm")]
pub use gl_web::*;

// NOTE: i couldn't find any specifics on naming conventions in rust, thus i'm going to use what i
// used to: https://go.dev/doc/effective_go#interface-names
pub trait Contexter {
    unsafe fn clear_color(&self, red: f32, green: f32, blue: f32, alpha: f32);
    unsafe fn clear(&self, mask: u32);
}
