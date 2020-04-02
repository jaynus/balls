use enumflags2::BitFlags;
use rl_render_pod::sprite::{Sprite, SpriteRenderMode};
use strum_macros::{AsRefStr, EnumString};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpriteRef {
    #[serde(default = "SpriteRef::default_number")]
    pub number: Option<usize>,
    #[serde(default = "SpriteRef::default_color")]
    pub color: [u8; 4],
    #[serde(default = "SpriteRenderMode::default")]
    pub render: SpriteRenderMode,
}
impl Default for SpriteRef {
    fn default() -> Self {
        Self {
            number: Self::default_number(),
            color: Self::default_color(),
            render: SpriteRenderMode::default(),
        }
    }
}
impl SpriteRef {
    pub fn default_number() -> Option<usize> {
        Some(63)
    }

    pub fn default_color() -> [u8; 4] {
        [255, 0, 0, 255]
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn make(&self) -> Sprite {
        Sprite {
            sprite_number: self.number.map_or(63, |n| n as u32),
            color: self.color.into(),
            render: self.render,
        }
    }
}

#[derive(
    BitFlags,
    Debug,
    AsRefStr,
    Clone,
    Copy,
    Hash,
    PartialEq,
    Eq,
    serde::Deserialize,
    serde::Serialize,
    EnumString,
)]
#[strum(serialize_all = "snake_case")]
#[repr(u32)]
pub enum Property {
    Test = 0b1000_0000_0000_0000_0000_0000_0000_0000,
}
