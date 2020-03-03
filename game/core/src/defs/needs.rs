use std::ops::Range;
use strum_macros::{AsStaticStr, EnumCount, EnumIter};

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub enum ProvidesNutrition {
    FromMaterial,
    Value(Nutrition),
}
impl Default for ProvidesNutrition {
    fn default() -> Self {
        Self::FromMaterial
    }
}

#[derive(Debug, Clone, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub struct Nutrition {
    #[serde(default = "Nutrition::default_calories")]
    pub calories: Range<i32>,
    #[serde(default = "Nutrition::default_hydration")]
    pub hydration: Range<i32>,
}
impl Default for Nutrition {
    fn default() -> Self {
        Self {
            calories: Self::default_calories(),
            hydration: Self::default_hydration(),
        }
    }
}
impl Nutrition {
    fn default_calories() -> Range<i32> {
        Range { start: 0, end: 0 }
    }
    fn default_hydration() -> Range<i32> {
        Range { start: 0, end: 0 }
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Deserialize,
    serde::Serialize,
    EnumCount,
    EnumIter,
    AsStaticStr,
)]
#[repr(u8)]
pub enum NeedKind {
    Calories = 0,
    Hydration = 1,
    Sleep = 2,
}
impl NeedKind {
    pub fn as_usize(self) -> usize {
        self as usize
    }
}
