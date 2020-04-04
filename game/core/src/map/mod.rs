#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::wildcard_imports,
    clippy::missing_safety_doc,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::missing_safety_doc,
    clippy::use_self,
    clippy::cast_lossless,
    clippy::cast_sign_loss,
    clippy::unused_self
)]

use crate::{
    fxhash::FxHashSet,
    map::{
        encoders::FlatEncoder,
        tile::{TileFlag, TileKind},
    },
    math::{Vec2i, Vec3, Vec3Proxy, Vec3i, Vec3iProxy},
};
use crossbeam::queue::SegQueue;
use derivative::Derivative;
use parking_lot::{Mutex, MutexGuard, RwLock};
use rayon::prelude::*;
use smallvec::SmallVec;
use std::borrow::Borrow;

pub mod encoders;
pub mod generate_region;
pub mod spatial;
pub mod systems;
pub mod tile;

use tile::Tile;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Map {
    storage: Vec<Tile>,
    encoder: encoders::FlatEncoder,
    #[serde(with = "Vec3iProxy")]
    dimensions: Vec3i,
    #[serde(with = "Vec3iProxy")]
    pub sprite_dimensions: Vec3i,
    version: Mutex<MapVersion>,

    #[serde(with = "Vec3Proxy")]
    half_world_dimensions: Vec3,

    pub height_map: Vec<u8>,

    pub has_liquid: RwLock<FxHashSet<usize>>,
}
impl Map {
    #[allow(clippy::cast_precision_loss)]
    pub fn with_default<F>(dimensions: Vec3i, default: F) -> Result<Self, anyhow::Error>
    where
        F: Fn() -> Tile,
    {
        let encoder = FlatEncoder::from_dimensions(dimensions);
        let size = FlatEncoder::allocation_size(dimensions);

        let storage = vec![(default)(); size];

        let sprite_dimensions = Vec3i::new(16, 24, 1);

        let mut r = Self {
            storage,
            encoder,
            half_world_dimensions: Vec3::new(
                ((dimensions.x as f32) * sprite_dimensions.x as f32) / 2.0,
                ((dimensions.y as f32) * sprite_dimensions.y as f32) / 2.0,
                1.0,
            ),
            dimensions,
            version: Mutex::new(MapVersion::default()),
            sprite_dimensions,
            height_map: Vec::default(),
            has_liquid: RwLock::new(FxHashSet::default()),
        };
        r.recompute_height_map();

        Ok(r)
    }

    pub fn new(dimensions: Vec3i) -> Result<Self, anyhow::Error> {
        Self::with_default(dimensions, Tile::default)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    #[inline]
    #[allow(clippy::cast_precision_loss)]
    pub fn tile_to_world(&self, tile_coord: Vec3i) -> Vec3 {
        Vec3::new(
            (tile_coord.x * self.sprite_dimensions.x) as f32 - self.half_world_dimensions.x,
            (tile_coord.y * self.sprite_dimensions.y) as f32 - self.half_world_dimensions.y,
            tile_coord.z as f32,
        )
    }

    #[inline]
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    pub fn world_to_tile(&self, world_coord: Vec3) -> Vec3i {
        let mut coord = Vec3i::new(
            ((world_coord.x + self.half_world_dimensions.x) / self.sprite_dimensions.x as f32)
                .round() as i32,
            ((world_coord.y + self.half_world_dimensions.y) / self.sprite_dimensions.y as f32)
                .round() as i32,
            world_coord.z.floor() as i32,
        );
        coord.clamp(Vec3i::new(0, 0, 0), self.dimensions() - Vec3i::new(1, 1, 1));

        coord
    }

    #[inline]
    pub fn encoder(&self) -> &FlatEncoder {
        &self.encoder
    }

    #[inline]
    pub fn version(&self) -> MutexGuard<'_, MapVersion> {
        self.version.lock()
    }

    #[inline]
    pub fn clear_dirty(&self) {
        self.version.lock().dirty.clear();
    }

    #[inline]
    pub fn height_at(&self, coords: Vec2i) -> i32 {
        *self
            .height_map
            .get(self.encoder().encode(Vec3i::new(coords.x, coords.y, 0)))
            .unwrap() as i32
    }

    #[inline]
    pub fn get(&self, coords: Vec3i) -> &Tile {
        self.storage
            .get(self.encoder.encode(coords) as usize)
            .expect(&format!("Coordinate out of bounds: {:?}", coords))
    }

    #[inline]
    pub fn get_mut_untracked(&mut self, coords: Vec3i) -> &mut Tile {
        self.storage
            .get_mut(self.encoder.encode(coords) as usize)
            .expect(&format!("Coordinate out of bounds: {:?}", coords))
    }

    #[inline]
    pub fn get_mut(&mut self, coords: Vec3i) -> &mut Tile {
        let morton = self.encoder.encode(coords);

        /*slog::trace!(
            &self.log,
            "get_mut() - Marked {:?}, {:?} as dirty",
            morton,
            coord
        )*/
        let result = self
            .storage
            .get_mut(morton as usize)
            .expect(&format!("Coordinate out of bounds: {:?}", coords));
        self.version.get_mut().mark_dirty(morton, (*result).clone());

        result
    }

    #[inline]
    pub fn mark_dirty(&self, coords: Vec3i) {
        let morton = self.encoder.encode(coords);

        let result = self
            .storage
            .get(morton as usize)
            .expect(&format!("Coordinate out of bounds: {:?}", coords));
        self.version().mark_dirty(morton, (*result).clone());
    }

    #[inline]
    pub fn set(&mut self, coords: Vec3i, tile: Tile) {
        let morton = self.encoder.encode(coords);
        self.version.get_mut().mark_dirty(morton, tile.clone());
        /*slog::trace!(
            &self.log,
            "set() -  Marked {:?}, {:?} as dirty",
            morton,
            coord
        );*/
        *self
            .storage
            .get_mut(morton as usize)
            .expect(&format!("Coordinate out of bounds: {:?}", coords)) = tile;
    }

    #[inline]
    pub fn set_untracked(&mut self, coords: Vec3i, tile: Tile) {
        *self
            .storage
            .get_mut(self.encoder.encode(coords) as usize)
            .expect(&format!("Coordinate out of bounds: {:?}", coords)) = tile;
    }

    #[inline]
    pub fn size(&self) -> usize {
        self.storage.len()
    }

    #[inline]
    pub fn dimensions(&self) -> Vec3i {
        self.dimensions
    }

    #[inline]
    pub fn storage(&self) -> &Vec<Tile> {
        &self.storage
    }

    pub fn neighbors_3d(&self, coord: &Vec3i) -> SmallVec<[Vec3i; 24]> {
        use std::iter::FromIterator;

        SmallVec::from_iter(
            self.neighbors(coord)
                .into_iter()
                .chain(
                    self.neighbors(&Vec3i::new(coord.x, coord.y, coord.z - 1))
                        .into_iter(),
                )
                .chain(
                    self.neighbors(&Vec3i::new(coord.x, coord.y, coord.z + 1))
                        .into_iter(),
                ),
        )
    }

    pub fn neighbors(&self, coord: &Vec3i) -> SmallVec<[Vec3i; 8]> {
        let mut ret = SmallVec::new();

        let dimensions = self.dimensions();

        let x = coord.x - 1;
        if x >= 0 {
            ret.push(Vec3i::new(x, coord.y, coord.z));
            let y = coord.y + 1;
            if y < dimensions.y {
                ret.push(Vec3i::new(x, y, coord.z));
            }
        }
        let y = coord.y - 1;
        if y >= 0 {
            ret.push(Vec3i::new(coord.x, y, coord.z));
            let x = coord.x + 1;
            if x < dimensions.x {
                ret.push(Vec3i::new(x, y, coord.z));
            }
        }
        let x = coord.x - 1;
        let y = coord.y - 1;
        if x >= 0 && y >= 0 {
            ret.push(Vec3i::new(x, y, coord.z));
        }

        let x = coord.x + 1;
        let y = coord.y + 1;
        if x < dimensions.x {
            if y < dimensions.y {
                ret.push(Vec3i::new(x, y, coord.z));
            }
            ret.push(Vec3i::new(x, coord.y, coord.z));
        }
        if y < dimensions.y {
            ret.push(Vec3i::new(coord.x, y, coord.z));
        }
        ret
    }

    pub fn recompute_height_map_single(&mut self, coord: Vec3i) {
        // Was this mutation along the heightmap?
        let height_coord = self.encoder.encode(Vec3i::new(coord.x, coord.y, 0));

        if coord.z == *self.height_map.get(height_coord).unwrap() as i32 {
            let z = (0..self.dimensions.z - 1)
                .find(|z| !self.get(Vec3i::new(coord.x, coord.y, *z)).is_empty());
            *self.height_map.get_mut(height_coord).unwrap() = z.map(|z| z as u8).unwrap();
        }
    }

    pub fn recompute_height_map(&mut self) {
        let min = self.encoder.encode(Vec3i::new(0, 0, 0));
        let max = self
            .encoder
            .encode(Vec3i::new(self.dimensions.x - 1, self.dimensions.y - 1, 0));

        self.height_map = (min..max)
            .into_par_iter()
            .map(|index| {
                let xy = self.encoder.decode(index);

                if let Some(z) = (0..self.dimensions.z - 1)
                    .find(|z| !self.get(Vec3i::new(xy.x, xy.y, *z)).is_empty())
                {
                    return z as u8;
                }
                panic!(
                    "No height Z at coordinates!? {:?}",
                    self.encoder.decode(index)
                );
            })
            .collect::<Vec<_>>();
    }

    pub fn add_liquid(&self, coord: Vec3i) {
        self.has_liquid.write().insert(self.encoder().encode(coord));
    }

    pub fn update_liquid(&self, coord: Vec3i) {
        if self.get(coord).liquid.is_none() {
            self.has_liquid.write().remove(&self.encoder.encode(coord));
        }
    }

    pub fn writer(&mut self) -> MapWriter<'_> {
        MapWriter::new(self)
    }

    pub fn writer_with_capacity(&mut self, capacity: usize) -> MapWriter<'_> {
        MapWriter::with_capacity(self, capacity)
    }

    pub fn maintain(&mut self) {
        self.recompute_height_map();
    }

    /// # Safety
    /// Must garunteee that indices never conflict
    pub fn par_iter_indices_mut<T, F>(&self, indices: impl ParallelIterator<Item = T>, f: F)
    where
        T: Borrow<usize> + Send + Sync,
        F: Fn(Vec3i, &mut Tile) -> bool + Send + Sync, // return if dirty
    {
        let storage_ref = &self.storage;
        let encoder = self.encoder();
        indices.for_each(|index| {
            let index = *index.borrow();
            let ptr = storage_ref.as_ptr() as *mut Tile;
            assert!(index < storage_ref.len());

            let tile = unsafe { &mut *ptr.add(index) };
            if (f)(encoder.decode(index), tile) {
                self.mark_dirty(encoder.decode(index));
            }
        })
    }

    /// # Safety
    /// Must garunteee that indices never conflict
    pub fn par_iter_coords_mut<T, F>(&self, indices: impl ParallelIterator<Item = T>, f: F)
    where
        T: Borrow<Vec3i> + Send + Sync,
        F: Fn(Vec3i, &mut Tile) -> bool + Send + Sync, // return if dirty
    {
        let storage_ref = &self.storage;
        let encoder = self.encoder();
        indices.for_each(|coord| {
            let coord = *coord.borrow();
            let ptr = storage_ref.as_ptr() as *mut Tile;
            let index = encoder.encode(coord);
            assert!(index < storage_ref.len());

            let tile = unsafe { &mut *ptr.add(index) };
            if (f)(encoder.decode(index), tile) {
                self.mark_dirty(encoder.decode(index));
            }
        })
    }
}
pub struct MapWriter<'a> {
    map: &'a mut Map,
    wrote_coords: SegQueue<Vec3i>,
}
impl<'a> MapWriter<'a> {
    pub fn new(map: &'a mut Map) -> Self {
        Self {
            map,
            wrote_coords: SegQueue::default(),
        }
    }
    pub fn with_capacity(map: &'a mut Map, _: usize) -> Self {
        Self {
            map,
            wrote_coords: SegQueue::default(),
        }
    }
    pub fn finish(self) {}

    pub fn map(&self) -> &Map {
        self.map
    }

    pub fn encoder(&self) -> &FlatEncoder {
        self.map.encoder()
    }

    pub fn make_empty(self, coord: Vec3i) -> Self {
        let tile = self.map.get_mut(coord);
        tile.kind = TileKind::Empty;
        tile.material = 0;

        self.wrote_coords.push(coord);

        self
    }

    pub fn make_ramp(self, coord: Vec3i, kind: TileKind) -> Self {
        let tile = self.map.get_mut(coord);
        tile.kind = kind;
        tile.flags.insert(TileFlag::HAS_Z_TRANSITION);

        self.wrote_coords.push(coord);

        self
    }

    pub fn make_floor(self, coord: Vec3i) -> Self {
        let tile = self.map.get_mut(coord);
        tile.kind = TileKind::Floor;
        tile.flags.remove(TileFlag::CLEAR_FLOOR);

        self.wrote_coords.push(coord);

        self
    }
}
impl<'a> Drop for MapWriter<'a> {
    fn drop(&mut self) {
        let wrote_coords = &mut self.wrote_coords;
        let map = &mut *self.map;

        while let Ok(coord) = wrote_coords.pop() {
            map.neighbors_3d(&coord)
                .iter()
                .for_each(|inner| map.mark_dirty(*inner));

            map.recompute_height_map_single(coord)
        }
    }
}

#[derive(Default, Clone, Derivative, serde::Serialize, serde::Deserialize)]
#[derivative(Debug)]
pub struct MapVersion {
    pub version: u64,
    #[derivative(Debug = "ignore")]
    pub dirty: SmallVec<[(usize, Tile); 128]>,
}
impl MapVersion {
    pub fn mark_dirty(&mut self, morton: usize, tile: Tile) {
        self.version += 1;
        self.dirty.push((morton, tile));
    }
}
impl PartialEq for MapVersion {
    fn eq(&self, rhv: &Self) -> bool {
        self.version == rhv.version
    }
}
impl Eq for MapVersion {}
impl PartialOrd for MapVersion {
    fn partial_cmp(&self, rhv: &Self) -> Option<std::cmp::Ordering> {
        self.version.partial_cmp(&rhv.version)
    }
}
impl Ord for MapVersion {
    fn cmp(&self, rhv: &Self) -> std::cmp::Ordering {
        self.version.cmp(&rhv.version)
    }
}
