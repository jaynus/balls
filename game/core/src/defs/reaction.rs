#![allow(clippy::pub_enum_variant_names)]

use crate::defs::{
    condition::ConditionSetRef,
    item::{ItemDefinition, ItemRef},
    material::{MaterialLimit, MaterialState},
    DefinitionDetails, DefinitionResolver, DefinitionStorage,
};
use crate::legion::prelude::*;
use rl_macros::Definition;

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum ReactionCategory {
    MapTransformation,
    WorkshopProduction,
    Construction,
    PawnAction,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RandomParameters {
    #[serde(default = "RandomParameters::default_chance")]
    pub chance: f32,
    #[serde(default = "RandomParameters::default_per_count_modifier")]
    pub per_count_modifier: f32,
    #[serde(default = "RandomParameters::default_skill_modifier")]
    pub skill_modifier: f32,
}
impl RandomParameters {
    fn default_chance() -> f32 {
        1.0
    }
    fn default_per_count_modifier() -> f32 {
        1.0
    }
    fn default_skill_modifier() -> f32 {
        1.0
    }
}
impl Default for RandomParameters {
    fn default() -> Self {
        Self {
            chance: 1.0,
            per_count_modifier: 0.0,
            skill_modifier: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Reagent {
    #[serde(default)]
    pub conditions: Vec<ConditionSetRef>,
    #[serde(default)]
    pub consume_chance: u32,
    #[serde(default = "Reagent::default_count")]
    pub count: usize,
}
impl Reagent {
    fn default_materials() -> Vec<MaterialLimit> {
        vec![MaterialLimit::Any(MaterialState::Solid)]
    }
    const fn default_count() -> usize {
        1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ProductKind {
    Item(ItemRef),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Product {
    pub kind: ProductKind,
    #[serde(default = "Product::default_material")]
    pub material: MaterialLimit,
    #[serde(default = "Product::default_count")]
    pub count: usize,

    #[serde(default)]
    pub random: Option<RandomParameters>,
}
impl Product {
    const fn default_material() -> MaterialLimit {
        MaterialLimit::Source
    }
    const fn default_count() -> usize {
        1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EffectParameters {
    Todo, // TODO:
}

#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct ReactionEffect {
    pub name: String,
    #[serde(default)]
    pub parameters: Option<EffectParameters>,
}

#[derive(Definition, Debug, serde::Serialize, serde::Deserialize)]
#[definition(resolver = "Self")]
pub struct ReactionDefinition {
    pub details: DefinitionDetails,
    pub category: ReactionCategory,
    #[serde(skip)]
    pub id: ReactionDefinitionId,
    #[serde(default)]
    pub reagents: Vec<Reagent>,
    #[serde(default)]
    pub product: Option<Product>,
    #[serde(default)]
    pub effects: Vec<ReactionEffect>,
    #[serde(default)]
    pub duration: f64,
}

impl DefinitionResolver<Self> for ReactionDefinition {
    #[allow(irrefutable_let_patterns)]
    fn resolve(def: &mut Self, resources: &Resources) -> Result<(), anyhow::Error> {
        let items = resources
            .get::<DefinitionStorage<ItemDefinition>>()
            .unwrap();

        for reagent in &mut def.reagents {
            for condition in &mut reagent.conditions {
                condition.parse()?;
            }
        }

        if let Some(product) = def.product.as_mut() {
            if let ProductKind::Item(item) = &mut product.kind {
                item.resolve(&items)?;
            }
        }

        Ok(())
    }
}
