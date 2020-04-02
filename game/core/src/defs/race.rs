#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::wildcard_imports,
    clippy::missing_safety_doc,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]

use crate::{
    defs::{
        body::{BodyDefinition, BodyRef},
        DefinitionDetails, DefinitionResolver, DefinitionStorage,
    },
    legion::prelude::*,
    rand, rand_distr,
};
use rl_macros::Definition;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
pub struct Attributes {
    // Body
    pub strength: u16,
    pub agility: u16,
    pub toughness: u16,
    pub endurance: u16,
    pub immunity: u16,
    pub healing: u16,

    // Mental
    pub analytical: u16,
    pub focus: u16,
    pub willpower: u16,
    pub creativity: u16,
    pub intuition: u16,
    pub patience: u16,
    pub memory: u16,
    pub linguistic: u16,
    pub spatial: u16,
    pub kinesthetic: u16,
    pub empathy: u16,
    pub social: u16,
}
impl Attributes {
    #[allow(clippy::too_many_lines)]
    pub fn generate<R>(rng: &mut R, def: &RaceDefinition) -> Self
    where
        R: rand::Rng,
    {
        use rand_distr::{Distribution, Normal};

        Self {
            strength: Normal::new(
                f32::from(def.attributes.base.strength),
                f32::from(def.attributes.deviation.strength),
            )
            .unwrap()
            .sample(rng) as u16,
            agility: Normal::new(
                f32::from(def.attributes.base.agility),
                f32::from(def.attributes.deviation.agility),
            )
            .unwrap()
            .sample(rng) as u16,
            toughness: Normal::new(
                f32::from(def.attributes.base.toughness),
                f32::from(def.attributes.deviation.toughness),
            )
            .unwrap()
            .sample(rng) as u16,
            endurance: Normal::new(
                f32::from(def.attributes.base.endurance),
                f32::from(def.attributes.deviation.endurance),
            )
            .unwrap()
            .sample(rng) as u16,
            immunity: Normal::new(
                f32::from(def.attributes.base.immunity),
                f32::from(def.attributes.deviation.immunity),
            )
            .unwrap()
            .sample(rng) as u16,
            healing: Normal::new(
                f32::from(def.attributes.base.healing),
                f32::from(def.attributes.deviation.healing),
            )
            .unwrap()
            .sample(rng) as u16,
            analytical: Normal::new(
                f32::from(def.attributes.base.analytical),
                f32::from(def.attributes.deviation.analytical),
            )
            .unwrap()
            .sample(rng) as u16,
            focus: Normal::new(
                f32::from(def.attributes.base.focus),
                f32::from(def.attributes.deviation.focus),
            )
            .unwrap()
            .sample(rng) as u16,
            willpower: Normal::new(
                f32::from(def.attributes.base.willpower),
                f32::from(def.attributes.deviation.willpower),
            )
            .unwrap()
            .sample(rng) as u16,
            creativity: Normal::new(
                f32::from(def.attributes.base.creativity),
                f32::from(def.attributes.deviation.creativity),
            )
            .unwrap()
            .sample(rng) as u16,
            intuition: Normal::new(
                f32::from(def.attributes.base.intuition),
                f32::from(def.attributes.deviation.intuition),
            )
            .unwrap()
            .sample(rng) as u16,
            patience: Normal::new(
                f32::from(def.attributes.base.patience),
                f32::from(def.attributes.deviation.patience),
            )
            .unwrap()
            .sample(rng) as u16,
            memory: Normal::new(
                f32::from(def.attributes.base.memory),
                f32::from(def.attributes.deviation.memory),
            )
            .unwrap()
            .sample(rng) as u16,
            linguistic: Normal::new(
                f32::from(def.attributes.base.linguistic),
                f32::from(def.attributes.deviation.linguistic),
            )
            .unwrap()
            .sample(rng) as u16,
            spatial: Normal::new(
                f32::from(def.attributes.base.spatial),
                f32::from(def.attributes.deviation.spatial),
            )
            .unwrap()
            .sample(rng) as u16,
            kinesthetic: Normal::new(
                f32::from(def.attributes.base.kinesthetic),
                f32::from(def.attributes.deviation.kinesthetic),
            )
            .unwrap()
            .sample(rng) as u16,
            empathy: Normal::new(
                f32::from(def.attributes.base.empathy),
                f32::from(def.attributes.deviation.empathy),
            )
            .unwrap()
            .sample(rng) as u16,
            social: Normal::new(
                f32::from(def.attributes.base.social),
                f32::from(def.attributes.deviation.social),
            )
            .unwrap()
            .sample(rng) as u16,
        }
    }

    pub fn default_deviation() -> Self {
        Self {
            strength: 200,
            agility: 200,
            toughness: 200,
            endurance: 200,
            immunity: 200,
            healing: 200,
            analytical: 200,
            focus: 200,
            willpower: 200,
            creativity: 200,
            intuition: 200,
            patience: 200,
            memory: 200,
            linguistic: 200,
            spatial: 200,
            kinesthetic: 200,
            empathy: 200,
            social: 200,
        }
    }

    pub fn default_with_deviation() -> RaceAttributes {
        RaceAttributes {
            base: Self::default(),
            deviation: Self::default_deviation(),
        }
    }
}
impl Default for Attributes {
    fn default() -> Self {
        Self {
            strength: 1000,
            agility: 1000,
            toughness: 1000,
            endurance: 1000,
            immunity: 1000,
            healing: 1000,
            analytical: 1000,
            focus: 1000,
            willpower: 1000,
            creativity: 1000,
            intuition: 1000,
            patience: 1000,
            memory: 1000,
            linguistic: 1000,
            spatial: 1000,
            kinesthetic: 1000,
            empathy: 1000,
            social: 1000,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub struct RaceAttributes {
    pub base: Attributes,
    pub deviation: Attributes,
}

#[derive(Definition, Debug, Clone, serde::Deserialize, serde::Serialize)]
#[definition(resolver = "Self")]
pub struct RaceDefinition {
    pub details: DefinitionDetails,

    #[serde(skip)]
    pub id: RaceDefinitionId,

    pub body: BodyRef,

    #[serde(default)]
    pub sprite: crate::defs::common::SpriteRef,

    #[serde(default = "Attributes::default_with_deviation")]
    pub attributes: RaceAttributes, //base, deviation
}
impl DefinitionResolver<Self> for RaceDefinition {
    fn resolve(def: &mut Self, resources: &Resources) -> Result<(), anyhow::Error> {
        let bodies = resources
            .get::<DefinitionStorage<BodyDefinition>>()
            .unwrap();

        def.body.resolve(&bodies)?;

        Ok(())
    }
}
