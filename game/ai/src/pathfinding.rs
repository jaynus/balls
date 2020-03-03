#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use rl_core::{
    components::PositionComponent,
    fxhash::{FxBuildHasher, FxHashMap},
    map::{spatial::SpatialMapEntry, tile::TileFlag, Map},
    math::Vec3i,
    smallvec::SmallVec,
    Distance,
};

use std::{cmp::Ordering, collections::BinaryHeap};

pub trait SpatialMapSet {
    fn collides(&self, point: &Vec3i) -> bool;
}

impl<'a> SpatialMapSet for [&'a rl_core::rstar::RTree<SpatialMapEntry>; 2] {
    fn collides(&self, point: &Vec3i) -> bool {
        for map in self.iter() {
            if map
                .locate_all_at_point(&PositionComponent::from(*point))
                .find(|entry| !entry.collision.is_walkable())
                .is_some()
            {
                return true;
            }
        }

        false
    }
}

#[rl_core::metrics::instrument]
pub fn astar_simple<S>(src: Vec3i, dst: Vec3i, map: &Map, spatial_set: &S) -> Option<Vec<Vec3i>>
where
    S: SpatialMapSet,
{
    // Make sure the destination is even walkable
    if !map.get(dst).is_walkable() || spatial_set.collides(&dst) {
        return None;
    }

    let path = a_star_search(
        AStar::encode(map, src),
        AStar::encode(map, dst),
        map,
        spatial_set,
    );

    if path.success {
        Some(
            path.steps
                .into_iter()
                .skip(1)
                .map(|step| AStar::decode(map, step))
                .collect(),
        )
    } else {
        None
    }
}

#[inline]
pub fn neighbors(map: &Map, coord: &Vec3i) -> SmallVec<[(Vec3i, f32); 16]> {
    let mut res = SmallVec::default();

    let mut insert = |coord| {
        if let Some(cost) = map.get(coord).movement_cost() {
            res.push((coord, cost));
        }
    };

    map.neighbors(coord).into_iter().for_each(|coord| {
        let tile = map.get(coord);
        if tile.is_empty() {
            let below = coord + Vec3i::new(0, 0, 1);
            let below_tile = map.get(below);
            // If its empty, do we have a Z-transition?
            if below_tile.flags.contains(TileFlag::HAS_Z_TRANSITION) {
                insert(below);
            }
        } else {
            // Is this a tile a ramp? If so, do we have an adjascent tile above?
            if coord.z > 0
                && tile.flags.contains(TileFlag::HAS_Z_TRANSITION)
                && tile.flags.contains(TileFlag::HAS_Z_TRANSITION)
            {
                let above = coord - Vec3i::new(0, 0, 1);
                map.neighbors(&above).into_iter().for_each(|above_n| {
                    if map.get(above_n).is_walkable() {
                        insert(above_n);
                    }
                });
            }
            insert(coord);
        }
    });

    /*
    let mut log_entry = format!("Neighbors for ({}, {}, {})\n", coord.x, coord.y, coord.z);
    res.iter().for_each(|(neighbor, cost): &(Vec3i, u32)| {
        log_entry.push_str(&format!(
            "\t{},{},{} - {}\n",
            neighbor.x, neighbor.y, neighbor.z, cost
        ))
    });
    slog::trace!(log, "{}", log_entry);
    */
    res
}

const MAX_ASTAR_STEPS: u32 = 65536;

pub fn a_star_search<S>(start: u32, end: u32, map: &Map, spatial_set: &S) -> NavigationPath
where
    S: SpatialMapSet,
{
    AStar::new(start, end).search(map, spatial_set)
}

/// Holds the result of an A-Star navigation query.
/// `destination` is the index of the target tile.
/// `success` is true if it reached the target, false otherwise.
/// `steps` is a vector of each step towards the target, *including* the starting position.
#[derive(Clone, Default)]
pub struct NavigationPath {
    pub destination: u32,
    pub success: bool,
    pub steps: Vec<u32>,
}

#[allow(dead_code)]
#[derive(Copy, Clone)]
/// Node is an internal step inside the A-Star path (not exposed/public). Idx is the current cell,
/// f is the total cost, g the neighbor cost, and h the heuristic cost.
struct Node {
    idx: u32,
    f: f32,
    g: f32,
    h: f32,
}

impl PartialEq for Node {
    fn eq(&self, other: &Self) -> bool {
        self.f == other.f
    }
}

impl Eq for Node {}

impl Ord for Node {
    fn cmp(&self, b: &Self) -> Ordering {
        b.f.partial_cmp(&self.f).unwrap()
    }
}

impl PartialOrd for Node {
    fn partial_cmp(&self, b: &Self) -> Option<Ordering> {
        b.f.partial_cmp(&self.f)
    }
}

impl NavigationPath {
    /// Makes a new (empty) `NavigationPath`
    pub fn new() -> NavigationPath {
        NavigationPath {
            destination: 0,
            success: false,
            steps: Vec::with_capacity(1024),
        }
    }
}

/// Private structure for calculating an A-Star navigation path.
struct AStar {
    start: u32,
    end: u32,
    open_list: BinaryHeap<Node>,
    closed_list: FxHashMap<u32, f32>,
    parents: FxHashMap<u32, u32>,
    step_counter: u32,
}

impl AStar {
    /// Creates a new path, with specified starting and ending indices.
    fn new(start: u32, end: u32) -> AStar {
        let mut open_list: BinaryHeap<Node> = BinaryHeap::with_capacity(1000);
        open_list.push(Node {
            idx: start,
            f: 0.0,
            g: 0.0,
            h: 0.0,
        });

        AStar {
            start,
            end,
            open_list,
            parents: FxHashMap::with_capacity_and_hasher(1000, FxBuildHasher::default()),
            closed_list: FxHashMap::with_capacity_and_hasher(1000, FxBuildHasher::default()),
            step_counter: 0,
        }
    }

    #[inline]
    fn decode(map: &Map, idx: u32) -> Vec3i {
        map.encoder().decode(idx as usize)
    }

    fn encode(map: &Map, idx: Vec3i) -> u32 {
        map.encoder().encode(idx) as u32
    }

    #[allow(clippy::cast_possible_truncation)]
    fn distance_to_end(&self, idx: u32, map: &Map) -> f32 {
        Self::decode(map, idx).distance(&Self::decode(map, self.end))
    }

    /// Adds a successor; if we're at the end, marks success.
    fn add_successor(&mut self, q: Node, idx: u32, cost: f32, map: &Map) -> bool {
        // Did we reach our goal?
        if idx == self.end {
            self.parents.insert(idx, q.idx);
            true
        } else {
            let distance = self.distance_to_end(idx, map);
            let s = Node {
                idx,
                f: distance + cost,
                g: cost,
                h: distance,
            };

            // If a node with the same position as successor is in the open list with a lower f, skip add
            let mut should_add = true;
            for e in &self.open_list {
                if e.f < s.f && e.idx == idx {
                    should_add = false;
                }
            }

            // If a node with the same position as successor is in the closed list, with a lower f, skip add
            if should_add && self.closed_list.contains_key(&idx) && self.closed_list[&idx] < s.f {
                should_add = false;
            }

            if should_add {
                self.open_list.push(s);
                self.parents.insert(idx, q.idx);
            }

            false
        }
    }

    /// Helper function to unwrap a path once we've found the end-point.
    fn found_it(&self) -> NavigationPath {
        let mut result = NavigationPath::new();
        result.success = true;
        result.destination = self.end;

        result.steps.push(self.end);
        let mut current = self.end;
        while current != self.start {
            let parent = self.parents[&current];
            result.steps.insert(0, parent);
            current = parent;
        }

        result
    }

    /// Performs an A-Star search
    fn search<S>(&mut self, map: &Map, spatial_set: &S) -> NavigationPath
    where
        S: SpatialMapSet,
    {
        let result = NavigationPath::new();
        while !self.open_list.is_empty() && self.step_counter < MAX_ASTAR_STEPS {
            self.step_counter += 1;

            // Pop Q off of the list
            let q = self.open_list.pop().unwrap();

            // Generate successors
            let successors = neighbors(map, &Self::decode(map, q.idx));

            for s in successors.iter().filter_map(|(coord, cost)| {
                if spatial_set.collides(coord) {
                    return None;
                }
                Some((Self::encode(map, *coord), *cost))
            }) {
                if self.add_successor(q, s.0, s.1 + q.f, map) {
                    let success = self.found_it();
                    return success;
                }
            }

            if self.closed_list.contains_key(&q.idx) {
                self.closed_list.remove(&q.idx);
            }
            self.closed_list.insert(q.idx, q.f);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_cost() {
        let src = Vec3::new(20.0, 20.0, 1.0);
        let dst = Vec3::new(20.0, 10.0, 0.0);

        let cost = (src - dst).mag() as u32;
        println!("cost = {}", cost);
    }
}
