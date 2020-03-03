use rl_core::defs::{
    reaction::{ReactionDefinition, ReactionDefinitionId},
    DefinitionStorage,
};
use rl_core::{
    bitflags::*,
    bitflags_serial,
    components::{Destroy, PositionComponent},
    fxhash::{FxHashMap, FxHashSet},
    legion::{borrow::AtomicRefCell, entity::Entity},
    map::Map,
    rstar, slotmap,
    smallvec::SmallVec,
    strum_macros::EnumDiscriminants,
    uuid::Uuid,
    GameStateRef,
};
use rl_reaction::{ReactionEntity, ReactionExecution};
use std::{convert::TryInto, hash::Hash, iter::FromIterator, sync::Arc};

pub const TASKKIND_COUNT: usize = 3;

bitflags_serial! {
    pub struct TaskKind: u32 {
        const MINING           =  0b1000_0000_0000_0000_0000_0000_0000_0000;
        const CLEANING         =  0b0100_0000_0000_0000_0000_0000_0000_0000;
        const WOODCUTTING      =  0b0010_0000_0000_0000_0000_0000_0000_0000;
    }
}

#[derive(Default, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TaskPrioritiesComponent {
    priorities: [u8; TASKKIND_COUNT],
}
impl TaskPrioritiesComponent {
    pub fn get(&self, kind: TaskKind) -> u8 {
        self.priorities
            .get(Self::get_bit_index(kind.bits()).unwrap())
            .copied()
            .unwrap()
    }

    pub fn get_mut(&mut self, kind: TaskKind) -> &mut u8 {
        self.priorities
            .get_mut(Self::get_bit_index(kind.bits()).unwrap())
            .unwrap()
    }

    pub fn set(&mut self, kind: TaskKind, priority: u8) {
        self.priorities[Self::get_bit_index(kind.bits()).unwrap()] = priority;
    }

    pub fn sorted(&self) -> SmallVec<[(TaskKind, u8); TASKKIND_COUNT]> {
        let mut sorted: SmallVec<[(TaskKind, u8); TASKKIND_COUNT]> = SmallVec::default();

        self.iter()
            .for_each(|(kind, priority)| sorted.push((kind, *priority)));

        sorted.sort_by(|left, right| left.1.cmp(&right.1));

        sorted
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn iter(&self) -> impl Iterator<Item = (TaskKind, &u8)> {
        self.priorities.iter().enumerate().map(|(n, priority)| {
            (
                TaskKind::from_bits(Self::set_bit(n as u32)).unwrap(),
                priority,
            )
        })
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (TaskKind, &mut u8)> {
        self.priorities.iter_mut().enumerate().map(|(n, priority)| {
            (
                TaskKind::from_bits(Self::set_bit(n as u32)).unwrap(),
                priority,
            )
        })
    }

    #[inline]
    fn set_bit(p: u32) -> u32 {
        1 << (31 - p)
    }

    #[inline]
    fn get_bit_index(mut n: u32) -> Option<usize> {
        if !(n > 0 && ((n & (n - 1)) == 0)) {
            return None;
        }

        let mut count: u32 = 0;

        while n != 0 {
            n >>= 1;
            count += 1;
        }

        Some(32 - count as usize)
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum TaskResult {
    Waiting,
    #[serde(with = "rl_core::saveload::entity")]
    Running(Entity),
    Complete,
    Failed,
    Cancelled,
}
impl Default for TaskResult {
    fn default() -> Self {
        Self::Waiting
    }
}

#[derive(Default, Clone)]
// TODO: serialize/deserialize
pub struct HasTasksComponent {
    pub storage: TaskQueuePtr,
}
impl HasTasksComponent {
    pub fn is_empty(&self) -> bool {
        self.storage.get().is_empty()
    }
    pub fn len(&self) -> usize {
        self.storage.get().len()
    }
}
impl FromIterator<Task> for HasTasksComponent {
    fn from_iter<I: IntoIterator<Item = Task>>(iter: I) -> Self {
        Self {
            storage: Arc::new(AtomicRefCell::new(TaskQueue::from_iter(iter))),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Task {
    pub id: u128,
    pub priority: u8,
    pub kind: TaskKind,
    pub reaction: ReactionDefinitionId,
}
impl PartialEq for Task {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.reaction.eq(&other.reaction)
    }
}
impl Task {
    pub fn new(priority: u8, kind: TaskKind, reaction: ReactionDefinitionId) -> Self {
        let id = Uuid::new_v4().as_u128();
        Self {
            id,
            priority,
            kind,
            reaction,
        }
    }
}

use rl_core::{
    legion::{borrow, prelude::*},
    math::Vec3i,
    Logging,
};

#[derive(Clone, Debug, PartialEq, EnumDiscriminants)]
#[strum_discriminants(name(FindBestTaskErrorKind))]
pub enum FindBestTaskError {
    MissingReagent(rl_core::defs::reaction::Reagent),
    NoPath(Vec3i),
    Empty,
    Other,
}

#[derive(Clone)]
pub struct TaskCacheEntry {
    pub entity: Entity,
    pub queue: TaskQueuePtr,
}

#[derive(Default)]
pub struct TaskCache {
    pub tree: rstar::RTree<[i32; 3]>,
    pub tasks: FxHashMap<Vec3i, TaskCacheEntry>,
}

impl TaskCache {
    fn update<'a>(
        &mut self,
        iter: impl Iterator<
            Item = (
                Entity,
                (borrow::Ref<'a, HasTasksComponent>, PositionComponent),
            ),
        >,
    ) {
        self.tasks.clear();

        self.tree = rstar::RTree::bulk_load(
            iter.filter_map(|(entity, (tasks, map_position))| {
                // Dont cache empty task storages
                if tasks.storage.get().is_empty() {
                    return None;
                }

                self.tasks.insert(
                    *map_position,
                    TaskCacheEntry {
                        entity,
                        queue: tasks.storage.clone(),
                    },
                );

                Some(
                    (*map_position)
                        .as_slice()
                        .try_into()
                        .expect("slice with incorrect length"),
                )
            })
            .collect::<Vec<_>>(),
        );
    }

    pub fn len(&self) -> usize {
        self.tasks
            .values()
            .fold(0, |acc, entry| acc + entry.queue.get().len())
    }

    pub fn is_empty(&self) -> bool {
        for entry in self.tasks.values() {
            if !entry.queue.get().is_empty() {
                return false;
            }
        }

        true
    }

    pub fn find_best(
        &self,
        state: GameStateRef,
        source_entity: Entity,
        source_location: Vec3i,
        source_priorities: &TaskPrioritiesComponent,
    ) -> Result<(Vec3i, TaskCacheEntry, TaskHandle), FindBestTaskError> {
        game_metrics::scope!("find_best_task");

        let priorities = source_priorities.sorted();

        let mut err = FindBestTaskError::Empty;

        let (map, reactions) =
            <(Read<Map>, Read<DefinitionStorage<ReactionDefinition>>)>::fetch(state.resources);

        for (task_kind, _) in &priorities {
            for task_location in self.tree.nearest_neighbor_iter(
                source_location
                    .as_slice()
                    .try_into()
                    .expect("slice with incorrect length"),
            ) {
                // Nearest neighbor in order
                if let Some(queue_ptr) = self.tasks.get(&task_location.into()) {
                    let queue = queue_ptr.queue.get();

                    let iter = queue.iter_available(*task_kind);

                    if let Some(iter) = iter {
                        let mut sorted = iter.collect::<Vec<_>>();
                        sorted.sort_by(|left, right| left.priority.cmp(&right.priority));

                        for entry in &sorted {
                            // Before expensive pathfinding, check the tasks reaction
                            let task = queue.get(entry.handle).unwrap();

                            match task
                                .reaction
                                .fetch(&reactions)
                                .can_initiate(state, ReactionEntity::Pawn(source_entity))
                            {
                                Ok(_) => {
                                    if let Some(dst) =
                                        crate::pathfinding::neighbors(&map, &task_location.into())
                                            .into_iter()
                                            .nth(0)
                                    {
                                        return Ok((dst.0, queue_ptr.clone(), entry.handle));
                                    } else if err == FindBestTaskError::Empty {
                                        err = FindBestTaskError::NoPath(task_location.into())
                                    }
                                }
                                Err(e) => {
                                    if FindBestTaskErrorKind::MissingReagent != err.clone().into() {
                                        err = FindBestTaskError::MissingReagent(e);
                                    }
                                }
                            };
                        }
                    }
                }
            }
        }
        Err(err)
    }
}

pub fn build_update_task_cache_system(
    _: &mut World,
    resources: &mut Resources,
) -> Box<dyn Schedulable> {
    resources.insert(TaskCache::default());

    SystemBuilder::<()>::new("update_task_cache_system")
        .read_resource::<Logging>()
        .read_resource::<Map>()
        .write_resource::<TaskCache>()
        .with_query(
            <(Read<HasTasksComponent>, Read<PositionComponent>)>::query()
                .filter(!component::<Destroy>()),
        )
        .with_query(
            <Write<HasTasksComponent>>::query()
                .filter(changed::<HasTasksComponent>() & !component::<Destroy>()),
        )
        .build(
            move |_command_buffer,
                  world,
                  (_log, _map, cache),
                  (task_query, _task_changed_query)| {
                game_metrics::scope!("update_task_cache_system");

                //let has_change = task_changed_query.iter_mut(world).nth(0).is_some();
                //if has_change {
                cache.update(
                    task_query
                        .iter_entities(world)
                        .map(|(e, (a, b))| (e, (a, PositionComponent::new(**b)))),
                );
                //}
            },
        )
}

pub fn build_cleanup_virtual_tasks(_: &mut World, _: &mut Resources) -> Box<dyn Schedulable> {
    use rl_core::components::VirtualTaskTag;
    SystemBuilder::<()>::new("cleanup_virtual_tasks")
        .write_resource::<TaskCache>()
        .with_query(<Read<HasTasksComponent>>::query().filter(tag::<VirtualTaskTag>()))
        .with_query(
            <Read<HasTasksComponent>>::query()
                .filter(tag::<VirtualTaskTag>() & component::<Destroy>()),
        )
        .with_query(
            <(Read<HasTasksComponent>, Read<PositionComponent>)>::query()
                .filter(!component::<Destroy>()),
        )
        .build(
            move |command_buffer, world, _cache, (virtual_task_query, _, _task_query)| {
                game_metrics::scope!("cleanup_virtual_tasks");

                for (entity, tasks) in virtual_task_query.iter_entities(world) {
                    if tasks.storage.get().len() == 0 {
                        command_buffer.add_component(entity, Destroy::default());
                    }
                }

                //if flag {
                //    cache.update(
                //        task_query
                //            .iter_entities(world)
                //            .map(|(e, (a, b))| (e, (a, PositionComponent::new(**b)))),
                //    );
                //}
            },
        )
}

use std::collections::BTreeSet;

slotmap::new_key_type! { pub struct TaskHandle; }

#[derive(
    Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct TaskQueueEntry {
    priority: u8,
    handle: TaskHandle,
}

pub type TaskQueuePtr = Arc<AtomicRefCell<TaskQueue>>;

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskQueue {
    storage: slotmap::SlotMap<TaskHandle, Task>,
    available: FxHashMap<TaskKind, BTreeSet<TaskQueueEntry>>,
    taken: FxHashSet<TaskQueueEntry>,
}
impl FromIterator<Task> for TaskQueue {
    fn from_iter<I: IntoIterator<Item = Task>>(iter: I) -> Self {
        let mut storage = slotmap::SlotMap::default();
        let mut available = FxHashMap::default();

        for task in iter {
            let priority = task.priority;

            let handle = storage.insert(task);

            available
                .entry(task.kind)
                .or_insert_with(BTreeSet::<TaskQueueEntry>::default)
                .insert(TaskQueueEntry { priority, handle });
        }

        Self {
            storage,
            available,
            ..Default::default()
        }
    }
}
impl TaskQueue {
    pub fn insert(&mut self, task: Task) -> TaskHandle {
        let handle = self.storage.insert(task);
        self.available
            .entry(task.kind)
            .or_insert_with(Default::default)
            .insert(TaskQueueEntry {
                priority: task.priority,
                handle,
            });

        handle
    }

    pub fn get(&self, handle: TaskHandle) -> Option<&Task> {
        self.storage.get(handle)
    }

    pub fn get_mut(&mut self, handle: TaskHandle) -> Option<&mut Task> {
        self.storage.get_mut(handle)
    }

    pub fn iter_available(&self, kind: TaskKind) -> Option<impl Iterator<Item = &TaskQueueEntry>> {
        Some(self.available.get(&kind)?.iter())
    }

    pub fn iter_all(&self) -> impl Iterator<Item = (TaskHandle, &Task)> {
        self.storage.iter()
    }

    pub fn is_available(&self, handle: TaskHandle) -> bool {
        if let Some(task) = self.storage.get(handle) {
            return !self.taken.contains(&TaskQueueEntry {
                priority: task.priority,
                handle,
            });
        }

        false
    }

    pub fn top_any(&self) -> Option<TaskHandle> {
        for set in self.available.values() {
            if let Some(entry) = set.iter().rev().nth(0) {
                return Some(entry.handle);
            }
        }
        None
    }

    pub fn top(&self, kind: TaskKind) -> Option<TaskHandle> {
        self.available
            .get(&kind)?
            .iter()
            .rev()
            .nth(0)
            .map(|e| e.handle)
    }

    pub fn take(&mut self, handle: TaskHandle) -> Option<Task> {
        let task = self.storage.get(handle)?;

        let entry = TaskQueueEntry {
            priority: task.priority,
            handle,
        };

        if !self.available.get_mut(&task.kind)?.remove(&entry) {
            return None;
        }

        self.taken.insert(entry);

        Some(*task)
    }

    pub fn cancel(&mut self, handle: TaskHandle) -> bool {
        if let Some(task) = self.storage.get(handle) {
            let entry = TaskQueueEntry {
                priority: task.priority,
                handle,
            };
            self.taken.remove(&entry);

            self.available
                .entry(task.kind)
                .or_insert_with(Default::default)
                .insert(entry);

            return true;
        }

        false
    }

    pub fn complete(&mut self, handle: TaskHandle) -> bool {
        let mut res = false;
        if let Some(task) = self.storage.get(handle) {
            res = self.taken.remove(&TaskQueueEntry {
                priority: task.priority,
                handle,
            });
        }
        self.storage.remove(handle);
        res
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_priorities_storage() {
        let mut p = TaskPrioritiesComponent::default();

        p.set(TaskKind::MINING, 55);
        p.set(TaskKind::CLEANING, 123);
        p.set(TaskKind::WOODCUTTING, 222);

        assert_eq!(55, p.get(TaskKind::MINING));
        assert_eq!(123, p.get(TaskKind::CLEANING));
        assert_eq!(222, p.get(TaskKind::WOODCUTTING));

        for task in p.iter() {
            println!("itered: {:?}", task);
        }
    }
}
