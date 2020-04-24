pub use crate::blackboard::BlackboardComponent;
use crate::{
    data::{CollisionKind, DimensionsVec, PartGraphId},
    defs::{
        foliage::FoliageKind,
        needs::{NeedKind, NEEDKIND_COUNT},
    },
    math::{Aabbi, Vec3i, Vec3iProxy, Vec3u},
    TypeUuid,
};
use legion::entity::Entity;
use shrinkwraprs::Shrinkwrap;
use smallvec::SmallVec;
use std::convert::{TryFrom, TryInto};

#[derive(TypeUuid, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[uuid = "fe5d2950-e7ee-4c71-9869-2d782604329d"]
pub struct StaticTag;

#[derive(TypeUuid, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[uuid = "0861420d-4c3f-45d4-b8d6-5df668c86de0"]
pub struct BuildingTag;

#[derive(TypeUuid, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[uuid = "5fdc02df-e199-46f0-b80d-754fd97bdf50"]
pub struct WorkshopTag;

#[derive(TypeUuid, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[uuid = "4130f721-b467-4f0d-a9de-5cc03fba4b6c"]
pub struct ItemTag;

#[derive(
    TypeUuid,
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
#[uuid = "22c8641a-d14b-4119-b156-bd4cbf3cd597"]
pub struct FoliageTag(pub FoliageKind);

#[derive(
    TypeUuid, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[uuid = "cfc3e23b-c58e-4b1d-aa8c-7fa5da894c64"]
pub struct PawnTag;

#[derive(
    TypeUuid, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct CreatureTag;

#[derive(
    TypeUuid,
    Debug,
    Clone,
    Copy,
    Eq,
    PartialEq,
    Hash,
    Ord,
    PartialOrd,
    serde::Serialize,
    serde::Deserialize,
)]
#[uuid = "de6d700d-0c9e-4708-b204-f2194ac3a902"]
pub struct Destroy {
    pub delay: u64,
}
impl Default for Destroy {
    fn default() -> Self {
        Self { delay: 1 }
    }
}

#[derive(TypeUuid, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[uuid = "46df1ba0-e00f-4a6a-b7e6-3e82ef113759"]
pub struct EntityMeta {
    created: crate::time::TimeStamp,
}
impl EntityMeta {
    pub fn created(&self) -> &crate::time::TimeStamp {
        &self.created
    }

    pub fn new(created: crate::time::TimeStamp) -> Self {
        Self { created }
    }
}

#[derive(TypeUuid, Default, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[uuid = "a425616f-bc28-4610-b655-4232ed107951"]
pub struct SelectedComponent;

#[derive(
    TypeUuid, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[uuid = "e7f926fd-99fb-4416-bdfb-fdf0c2b89368"]
pub struct DeadTag;

#[derive(
    TypeUuid, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[uuid = "6ae294c4-2ab3-47b6-ae5e-47c1adf2bf81"]
pub struct VirtualTaskTag;

#[derive(
    Clone, Copy, PartialEq, Eq, Hash, Debug, thiserror::Error, serde::Serialize, serde::Deserialize,
)]
pub enum MovementError {
    #[error("No path to target")]
    NoPath,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct MovementRequest {
    #[serde(with = "Vec3iProxy")]
    pub destination: Vec3i,
    #[serde(with = "Vec3iProxy")]
    pub started_at: Vec3i,
    pub distance: Option<u32>,
}
impl PartialEq for MovementRequest {
    fn eq(&self, rhv: &Self) -> bool {
        self.destination.eq(&rhv.destination)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MovementResult {
    pub request: MovementRequest,
    pub result: Result<(), MovementError>,
}
impl MovementRequest {
    pub fn new(started_at: Vec3i, destination: Vec3i) -> Self {
        Self {
            destination,
            started_at,
            distance: None,
        }
    }

    pub fn with_distance(started_at: Vec3i, destination: Vec3i, distance: u32) -> Self {
        Self {
            destination,
            started_at,
            distance: Some(distance),
        }
    }
}

#[derive(TypeUuid, Default, Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[uuid = "19d9285c-cfc6-4acd-8f82-83128baf075c"]
pub struct MovementComponent {
    pub current: Option<MovementRequest>,
    pub acc: f64,
}
impl PartialEq for MovementComponent {
    fn eq(&self, rhv: &Self) -> bool {
        self.current.eq(&rhv.current)
    }
}
impl PartialEq<MovementRequest> for MovementComponent {
    fn eq(&self, rhv: &MovementRequest) -> bool {
        if let Some(current) = self.current {
            current.eq(&rhv)
        } else {
            false
        }
    }
}

#[derive(TypeUuid, Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[uuid = "ae5f89ce-6718-454b-b11c-fb011a3fa4a4"]
pub struct NameComponent {
    pub name: String,
}
impl NameComponent {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[derive(TypeUuid, Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[uuid = "6280e474-aaf8-42b3-af83-c2e7b19fe86d"]
pub struct ItemContainerComponent {
    pub capacity: DimensionsVec,
    pub consumed: DimensionsVec,
    #[serde(with = "crate::saveload::entity::list")]
    pub inside: SmallVec<[Entity; 32]>,
    #[serde(with = "crate::saveload::entity::list")]
    pub queued_inside: SmallVec<[Entity; 3]>,
}
impl Default for ItemContainerComponent {
    fn default() -> Self {
        Self {
            capacity: DimensionsVec::new(u32::max_value(), u32::max_value(), u32::max_value()),
            consumed: DimensionsVec::new(0, 0, 0),
            inside: SmallVec::default(),
            queued_inside: SmallVec::default(),
        }
    }
}
impl ItemContainerComponent {
    pub fn push(&mut self, entity: Entity) {
        self.queued_inside.push(entity);
    }
    pub fn remove(&mut self, entity: Entity) -> bool {
        let mut removed = false;
        self.inside.retain(|e| {
            if *e == entity {
                removed = true;
                false
            } else {
                true
            }
        });

        removed
    }
}

#[derive(
    TypeUuid, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[uuid = "d62f5780-161c-4923-8397-43eb00d9245b"]
pub struct ItemContainerChildComponent {
    #[serde(with = "crate::saveload::entity")]
    pub parent: Entity,
}

#[derive(TypeUuid, Default, Clone)]
#[uuid = "a45d13d0-424b-4c6e-a1a6-14e979672724"]
pub struct CarryComponent {
    pub limbs: SmallVec<[(PartGraphId, Option<Entity>); 2]>,
}
impl CarryComponent {
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.limbs.iter().filter_map(|(_, item)| *item)
    }

    pub fn remove(&mut self, item: Entity) -> bool {
        for limb in &mut self.limbs {
            if let Some(limb_item) = limb.1 {
                if item == limb_item {
                    limb.1 = None;
                    return true;
                }
            }
        }
        false
    }
}

#[derive(TypeUuid, Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[uuid = "19670343-bbc6-4ace-8340-a7d126fb9d69"]
pub struct ActivePickupComponent {
    #[serde(with = "crate::saveload::entity")]
    pub initiator: Entity,
    pub started_at: f64,
}
impl ActivePickupComponent {
    pub fn new(initiator: Entity, started_at: f64) -> Self {
        Self {
            initiator,
            started_at,
        }
    }
}

#[derive(
    TypeUuid, Default, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[uuid = "b36dcee3-905e-4c72-80b3-612b74c1e7f6"]
pub struct DimensionsComponent {
    pub min: DimensionsVec,
    pub norm: DimensionsVec,
    pub max: DimensionsVec,
    pub current: DimensionsVec,
    pub collision: CollisionKind,
}
impl DimensionsComponent {
    pub fn with_tiles(dimensions: Vec3i) -> Self {
        let dimensions = DimensionsVec::from(Vec3u::try_from(dimensions).unwrap() * 1000);
        Self {
            min: dimensions,
            norm: dimensions,
            max: dimensions,
            current: dimensions,
            ..Default::default()
        }
    }

    pub fn with_collision(dimensions: DimensionsVec, collision: CollisionKind) -> Self {
        Self {
            min: dimensions,
            norm: dimensions,
            max: dimensions,
            current: dimensions,
            collision,
        }
    }

    pub fn new(dimensions: DimensionsVec) -> Self {
        Self {
            min: dimensions,
            norm: dimensions,
            max: dimensions,
            current: dimensions,
            ..Default::default()
        }
    }

    pub fn with_minmax(min: DimensionsVec, norm: DimensionsVec, max: DimensionsVec) -> Self {
        Self {
            min,
            norm,
            max,
            current: norm,
            ..Default::default()
        }
    }

    pub fn collision(&self) -> CollisionKind {
        self.collision
    }

    pub fn as_tiles(&self) -> Vec3i {
        self.current.into_tiles()
    }

    #[inline]
    pub fn aabb(&self) -> Aabbi {
        Aabbi::new(Vec3i::new(0, 0, 0), self.as_tiles())
    }

    #[inline]
    pub fn occupies_limit_z(&self, coord: Vec3i) -> Aabbi {
        let current = self.as_tiles();
        Aabbi::new(
            coord,
            coord
                + Vec3i::new(
                    current.x.try_into().unwrap(),
                    current.y.try_into().unwrap(),
                    1,
                ),
        )
    }

    #[inline]
    pub fn occupies(&self, coord: Vec3i) -> Aabbi {
        // Flip the z-axis

        Aabbi::new(coord + (self.as_tiles() * Vec3i::new(1, 1, -1)), coord)
    }
}

#[derive(
    TypeUuid, Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
#[uuid = "92383511-ae73-456b-a4d8-8350fca5bc07"]
pub struct PositionComponent {
    #[serde(with = "crate::math::Vec3iProxy")]
    pub coord: Vec3i,
}
impl PositionComponent {
    pub fn new(coord: Vec3i) -> Self {
        Self { coord }
    }
}
impl From<Vec3i> for PositionComponent {
    fn from(src: Vec3i) -> Self {
        Self { coord: src }
    }
}
impl AsRef<Vec3i> for PositionComponent {
    fn as_ref(&self) -> &Vec3i {
        &self.coord
    }
}
impl std::ops::Deref for PositionComponent {
    type Target = Vec3i;

    fn deref(&self) -> &Self::Target {
        &self.coord
    }
}
impl std::ops::DerefMut for PositionComponent {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.coord
    }
}

////////
use std::time::Duration;

#[derive(Debug, Copy, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct NeedDecay {
    pub value: i32,
    #[serde(default = "NeedDecay::default_minmax")]
    pub minmax: (i32, i32),
    pub frequency: Duration,
    pub lifetime: Duration,
    // State tracking
    pub start: f64,
    pub acc: f64,
}
impl NeedDecay {
    fn default_minmax() -> (i32, i32) {
        (i32::from(i16::min_value()), i32::from(i16::max_value()))
    }
}
impl NeedDecay {
    fn new(value: i32, frequency: Duration) -> Self {
        Self {
            value,
            frequency,
            lifetime: Duration::new(u64::max_value(), 0),
            minmax: Self::default_minmax(),
            start: 0.0,
            acc: 0.0,
        }
    }
    fn with_lifetime(value: i32, frequency: Duration, lifetime: Duration) -> Self {
        Self {
            value,
            frequency,
            lifetime,
            minmax: Self::default_minmax(),
            start: 0.0,
            acc: 0.0,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct NeedState {
    pub value: i32,
    pub decays: SmallVec<[NeedDecay; 6]>,
}
impl Default for NeedState {
    fn default() -> Self {
        Self {
            value: 0,
            decays: SmallVec::default(),
        }
    }
}

#[derive(TypeUuid, Shrinkwrap, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[shrinkwrap(mutable)]
#[uuid = "79cc5a17-e08c-41bf-bfec-911e6ecc0c3e"]
pub struct NeedsComponent(pub [NeedState; NEEDKIND_COUNT]);
impl Default for NeedsComponent {
    fn default() -> Self {
        let mut this = Self(init_array!(NEEDKIND_COUNT, NeedState::default()));

        // TODO: read from actual race definition
        // just add bases for now
        this.get_mut(NeedKind::Calories)
            .decays
            .push(NeedDecay::new(-1, Duration::new(1, 0)));

        this.get_mut(NeedKind::Hydration)
            .decays
            .push(NeedDecay::new(-1, Duration::new(1, 0)));

        this.get_mut(NeedKind::Sleep)
            .decays
            .push(NeedDecay::new(-1, Duration::new(1, 0)));

        this
    }
}
impl NeedsComponent {
    pub fn add(&mut self, kind: NeedKind, value: i32) {
        let state = self.0.get_mut(kind.as_usize()).unwrap();
        state.value = state.value.checked_add(value).unwrap_or(state.value);
    }
    pub fn sub(&mut self, kind: NeedKind, value: i32) {
        let state = self.0.get_mut(kind.as_usize()).unwrap();
        state.value = state.value.checked_sub(value).unwrap_or(state.value);
    }

    pub fn get(&self, kind: NeedKind) -> &NeedState {
        self.0.get(kind.as_usize()).unwrap()
    }
    pub fn get_mut(&mut self, kind: NeedKind) -> &mut NeedState {
        self.0.get_mut(kind.as_usize()).unwrap()
    }
}
