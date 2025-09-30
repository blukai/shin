use std::ops;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct BVec2 {
    pub x: bool,
    pub y: bool,
}

impl BVec2 {
    #[inline]
    pub const fn new(x: bool, y: bool) -> Self {
        Self { x, y }
    }

    #[inline]
    pub const fn splat(v: bool) -> Self {
        Self::new(v, v)
    }

    #[inline]
    pub fn any(self) -> bool {
        self.x || self.y
    }

    #[inline]
    pub fn all(self) -> bool {
        self.x && self.y
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl ops::Add<Vec2> for Vec2 {
    type Output = Self;

    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x.add(rhs.x), self.y.add(rhs.y))
    }
}

impl ops::Sub<Vec2> for Vec2 {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x.sub(rhs.x), self.y.sub(rhs.y))
    }
}

impl ops::Mul<Vec2> for Vec2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self::new(self.x.mul(rhs.x), self.y.mul(rhs.y))
    }
}

impl ops::Div<Vec2> for Vec2 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: Self) -> Self {
        Self::new(self.x.div(rhs.x), self.y.div(rhs.y))
    }
}

impl ops::AddAssign<Vec2> for Vec2 {
    fn add_assign(&mut self, rhs: Vec2) {
        self.x.add_assign(rhs.x);
        self.y.add_assign(rhs.y);
    }
}

impl ops::SubAssign<Vec2> for Vec2 {
    fn sub_assign(&mut self, rhs: Vec2) {
        self.x.sub_assign(rhs.x);
        self.y.sub_assign(rhs.y);
    }
}

impl ops::MulAssign<Vec2> for Vec2 {
    fn mul_assign(&mut self, rhs: Vec2) {
        self.x.mul_assign(rhs.x);
        self.y.mul_assign(rhs.y);
    }
}

impl ops::DivAssign<Vec2> for Vec2 {
    fn div_assign(&mut self, rhs: Vec2) {
        self.x.div_assign(rhs.x);
        self.y.div_assign(rhs.y);
    }
}

impl ops::Mul<f32> for Vec2 {
    type Output = Self;

    #[inline]
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.x.mul(rhs), self.y.mul(rhs))
    }
}

impl ops::Div<f32> for Vec2 {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self {
        Self::new(self.x.div(rhs), self.y.div(rhs))
    }
}

impl ops::Neg for Vec2 {
    type Output = Self;

    #[inline]
    fn neg(self) -> Self {
        Self::new(self.x.neg(), self.y.neg())
    }
}

impl From<(f32, f32)> for Vec2 {
    fn from((x, y): (f32, f32)) -> Self {
        Self::new(x, y)
    }
}

impl From<&(f32, f32)> for Vec2 {
    fn from((x, y): &(f32, f32)) -> Self {
        Self::new(*x, *y)
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
        Self::new(v, v)
    }

    // ----

    #[inline]
    pub fn with_x(mut self, x: f32) -> Self {
        self.x = x;
        self
    }

    #[inline]
    pub fn with_y(mut self, y: f32) -> Self {
        self.y = y;
        self
    }

    // ----

    #[inline]
    pub fn dot(self, rhs: Self) -> f32 {
        (self.x * rhs.x) + (self.y * rhs.y)
    }

    /// computes the length (magnitude) of the vector.
    #[inline]
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
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
        Self::new(-self.y, self.x)
    }

    #[inline]
    pub fn min(self, rhs: Self) -> Self {
        Self::new(self.x.min(rhs.x), self.y.min(rhs.y))
    }

    #[inline]
    pub fn max(self, rhs: Self) -> Self {
        Self::new(self.x.max(rhs.x), self.y.max(rhs.y))
    }

    #[inline]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        assert!(min.x <= max.x && min.y <= max.y);
        self.max(min).min(max)
    }

    #[inline]
    pub fn abs(self) -> Self {
        Self::new(self.x.abs(), self.y.abs())
    }

    #[inline]
    pub fn lt(self, rhs: Self) -> BVec2 {
        BVec2::new(self.x.lt(&rhs.x), self.y.lt(&rhs.y))
    }

    #[inline]
    pub fn le(self, rhs: Self) -> BVec2 {
        BVec2::new(self.x.le(&rhs.x), self.y.le(&rhs.y))
    }

    #[inline]
    pub fn gt(self, rhs: Self) -> BVec2 {
        BVec2::new(self.x.gt(&rhs.x), self.y.gt(&rhs.y))
    }

    #[inline]
    pub fn ge(self, rhs: Self) -> BVec2 {
        BVec2::new(self.x.ge(&rhs.x), self.y.ge(&rhs.y))
    }
}

// ----

// NOTE: F64Vec2 and U32Vec2 are sort of transient structs
//   i don't wnat to implement From<(f64, f64)> etc. for Vec2, i don't wnat things to be implicit.

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct F64Vec2 {
    pub x: f64,
    pub y: f64,
}

impl From<(f64, f64)> for F64Vec2 {
    #[inline]
    fn from((x, y): (f64, f64)) -> Self {
        Self::new(x, y)
    }
}

impl From<&(f64, f64)> for F64Vec2 {
    #[inline]
    fn from((x, y): &(f64, f64)) -> Self {
        Self::new(*x, *y)
    }
}

impl F64Vec2 {
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn as_vec2(&self) -> Vec2 {
        Vec2::new(self.x as f32, self.y as f32)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct U32Vec2 {
    pub x: u32,
    pub y: u32,
}

impl From<(u32, u32)> for U32Vec2 {
    #[inline]
    fn from((x, y): (u32, u32)) -> Self {
        Self::new(x, y)
    }
}

impl From<&(u32, u32)> for U32Vec2 {
    #[inline]
    fn from((x, y): &(u32, u32)) -> Self {
        Self::new(*x, *y)
    }
}

impl U32Vec2 {
    #[inline]
    pub const fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn as_vec2(&self) -> Vec2 {
        Vec2::new(self.x as f32, self.y as f32)
    }
}

// ----

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}

impl Rect {
    #[inline]
    pub fn new(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }

    #[inline]
    pub fn from_center_half_size(center: Vec2, size: f32) -> Self {
        let radius = Vec2::splat(size / 2.0);
        Self::new(center - radius, center + radius)
    }

    // ----

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    // TODO: consider renaming this to `dimensions`.
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) / 2.0
    }

    pub fn contains(&self, point: Vec2) -> bool {
        let x_in_bounds = (point.x >= self.min.x) & (point.x <= self.max.x);
        let y_in_bounds = (point.y >= self.min.y) & (point.y <= self.max.y);
        x_in_bounds & y_in_bounds
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        let x_overlap = (self.min.x < other.max.x) & (self.max.x > other.min.x);
        let y_overlap = (self.min.y < other.max.y) & (self.max.y > other.min.y);
        x_overlap & y_overlap
    }

    pub fn translate(self, delta: Vec2) -> Self {
        Self::new(self.min + delta, self.max + delta)
    }

    pub fn inflate(self, amount: Vec2) -> Self {
        Self::new(self.min - amount, self.max + amount)
    }

    pub fn scale(self, amount: f32) -> Self {
        Self::new(self.min * amount, self.max * amount)
    }

    // NOTE: do no think of this as vector normalize or anything alike; - unrealted.
    //
    // TODO: think of a better name for this function that basically flips `min` and `max` if
    // needed, so that `min <= max`.
    pub fn normalize(self) -> Self {
        Self::new(self.min.min(self.max), self.min.max(self.max))
    }

    pub fn is_normalized(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y
    }

    pub fn clamp(self, other: Self) -> Self {
        assert_eq!(self, self.normalize());
        assert_eq!(other, other.normalize());
        Self::new(self.min.max(other.min), self.max.min(other.max))
    }

    // ----
    // sugary stuff

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

    pub fn with_top_left(mut self, top_left: Vec2) -> Self {
        self.min = top_left;
        self
    }

    pub fn with_top_right(mut self, top_right: Vec2) -> Self {
        self.min = Vec2::new(self.min.x, top_right.y);
        self.max = Vec2::new(top_right.x, self.max.y);
        self
    }

    pub fn with_bottom_right(mut self, bottom_right: Vec2) -> Self {
        self.max = bottom_right;
        self
    }

    pub fn with_bottom_left(mut self, bottom_left: Vec2) -> Self {
        self.min = Vec2::new(bottom_left.x, self.min.y);
        self.max = Vec2::new(self.max.x, bottom_left.y);
        self
    }
}

#[test]
fn test_rect_contains() {
    let rect = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(2.0, 2.0));

    let inside = Vec2::new(1.0, 1.0);
    assert!(rect.contains(inside));

    // NOTE: contains is inclusive
    let on_edge = Vec2::new(2.0, 2.0);
    assert!(rect.contains(on_edge));

    let outside = Vec2::new(3.0, 3.0);
    assert!(!rect.contains(outside));
}

#[test]
fn test_rect_intersects() {
    let rect = Rect::new(Vec2::new(0.0, 0.0), Vec2::new(2.0, 2.0));

    let overlapping = Rect::new(Vec2::new(1.0, 1.0), Vec2::new(3.0, 3.0));
    assert!(rect.intersects(&overlapping));

    let touching = Rect::new(Vec2::new(2.0, 0.0), Vec2::new(4.0, 2.0));
    assert!(!rect.intersects(&touching));

    let non_overlapping = Rect::new(Vec2::new(3.0, 3.0), Vec2::new(4.0, 4.0));
    assert!(!rect.intersects(&non_overlapping));
}
