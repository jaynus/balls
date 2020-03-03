use crate::bitflags_serial;
use crate::defs::{common::SpriteRef, DefinitionDetails};
use crate::math::{Vec3i, Vec3iProxy};
use bitflags::*;
use rl_macros::Definition;

bitflags_serial! {
    pub struct BuildingProperty: u32 {
        const IS_SEAT           =  0b1000_0000_0000_0000_0000_0000_0000_0000;
        const IS_EAT_ASSIST     =  0b0100_0000_0000_0000_0000_0000_0000_0000;
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub enum PlacementKind {
    Entity,
    Tile,
}

#[derive(Definition, Debug, serde::Deserialize, serde::Serialize)]
pub struct BuildingDefinition {
    pub details: DefinitionDetails,
    #[serde(skip)]
    pub id: BuildingDefinitionId,

    #[serde(default)]
    pub properties: Vec<BuildingProperty>,

    #[serde(default)]
    pub sprite: SpriteRef,
    #[serde(with = "Vec3iProxy")]
    pub dimensions: Vec3i,

    #[serde(default = "BuildingDefinition::default_placement")]
    pub placement: PlacementKind,
}
impl BuildingDefinition {
    pub fn default_placement() -> PlacementKind {
        PlacementKind::Entity
    }
}
