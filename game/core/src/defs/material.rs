use crate::defs::{needs::Nutrition, DefinitionDetails, DefinitionResolver, DefinitionStorage};
use crate::{fxhash::FxHashMap, legion::prelude::*, strum_macros::EnumString};
use rl_macros::Definition;

#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    EnumString,
)]
#[strum(serialize_all = "snake_case")]
#[repr(u8)]
pub enum MaterialState {
    Solid,
    Powder,
    Paste,
    Liquid,
    Frozen,
    Gas,
    Any,
}

impl Default for MaterialState {
    fn default() -> Self {
        Self::Solid
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    EnumString,
)]
#[strum(serialize_all = "snake_case")]
pub enum RockSubKind {
    Igneous,
    Metamorphic,
    Sedimentary,
    Any,
}
impl Default for RockSubKind {
    fn default() -> Self {
        Self::Any
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    EnumString,
)]
#[strum(serialize_all = "snake_case")]
pub enum OrganicSubKind {
    Wood,
    Bone,
    Flesh,
    Any,
}
impl Default for OrganicSubKind {
    fn default() -> Self {
        Self::Any
    }
}

#[derive(
    Debug,
    Clone,
    Hash,
    PartialEq,
    Eq,
    Ord,
    PartialOrd,
    serde::Serialize,
    serde::Deserialize,
    EnumString,
)]
#[strum(serialize_all = "snake_case")]
pub enum MaterialKind {
    Rock(RockSubKind),
    Organic(OrganicSubKind),
    Liquid,
    Soil,
    Todo,
}
impl Default for MaterialKind {
    fn default() -> Self {
        Self::Todo
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum MaterialLimit {
    Any(MaterialState),
    Kind(MaterialKind),
    #[strum(disabled = "true")]
    Source, // Used for product reagents
    #[strum(disabled = "true")]
    Material(MaterialRef),
}
impl Default for MaterialLimit {
    fn default() -> Self {
        Self::Any(MaterialState::Solid)
    }
}
impl DefinitionResolver<Self> for MaterialLimit {
    fn resolve(def: &mut Self, resources: &Resources) -> Result<(), anyhow::Error> {
        let materials = resources
            .get::<DefinitionStorage<MaterialDefinition>>()
            .unwrap();

        if let Self::Material(material_ref) = def {
            material_ref.resolve(&materials)?;
        }

        Ok(())
    }
}

#[derive(Definition, Debug, serde::Serialize, serde::Deserialize)]
pub struct MaterialStateDefinition {
    details: DefinitionDetails,
    #[serde(skip)]
    id: MaterialStateDefinitionId,

    #[serde(default)]
    inherits: Option<MaterialStateRef>,

    #[serde(default)]
    pub density: i64,
    #[serde(default)]
    pub hardness: i64,
    #[serde(default)]
    pub porosity: i64,
    #[serde(default)]
    pub permeability: i64,
    #[serde(default)]
    pub elasticity: i64,
    #[serde(default)]
    pub tensile_strength: i64,
    #[serde(default)]
    pub tensile_yield: i64,
    #[serde(default)]
    pub compressive_yield_strength: i64,
    #[serde(default)]
    pub fatigue_strength: i64,
    #[serde(default)]
    pub fracture_toughness: i64,

    #[serde(default)]
    pub flexural_strength: i64,
    #[serde(default)]
    pub shear_modulus: i64,
    #[serde(default)]
    pub poisson_ratio: i64,
    #[serde(default)]
    pub impact_toughness: i64,
    #[serde(default)]
    pub electric_resistance: i64,
    #[serde(default)]
    pub specific_heat_capacity: i64,
    #[serde(default)]
    pub thermal_conductivity: i64,
    #[serde(default)]
    pub abrasive_hardness: i64,

    #[serde(default)]
    pub map_sprite: crate::defs::common::SpriteRef,
    #[serde(default)]
    pub item_sprite: crate::defs::common::SpriteRef,

    #[serde(default)]
    pub nutrition: Nutrition,
}

#[derive(Definition, Debug, serde::Serialize, serde::Deserialize)]
#[definition(component = "MaterialComponent")]
pub struct MaterialDefinition {
    pub details: DefinitionDetails,
    #[serde(skip)]
    id: MaterialDefinitionId,

    pub inherits: Option<MaterialRef>,

    pub category: MaterialKind,

    #[serde(default)]
    pub states: FxHashMap<MaterialState, MaterialStateDefinition>,

    #[serde(default)]
    pub melt_point: Option<i64>,

    #[serde(default)]
    pub boil_point: Option<i64>,

    #[serde(default)]
    pub ignite_point: Option<i64>,

    #[serde(default)]
    pub freeze_point: Option<i64>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MaterialComponent {
    def: MaterialDefinitionId,
    state: MaterialState,
}
impl MaterialComponent {
    pub fn new(def: MaterialDefinitionId, state: MaterialState) -> Self {
        Self { def, state }
    }

    pub fn fetch_state<'a>(
        &self,
        storage: &'a crate::defs::DefinitionStorage<MaterialDefinition>,
    ) -> &'a MaterialStateDefinition {
        self.def.fetch(storage).states.get(&self.state).unwrap()
    }
}
impl crate::defs::DefinitionComponent<MaterialDefinition> for MaterialComponent {
    fn id(&self) -> MaterialDefinitionId {
        self.def
    }

    fn fetch<'a>(
        &self,
        storage: &'a crate::defs::DefinitionStorage<MaterialDefinition>,
    ) -> &'a MaterialDefinition {
        self.def.fetch(storage)
    }
}
