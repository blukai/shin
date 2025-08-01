use std::ops;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl ops::Add<Vec2> for Vec2 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x.add(rhs.x),
            y: self.y.add(rhs.y),
        }
    }
}

impl ops::Sub<Vec2> for Vec2 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x.sub(rhs.x),
            y: self.y.sub(rhs.y),
        }
    }
}

impl ops::Mul<Vec2> for Vec2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            x: self.x.mul(rhs.x),
            y: self.y.mul(rhs.y),
        }
    }
}

impl ops::Div<Vec2> for Vec2 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self {
        Self {
            x: self.x.div(rhs.x),
            y: self.y.div(rhs.y),
        }
    }
}

impl ops::Mul<f32> for Vec2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self {
            x: self.x.mul(rhs),
            y: self.y.mul(rhs),
        }
    }
}

impl ops::Div<f32> for Vec2 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self {
        Self {
            x: self.x.div(rhs),
            y: self.y.div(rhs),
        }
    }
}

impl From<(f32, f32)> for Vec2 {
    fn from((x, y): (f32, f32)) -> Self {
        Self::new(x, y)
    }
}

impl From<F64Vec2> for Vec2 {
    fn from(value: F64Vec2) -> Self {
        Self::new(value.x as f32, value.y as f32)
    }
}

impl From<U32Vec2> for Vec2 {
    fn from(value: U32Vec2) -> Self {
        Self::new(value.x as f32, value.y as f32)
    }
}

impl Vec2 {
    pub const ZERO: Self = Self::splat(0.0);

    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn splat(v: f32) -> Self {
        Self { x: v, y: v }
    }

    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        (self.x * rhs.x) + (self.y * rhs.y)
    }

    /// computes the length (magnitude) of the vector.
    #[inline]
    pub fn length(self) -> f32 {
        f32::sqrt(self.dot(self))
    }

    /// returns `self` normalized to length 1 if possible, else returns zero.
    /// in particular, if the input is zero, or non-finite, the result of
    /// this operation will be zero.
    #[inline]
    pub fn normalize_or_zero(self) -> Self {
        // reciprocal is also called multiplicative inverse
        let reciprocal_length = 1.0 / self.length();
        if reciprocal_length.is_finite() && reciprocal_length > 0.0 {
            self * reciprocal_length
        } else {
            Self::splat(0.0)
        }
    }

    #[inline]
    pub fn perp(self) -> Self {
        Self {
            x: -self.y,
            y: self.x,
        }
    }

    #[inline]
    pub fn min(self, rhs: Self) -> Self {
        Self {
            x: if self.x < rhs.x { self.x } else { rhs.x },
            y: if self.y < rhs.y { self.y } else { rhs.y },
        }
    }

    #[inline]
    pub fn max(self, rhs: Self) -> Self {
        Self {
            x: if self.x > rhs.x { self.x } else { rhs.x },
            y: if self.y > rhs.y { self.y } else { rhs.y },
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct F64Vec2 {
    pub x: f64,
    pub y: f64,
}

impl From<(f64, f64)> for F64Vec2 {
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl F64Vec2 {
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct U32Vec2 {
    pub x: u32,
    pub y: u32,
}

impl From<(u32, u32)> for U32Vec2 {
    fn from((x, y): (u32, u32)) -> Self {
        Self::new(x, y)
    }
}

impl U32Vec2 {
    #[inline]
    pub const fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }
}

// ----

#[derive(Debug, Default, Clone, Copy, PartialEq)]
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

    pub fn translate_by(self, delta: &Vec2) -> Self {
        Self::new(self.min + *delta, self.max + *delta)
    }

    pub fn contains(&self, point: &Vec2) -> bool {
        let x = point.x >= self.min.x && point.x <= self.max.x;
        let y = point.y >= self.min.y && point.y <= self.max.y;
        x && y
    }

    pub fn shrink(self, amount: &Vec2) -> Self {
        Self::new(self.min + *amount, self.max - *amount)
    }

    pub fn expand(self, amount: &Vec2) -> Self {
        Self::new(self.min - *amount, self.max + *amount)
    }
}
