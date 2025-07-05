use std::{fmt, mem};

use glam::Vec2;

mod context;
mod drawbuffer;
mod fontservice;
mod layout;
mod renderer;
mod text;
mod texturepacker;
mod textureservice;

pub use context::*;
pub use drawbuffer::*;
pub use fontservice::*;
pub use layout::*;
pub use renderer::*;
pub use text::*;
pub use texturepacker::*;
pub use textureservice::*;

pub trait Externs {
    type TextureHandle: fmt::Debug + Clone;
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    pub fn top_left(&self) -> Vec2 {
        self.min
    }

    pub fn top_right(&self) -> Vec2 {
        Vec2::new(self.max.x, self.min.y)
    }

    pub fn bottom_left(&self) -> Vec2 {
        Vec2::new(self.min.x, self.max.y)
    }

    pub fn bottom_right(&self) -> Vec2 {
        self.max
    }

    pub fn set_top_left(&mut self, top_left: Vec2) {
        self.min = top_left;
    }

    pub fn set_top_right(&mut self, top_right: Vec2) {
        self.min = Vec2::new(self.min.x, top_right.y);
        self.max = Vec2::new(top_right.x, self.max.y);
    }

    pub fn set_bottom_right(&mut self, bottom_right: Vec2) {
        self.max = bottom_right;
    }

    pub fn set_bottom_left(&mut self, bottom_left: Vec2) {
        self.min = Vec2::new(bottom_left.x, self.min.y);
        self.max = Vec2::new(self.max.x, bottom_left.y);
    }

    pub fn from_center_size(center: Vec2, size: f32) -> Self {
        let radius = Vec2::splat(size / 2.0);
        Self {
            min: center - radius,
            max: center + radius,
        }
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    pub fn translate_by(&self, delta: &Vec2) -> Self {
        Self::new(self.min + *delta, self.max + *delta)
    }
}

// NOTE: repr(C) is here to ensure that ordering is correct in into_array transmutation.
#[repr(C)]
// NOTE: Copy is derived simply because it's cheap. size of u32.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba8 {
    // https://en.wikipedia.org/wiki/Web_colors#Basic_colors
    pub const WHITE: Self = Self::new(255, 255, 255, 255);
    pub const SILVER: Self = Self::new(192, 192, 192, 255);
    pub const GRAY: Self = Self::new(128, 128, 128, 255);
    pub const BLACK: Self = Self::new(0, 0, 0, 255);
    pub const RED: Self = Self::new(255, 0, 0, 255);
    pub const MAROON: Self = Self::new(128, 0, 0, 255);
    pub const YELLOW: Self = Self::new(255, 255, 0, 255);
    pub const OLIVE: Self = Self::new(128, 128, 0, 255);
    pub const LIME: Self = Self::new(0, 255, 0, 255);
    pub const GREEN: Self = Self::new(0, 128, 0, 255);
    pub const AQUA: Self = Self::new(0, 255, 255, 255);
    pub const TEAL: Self = Self::new(0, 128, 128, 255);
    pub const BLUE: Self = Self::new(0, 0, 255, 255);
    pub const NAVY: Self = Self::new(0, 0, 128, 255);
    pub const FUCHSIA: Self = Self::new(255, 0, 255, 255);
    pub const PURPLE: Self = Self::new(128, 0, 128, 255);
    // the missing rubik's cube color xd
    pub const ORANGE: Self = Self::new(255, 165, 0, 255);

    #[inline]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[inline]
    pub const fn from_bytes(arr: [u8; 4]) -> Self {
        unsafe { mem::transmute(arr) }
    }

    /// works with hex: `Rgba8::from_u32(0x8faf9fff)`.
    #[inline]
    pub const fn from_u32(value: u32) -> Self {
        Self::from_bytes(u32::to_be_bytes(value))
    }
}

#[derive(Debug, Clone)]
pub struct FillTexture<E: Externs> {
    pub kind: TextureKind<E>,
    pub coords: Rect,
}

#[derive(Debug, Clone)]
pub struct Fill<E: Externs> {
    pub color: Rgba8,
    pub texture: Option<FillTexture<E>>,
}

impl<E: Externs> Fill<E> {
    pub fn new(color: Rgba8, texture: FillTexture<E>) -> Self {
        Self {
            color,
            texture: Some(texture),
        }
    }

    // TODO: rename to something like new_color?
    pub fn with_color(color: Rgba8) -> Self {
        Self {
            color,
            texture: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Stroke {
    pub width: f32,
    pub color: Rgba8,
}

#[derive(Debug)]
pub struct RectShape<E: Externs> {
    pub coords: Rect,
    pub fill: Option<Fill<E>>,
    pub stroke: Option<Stroke>,
}

impl<E: Externs> RectShape<E> {
    pub fn new(coords: Rect, fill: Fill<E>, stroke: Stroke) -> Self {
        Self {
            coords,
            fill: Some(fill),
            stroke: Some(stroke),
        }
    }

    // TODO: rename to new_filled
    pub fn with_fill(coords: Rect, fill: Fill<E>) -> Self {
        Self {
            coords,
            fill: Some(fill),
            stroke: None,
        }
    }

    // TODO: rename to new_stroked
    pub fn with_stroke(coords: Rect, stroke: Stroke) -> Self {
        Self {
            coords,
            fill: None,
            stroke: Some(stroke),
        }
    }
}

#[derive(Debug)]
pub struct LineShape {
    pub points: [Vec2; 2],
    pub stroke: Stroke,
}

impl LineShape {
    pub fn new(a: Vec2, b: Vec2, stroke: Stroke) -> Self {
        Self {
            points: [a, b],
            stroke,
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct Vertex {
    /// screen pixel coordinates.
    /// 0, 0 is the top left corner of the screen.
    pub position: Vec2,
    /// normalized texture coordinates.
    /// 0, 0 is the top left corner of the texture.
    /// 1, 1 is the bottom right corner of the texture.
    pub tex_coord: Vec2,
    pub color: Rgba8,
}
