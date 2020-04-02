use crate::{
    data::{CollisionKind, DimensionsVec},
    defs::{
        material::{MaterialDefinition, MaterialRef},
        needs::ProvidesNutrition,
        DefinitionDetails, DefinitionResolver, DefinitionStorage,
    },
    legion::prelude::*,
};
use rl_macros::Definition;
use strum_macros::EnumString;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumString, serde::Serialize, serde::Deserialize,
)]
#[strum(serialize_all = "snake_case")]
pub enum FoliageKind {
    Tree,
    Bush,
    Grass,
}

#[derive(Definition, Debug, Clone, serde::Deserialize, serde::Serialize)]
#[definition(resolver = "Self")]
pub struct FoliageDefinition {
    pub details: DefinitionDetails,

    #[serde(skip)]
    pub id: FoliageDefinitionId,

    pub kind: FoliageKind,

    #[serde(default)]
    pub collision: CollisionKind,

    pub material: MaterialRef,

    #[serde(default)]
    pub sprite: crate::defs::common::SpriteRef,

    #[serde(default)]
    pub nutrition: ProvidesNutrition,

    #[serde(default)]
    pub dimensions: DimensionsVec,
}
impl DefinitionResolver<Self> for FoliageDefinition {
    fn resolve(def: &mut Self, resources: &Resources) -> Result<(), anyhow::Error> {
        let materials = resources
            .get::<DefinitionStorage<MaterialDefinition>>()
            .unwrap();

        def.material.resolve(&materials)?;

        Ok(())
    }
}
