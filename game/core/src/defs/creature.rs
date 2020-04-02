use crate::{
    defs::{
        race::{RaceDefinition, RaceRef},
        DefinitionDetails, DefinitionResolver, DefinitionStorage,
    },
    legion::prelude::*,
};
use rl_macros::Definition;

#[derive(Definition, Debug, Clone, serde::Deserialize, serde::Serialize)]
#[definition(resolver = "Self")]
pub struct CreatureDefinition {
    pub details: DefinitionDetails,

    #[serde(skip)]
    pub id: CreatureDefinitionId,

    pub race: RaceRef,

    #[serde(default)]
    pub sprite: crate::defs::common::SpriteRef,

    pub decisions: Vec<String>,
}
impl DefinitionResolver<Self> for CreatureDefinition {
    fn resolve(def: &mut Self, resources: &Resources) -> Result<(), anyhow::Error> {
        let races = resources
            .get::<DefinitionStorage<RaceDefinition>>()
            .unwrap();

        def.race.resolve(&races)?;

        Ok(())
    }
}
