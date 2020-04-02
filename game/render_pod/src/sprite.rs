use crate::color::Color;
use shrinkwraprs::Shrinkwrap;
use slotmap::DenseSlotMap;
use strum_macros::EnumIter;
use ultraviolet::{Vec2, Vec3};

#[derive(
    EnumIter, Debug, Copy, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize,
)]
#[repr(u8)]
pub enum SpriteLayer {
    None = 254,
    Ground = 200,
    Building = 50,
    Item = 30,
    Foliage = 20,
    Creature = 11,
    Pawn = 10,
    Overlay = 1,
}
impl SpriteLayer {
    pub fn into_f32(self) -> f32 {
        let v: u8 = self as u8;
        (f32::from(v) / 255.0).min(0.9999)
    }
}

impl Default for SpriteLayer {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SpriteRenderMode {
    Single,
    Tiled,
}
impl Default for SpriteRenderMode {
    fn default() -> Self {
        Self::Single
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sprite {
    pub sprite_number: u32,
    pub color: Color,
    pub render: SpriteRenderMode,
}
impl Default for Sprite {
    fn default() -> Self {
        Self {
            sprite_number: 0,
            color: Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            render: SpriteRenderMode::Single,
        }
    }
}
impl Sprite {
    pub fn new(sprite_number: u32, color: Color) -> Self {
        Self {
            sprite_number,
            color,
            render: SpriteRenderMode::Single,
        }
    }
}

#[derive(Shrinkwrap, Debug, Clone, Copy, PartialEq)]
#[shrinkwrap(mutable)]
pub struct StaticSpriteTag(pub Sprite);

#[derive(Debug, Copy, Clone)]
pub struct SparseSprite {
    pub position: Vec3,
    pub u_offset: Vec2,
    pub v_offset: Vec2,
    pub color: Color,
    pub dir_x: Vec2,
    pub dir_y: Vec2,
}

slotmap::new_key_type! { pub struct SparseSpriteHandle; }

#[derive(shrinkwraprs::Shrinkwrap, Default, Debug, Clone)]
#[shrinkwrap(mutable)]
pub struct SparseSpriteArray(pub DenseSlotMap<SparseSpriteHandle, SparseSprite>);

pub mod sprite_map {
    pub const FLOOR: u32 = 176;
    pub const WALL: u32 = 219;
    pub const RAMP_UP: u32 = 30;
    pub const RAMP_DOWN: u32 = 31;
}
