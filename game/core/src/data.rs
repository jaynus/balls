use crate::{
    components::{DimensionsComponent, MovementRequest},
    defs::{material::MaterialComponent, race::RaceDefinitionId},
    map::Map,
    math::{Vec3, Vec3i, Vec3u, Vec3uProxy},
};
use auto_ops::impl_op;
use legion::entity::Entity;
use std::convert::TryFrom;
use strum_macros::EnumDiscriminants;
pub type PartGraphId = petgraph::graph::NodeIndex<u32>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CollisionKind {
    Solid,
    None,
}
impl CollisionKind {
    pub fn is_walkable(&self) -> bool {
        *self != Self::Solid
    }
}
impl Default for CollisionKind {
    fn default() -> Self {
        Self::None
    }
}

pub mod bt {
    use super::*;

    #[derive(Copy, Clone, Debug)]
    pub enum Target {
        Entity(Entity),
        Tile(Vec3i),
    }

    #[derive(Copy, Clone, Debug)]
    pub struct DropParameters {
        pub item: Entity,
        pub target: Option<Target>,
    }
    impl DropParameters {
        pub fn new(item: Entity) -> Self {
            Self { item, target: None }
        }
        pub fn with_target(item: Entity, target: Target) -> Self {
            Self {
                item,
                target: Some(target),
            }
        }
    }

    #[derive(Copy, Clone, Debug)]
    pub struct HaulParameters {
        pub stockpile: Entity,
        pub item: Entity,
        pub target_tile: Vec3i,
    }

    #[derive(Copy, Clone, Debug)]
    pub enum PickupDestination {
        Container(Entity),
        Carry(PartGraphId),
    }

    #[derive(Copy, Clone, Debug)]
    pub struct PickupParameters {
        pub target: Entity,
        pub destination: Option<PickupDestination>,
    }
    impl PickupParameters {
        pub fn new(target: Entity) -> Self {
            Self {
                target,
                destination: None,
            }
        }
    }

    #[derive(Copy, Clone, Debug)]
    pub struct MoveParameters {
        pub target: Target,
        pub active_request: Option<MovementRequest>,
    }
    impl MoveParameters {
        pub fn new_entity(entity: Entity) -> Self {
            Self {
                target: Target::Entity(entity),
                active_request: None,
            }
        }

        pub fn new_tile(tile: Vec3i) -> Self {
            Self {
                target: Target::Tile(tile),
                active_request: None,
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SpawnPosition {
    World(Vec3),
    Tile(Vec3i),
}
impl SpawnPosition {
    pub fn from_map(&self, map: &Map) -> (Vec3, Vec3i) {
        match self {
            SpawnPosition::Tile(tile) => (map.tile_to_world(*tile), *tile),
            SpawnPosition::World(world) => (*world, map.world_to_tile(*world)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SpawnTarget {
    Entity(Entity),
    Position(SpawnPosition),
}
impl SpawnTarget {
    pub fn from_map(&self, map: &Map) -> (Vec3, Vec3i) {
        match self {
            SpawnTarget::Entity(_) => unimplemented!(),
            SpawnTarget::Position(pos) => pos.from_map(&map),
        }
    }
}

#[derive(Debug, Clone, EnumDiscriminants)]
#[strum_discriminants(name(SpawnKind))]
pub enum SpawnArguments {
    Item {
        material: MaterialComponent,
    },
    Workshop {
        material: MaterialComponent,
    },
    Creature {
        name: Option<String>,
    },
    Stockpile,
    Foliage {
        dimensions: Option<DimensionsComponent>,
    },
    Pawn {
        arguments: SpawnPawnArguments,
    },
}

#[derive(Debug, Clone)]
pub struct SpawnPawnArguments {
    pub name: String,
    pub race: RaceDefinitionId,
}

#[derive(Debug, Clone)]
pub struct SpawnEvent<T = ()>
where
    T: Clone + std::fmt::Debug,
{
    pub target: SpawnTarget,
    pub kind: SpawnArguments,
    pub id: usize,
    pub arguments: T,
}

#[derive(
    shrinkwraprs::Shrinkwrap,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
pub struct DimensionsVec(#[serde(with = "Vec3uProxy")] Vec3u);
impl DimensionsVec {
    pub fn new(x: u32, y: u32, z: u32) -> Self {
        Self(Vec3u::new(x, y, z))
    }

    pub fn into_tiles(self) -> Vec3i {
        Vec3i::try_from(self.0 / 1000).unwrap()
    }

    pub fn from_tiles(tiles: Vec3i) -> Self {
        Self(Vec3u::try_from(tiles).unwrap() * 1000)
    }
}
impl From<Vec3u> for DimensionsVec {
    fn from(v: Vec3u) -> Self {
        Self(v)
    }
}
impl Default for DimensionsVec {
    fn default() -> Self {
        DimensionsVec::new(1000, 1000, 1000)
    }
}

impl_op!(+ |a: DimensionsVec, b: DimensionsVec| -> DimensionsVec { DimensionsVec::from(*a + *b) });
impl_op!(-|a: DimensionsVec, b: DimensionsVec| -> DimensionsVec { DimensionsVec::from(*a - *b) });
impl_op!(/ |a: DimensionsVec, b: DimensionsVec| -> DimensionsVec { DimensionsVec::from(*a / *b) });
impl_op!(*|a: DimensionsVec, b: DimensionsVec| -> DimensionsVec { DimensionsVec::from(*a * *b) });

impl_op!(+= |a: &mut DimensionsVec, b: DimensionsVec| { a.0 += *b });
impl_op!(-= |a: &mut DimensionsVec, b: DimensionsVec| { a.0 -= *b });
impl_op!(/= |a: &mut DimensionsVec, b: DimensionsVec| { a.0 /= *b });
impl_op!(*= |a: &mut DimensionsVec, b: DimensionsVec| { a.0 *= *b });
