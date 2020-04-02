use num_traits::Float;
use std::ops::{Add, Mul, Sub};
pub use ultraviolet::{geometry::Aabbi, *};

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "Vec3i")]
pub struct Vec3iProxy {
    x: i32,
    y: i32,
    z: i32,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "Vec3u")]
pub struct Vec3uProxy {
    x: u32,
    y: u32,
    z: u32,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "Vec3")]
pub struct Vec3Proxy {
    x: f32,
    y: f32,
    z: f32,
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(remote = "geometry::Aabbi")]
pub struct AabbiProxy {
    #[serde(with = "Vec3iProxy")]
    min: Vec3i,
    #[serde(with = "Vec3iProxy")]
    max: Vec3i,
}

pub trait Lerp<F> {
    fn lerp(self, other: Self, t: F) -> Self;
}

impl<T, F> Lerp<F> for T
where
    T: Copy + Add<Output = T> + Sub<Output = T> + Mul<F, Output = T>,
    F: Float,
{
    fn lerp(self, other: Self, t: F) -> Self {
        self + ((other - self) * t)
    }
}
