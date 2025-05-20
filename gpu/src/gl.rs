pub(crate) mod types {
    include!(concat!(env!("OUT_DIR"), "/gl_types_generated.rs"));
}

#[allow(non_upper_case_globals)]
#[allow(dead_code)]
pub(crate) mod enums {
    use super::types::*;

    include!(concat!(env!("OUT_DIR"), "/gl_enums_generated.rs"));
}

#[cfg(not(target_family = "wasm"))]
#[path = "gl46.rs"]
mod gl46;

#[cfg(target_family = "wasm")]
#[path = "webgl2.rs"]
mod webgl2;

pub use enums::*;
#[cfg(not(target_family = "wasm"))]
pub use gl46::*;
pub use types::*;
#[cfg(target_family = "wasm")]
pub use webgl2::*;

// NOTE: i couldn't find any specifics on naming conventions in rust, thus i'm going to use what i
// used to: https://go.dev/doc/effective_go#interface-names
pub trait GlContexter {
    unsafe fn clear_color(&self, red: f32, green: f32, blue: f32, alpha: f32);
    unsafe fn clear(&self, mask: u32);
}
