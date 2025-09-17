use std::fmt;

mod drawbuffer;
mod fontservice;
mod geometry;
mod texturepacker;
mod textureservice;

pub use drawbuffer::*;
pub use fontservice::*;
pub use geometry::*;
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
