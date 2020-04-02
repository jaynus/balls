#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::use_self
)]

use std::ops::{BitAnd, Mul};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}
impl Default for Color {
    fn default() -> Self {
        Self {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }
}
impl Color {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn r(r: f32) -> Self {
        Self {
            r,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }
    }

    pub fn g(g: f32) -> Self {
        Self {
            r: 0.0,
            g,
            b: 0.0,
            a: 0.0,
        }
    }

    pub fn b(b: f32) -> Self {
        Self {
            r: 0.0,
            g: 0.0,
            b,
            a: 0.0,
        }
    }

    pub fn a(a: f32) -> Self {
        Self {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a,
        }
    }

    pub fn empty() -> Self {
        Self {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        }
    }

    pub fn normalize(self) -> [u8; 4] {
        let mut out = [0; 4];

        out[0] = (self.r * 255.0).round() as u8;
        out[1] = (self.g * 255.0).round() as u8;
        out[2] = (self.b * 255.0).round() as u8;
        out[3] = (self.a * 255.0).round() as u8;

        out
    }

    pub fn pack(self) -> u32 {
        let normalized = self.normalize();
        (u32::from(normalized[3]) << 24)
            | (u32::from(normalized[2]) << 16)
            | (u32::from(normalized[1]) << 8)
            | u32::from(normalized[0])
    }

    pub fn as_slice(&self) -> &[f32] {
        unsafe { std::slice::from_raw_parts(self as *const Self as *mut f32, 4) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [f32] {
        unsafe { std::slice::from_raw_parts_mut(self as *mut Self as *mut f32, 4) }
    }
}

impl BitAnd for Color {
    type Output = Self;

    // rhs is the "right-hand side" of the expression `a & b`
    fn bitand(self, rhs: Self) -> Self::Output {
        let mut new = self;

        if rhs.r > 0.0 {
            new.r = rhs.r;
        }
        if rhs.g > 0.0 {
            new.g = rhs.g;
        }
        if rhs.b > 0.0 {
            new.b = rhs.b;
        }
        if rhs.a > 0.0 {
            new.a = rhs.a;
        }

        new
    }
}
impl Mul for Color {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Color {
            r: self.r * rhs.r,
            b: self.b * rhs.b,
            g: self.g * rhs.g,
            a: self.a * rhs.a,
        }
    }
}

impl Mul<f32> for Color {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Color {
            r: self.r * rhs,
            b: self.b * rhs,
            g: self.g * rhs,
            a: self.a * rhs,
        }
    }
}

impl From<[f32; 4]> for Color {
    fn from(color: [f32; 4]) -> Self {
        Self {
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        }
    }
}

impl From<[u8; 4]> for Color {
    #[allow(clippy::cast_lossless)]
    fn from(color: [u8; 4]) -> Self {
        Self {
            r: color[0] as f32 / 255.0,
            g: color[1] as f32 / 255.0,
            b: color[2] as f32 / 255.0,
            a: color[3] as f32 / 255.0,
        }
    }
}
