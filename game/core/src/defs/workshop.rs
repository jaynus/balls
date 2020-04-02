use crate::defs::{
    building::{BuildingDefinition, BuildingRef},
    reaction::{ReactionDefinition, ReactionRef},
    DefinitionDetails, DefinitionResolver, DefinitionStorage,
};
use crate::legion::prelude::*;
use rl_macros::Definition;

#[derive(Definition, Debug, serde::Deserialize, serde::Serialize)]
#[definition(resolver = "Self")]
pub struct WorkshopDefinition {
    pub details: DefinitionDetails,
    #[serde(skip)]
    pub id: WorkshopDefinitionId,

    pub reactions: Vec<ReactionRef>,

    pub building: BuildingRef,
}
impl DefinitionResolver<Self> for WorkshopDefinition {
    fn resolve(def: &mut Self, resources: &Resources) -> Result<(), anyhow::Error> {
        let reactions = resources
            .get::<DefinitionStorage<ReactionDefinition>>()
            .unwrap();
        let buildings = resources
            .get::<DefinitionStorage<BuildingDefinition>>()
            .unwrap();

        for reaction in &mut def.reactions {
            reaction.resolve(&reactions)?;
        }
        def.building.resolve(&buildings)?;

        Ok(())
    }
}
