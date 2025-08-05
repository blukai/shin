use std::fmt;

mod context;
mod drawbuffer;
mod fontservice;
mod geometry;
mod layout;
mod renderer;
mod text;
mod texturepacker;
mod textureservice;

pub use context::*;
pub use drawbuffer::*;
pub use fontservice::*;
pub use geometry::*;
pub use layout::*;
pub use renderer::*;
pub use text::*;
pub use texturepacker::*;
pub use textureservice::*;

pub trait Externs {
    type TextureHandle: fmt::Debug + Clone;
}

/// use this in tests.
pub struct UnitExterns;

impl Externs for UnitExterns {
    type TextureHandle = ();
}

// TODO: so many things derive Debug. almost none need it!
