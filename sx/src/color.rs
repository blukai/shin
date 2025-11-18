use std::mem;

// NOTE: repr(C) is here to ensure that ordering is correct in into_array transmutation.
// NOTE: Copy is derived simply because it's cheap. size of Rgba == size of u32.
#[repr(C)]
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
    pub const fn from_u8_array(arr: [u8; 4]) -> Self {
        unsafe { mem::transmute(arr) }
    }

    /// works with hex: `Rgba8::from_u32(0x8faf9fff)`.
    #[inline]
    pub const fn from_u32(value: u32) -> Self {
        Self::from_u8_array(u32::to_be_bytes(value))
    }

    #[inline]
    pub const fn from_f32_array(arr: [f32; 4]) -> Self {
        Self::from_u8_array([
            (arr[0].clamp(0.0, 1.0) * 255.0) as u8,
            (arr[1].clamp(0.0, 1.0) * 255.0) as u8,
            (arr[2].clamp(0.0, 1.0) * 255.0) as u8,
            (arr[3].clamp(0.0, 1.0) * 255.0) as u8,
        ])
    }

    #[inline]
    pub const fn to_f32_array(self) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            self.a as f32 / 255.0,
        ]
    }

    pub const fn with_a(mut self, a: u8) -> Self {
        self.a = a;
        self
    }

    pub const fn with_af(mut self, a: f32) -> Self {
        debug_assert!(a >= 0.0 && a <= 1.0);
        self.a = (a * 255 as f32) as u8;
        self
    }
}

#[test]
fn test_rgba8_f32_conversions() {
    let arru8 = [255, 0, 0, 255];
    let arrf32 = [1.0, 0.0, 0.0, 1.0];
    assert_eq!(Rgba8::from_u8_array(arru8).to_f32_array(), arrf32);
    assert_eq!(Rgba8::from_f32_array(arrf32), Rgba8::from_u8_array(arru8));
}
