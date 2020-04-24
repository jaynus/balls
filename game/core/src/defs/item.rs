use crate::defs::{
    common::SpriteRef, material::MaterialLimit, needs::ProvidesNutrition, DefinitionDetails,
    DefinitionResolver,
};
use crate::{
    data::DimensionsVec,
    legion::prelude::*,
    math::Vec3i,
    shrinkwrap::Shrinkwrap,
    smallvec::SmallVec,
    strum_macros::{AsRefStr, EnumDiscriminants, EnumString},
    FreeList,
};
use enumflags2::BitFlags;
use rl_macros::Definition;

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
pub enum ItemProperty {
    IsEdible = 0b1000_0000_0000_0000_0000_0000_0000_0000,
    IsFlammable = 0b0100_0000_0000_0000_0000_0000_0000_0000,
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
pub enum ItemKind {
    Weapon = 0b1000_0000_0000_0000_0000_0000_0000_0000,
    Tool = 0b0100_0000_0000_0000_0000_0000_0000_0000,
    Stone = 0b0010_0000_0000_0000_0000_0000_0000_0000,
    Wood = 0b0001_0000_0000_0000_0000_0000_0000_0000,
    Trash = 0b0000_1000_0000_0000_0000_0000_0000_0000,
    Apparel = 0b0000_0100_0000_0000_0000_0000_0000_0000,
    Food = 0b0000_0010_0000_0000_0000_0000_0000_0000,
    Other = 0b0000_0000_0000_0000_0000_0000_0000_0001,
}
impl Default for ItemKind {
    fn default() -> Self {
        Self::Other
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
pub enum ItemAbilityKind {
    Digging = 0b1000_0000_0000_0000_0000_0000_0000_0000,
    Chopping = 0b0100_0000_0000_0000_0000_0000_0000_0000,
    Hammering = 0b0010_0000_0000_0000_0000_0000_0000_0000,
    Cutting = 0b0001_0000_0000_0000_0000_0000_0000_0000,
    Sawing = 0b0000_1000_0000_0000_0000_0000_0000_0000,
}

#[derive(Debug, Copy, Clone, serde::Deserialize, serde::Serialize)]
pub struct ItemAbility {
    pub kind: ItemAbilityKind,
    pub quality: u32,
}
impl ItemAbility {
    pub fn new(kind: ItemAbilityKind, quality: u32) -> Self {
        Self { kind, quality }
    }
}
impl PartialEq for ItemAbility {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
    }
}
impl Eq for ItemAbility {}
impl std::hash::Hash for ItemAbility {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.kind.hash(state);
    }
}
impl From<ItemAbilityKind> for ItemAbility {
    fn from(other: ItemAbilityKind) -> Self {
        Self {
            kind: other,
            quality: 0,
        }
    }
}
impl From<&ItemAbilityKind> for ItemAbility {
    fn from(other: &ItemAbilityKind) -> Self {
        Self {
            kind: *other,
            quality: 0,
        }
    }
}

#[derive(
    Debug, AsRefStr, EnumDiscriminants, Clone, Copy, Hash, serde::Deserialize, serde::Serialize,
)]
#[strum_discriminants(name(ItemExtensionKind))]
pub enum ItemExtension {
    Container { capacity: DimensionsVec },
}

#[derive(Definition, Debug, serde::Deserialize, serde::Serialize)]
#[definition(resolver = "Self")]
pub struct ItemDefinition {
    pub details: DefinitionDetails,
    #[serde(skip)]
    pub id: ItemDefinitionId,

    #[serde(default)]
    pub material_limits: Vec<MaterialLimit>,

    #[serde(default)]
    pub abilities: Vec<ItemAbility>,

    #[serde(default)]
    pub properties: BitFlags<ItemProperty>,

    #[serde(default)]
    pub sprite: SpriteRef,

    #[serde(default)]
    pub kind: ItemKind,

    #[serde(default)]
    pub base_status: ItemStatusComponent,

    #[serde(default)]
    pub extensions: SmallVec<[ItemExtension; 3]>,

    pub dimensions: DimensionsVec,

    #[serde(default = "ItemDefinition::default_weight")]
    pub weight: u64,

    #[serde(default)]
    pub nutrition: ProvidesNutrition,
}
impl ItemDefinition {
    pub fn has_extension(&self, variant: ItemExtensionKind) -> bool {
        self.extensions.iter().any(|ext| variant == ext.into())
    }

    pub fn get_extension(&self, variant: ItemExtensionKind) -> Option<&ItemExtension> {
        self.extensions.iter().find(|ext| variant == (*ext).into())
    }

    fn default_weight() -> u64 {
        10
    }
}
impl DefinitionResolver<Self> for ItemDefinition {
    fn resolve(def: &mut Self, resources: &Resources) -> Result<(), anyhow::Error> {
        for limit in &mut def.material_limits {
            MaterialLimit::resolve(limit, resources)?;
        }

        Ok(())
    }
}

#[derive(Shrinkwrap, Default)]
#[shrinkwrap(mutable)]
pub struct StockpileSpatialMap(pub crate::rstar::RTree<crate::map::spatial::SpatialMapEntry>);

#[derive(Debug, Clone)]
pub struct StockpileComponent {
    pub tiles: FreeList<Vec3i>,
    pub stores: BitFlags<ItemKind>,
    pub contains: Vec<Entity>,
    pub children_tiles: Vec<Entity>,
}
impl StockpileComponent {
    pub fn new(stores: BitFlags<ItemKind>, tiles: Vec<Vec3i>) -> Self {
        use std::iter::FromIterator;

        Self {
            contains: Vec::with_capacity(tiles.len()),
            children_tiles: Vec::with_capacity(tiles.len()),
            stores,
            tiles: FreeList::from_iter(tiles),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StockpileItemChildComponent {
    pub parent: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StockpileTileChildComponent {
    pub parent: Entity,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, serde::Deserialize, serde::Serialize)]
pub struct ItemStatusComponent {
    quality: u32,
    durability: u32,
}
impl Default for ItemStatusComponent {
    fn default() -> Self {
        Self {
            quality: 100,
            durability: 1000,
        }
    }
}
