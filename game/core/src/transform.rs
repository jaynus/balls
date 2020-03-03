use shrinkwraprs::Shrinkwrap;
use ultraviolet::Vec3;

#[derive(Debug, Clone, Copy, Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct Translation(pub Vec3);
impl Translation {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }

    pub fn zero() -> Self {
        Self(Vec3::new(0.0, 0.0, 0.0))
    }
}

impl From<Vec3> for Translation {
    fn from(other: Vec3) -> Self {
        Self(other)
    }
}
impl From<Translation> for Vec3 {
    fn from(other: Translation) -> Self {
        other.0
    }
}

#[derive(Clone, Copy, Debug, Shrinkwrap)]
#[shrinkwrap(mutable)]
pub struct Scale(pub f32);
impl Scale {
    pub fn new(scale: f32) -> Self {
        Self(scale)
    }

    pub fn identity() -> Self {
        Self(1.0)
    }
}
impl Default for Scale {
    fn default() -> Self {
        Self::identity()
    }
}
