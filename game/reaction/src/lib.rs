#![deny(clippy::pedantic, clippy::all)]
#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::wildcard_imports,
    clippy::missing_safety_doc,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    dead_code,
    clippy::use_self,
    clippy::default_trait_access,
    clippy::module_name_repetitions,
    clippy::match_single_binding,
    non_camel_case_types
)]

use num_derive::{FromPrimitive, ToPrimitive};
use rl_core::{
    blackboard::Blackboard,
    components::{Destroy, FoliageTag, PositionComponent},
    data::{SpawnArguments, SpawnEvent, Target, TargetPosition},
    defs::{
        foliage::FoliageKind,
        item::ItemComponent,
        material::{MaterialComponent, MaterialState},
        reaction::{ProductKind, ReactionDefinition, ReactionDefinitionId, Reagent},
        DefinitionStorage,
    },
    derivative::Derivative,
    dispatcher::{DispatcherBuilder, Stage},
    event::Channel,
    fnv_hash,
    fxhash::{FxBuildHasher, FxHashMap},
    legion::prelude::*,
    map::{spatial::SpatialMap, Map},
    math::Vec3i,
    smallvec::SmallVec,
    systems::progress_bar::ProgressBar,
    time::Time,
    uuid::Uuid,
    GameStateRef, GlobalCommandBuffer,
};
use std::{collections::HashMap, sync::Arc, time::Duration};

pub mod map_transformations;
pub mod needs;

pub fn effect_registration() -> Vec<ReactionEffectRegistration> {
    vec![
        ReactionEffectRegistration::new("ConsumeEdibleEffect", needs::ConsumeEdibleEffect::new),
        ReactionEffectRegistration::new(
            "TileChannelEffect",
            map_transformations::TileChannelEffect::new,
        ),
        ReactionEffectRegistration::new("TileDigEffect", map_transformations::TileDigEffect::new),
        ReactionEffectRegistration::new("ProduceItemEffect", ProduceItemEffect::new),
        ReactionEffectRegistration::new("TreeChopEffect", TreeChopEffect::new),
    ]
}

pub trait ReactionEffect: Send + Sync {
    fn name() -> &'static str
    where
        Self: Sized;

    fn new(
        _: GameStateRef,
        _: &ReactionDefinition,
        _: &ActiveReactionComponent,
        _: &BeginReactionEvent,
        _: &FxHashMap<Reagent, Target>,
    ) -> Box<dyn ReactionEffect>
    where
        Self: 'static + Sized + Default,
    {
        Box::new(Self::default())
    }

    fn tick(
        &mut self,
        state: GameStateRef,
        reaction: &ReactionDefinition,
        component: &ActiveReactionComponent,
        event: &BeginReactionEvent,
        entities: &FxHashMap<Reagent, Target>,
    ) -> ReactionResult;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReactionEntity {
    Pawn(Entity),
    Workshop(Entity),
    Any(Entity),
    Task(Entity),
}
impl ReactionEntity {
    pub fn entity(&self) -> Entity {
        match *self {
            Self::Pawn(e) | Self::Workshop(e) | Self::Any(e) | Self::Task(e) => e,
        }
    }
}

#[derive(FromPrimitive, ToPrimitive, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ReactionResult {
    Created,
    Running,
    Success,
    Failure,
}
impl ReactionResult {
    pub fn is_complete(self) -> bool {
        self == Self::Success || self == Self::Failure
    }
}
impl Default for ReactionResult {
    fn default() -> Self {
        Self::Created
    }
}

#[derive(Derivative, Clone)]
#[derivative(Debug)]
pub struct BeginReactionEvent {
    pub id: u128,
    pub reaction: ReactionDefinitionId,
    pub initiator: Option<ReactionEntity>,
    pub target: ReactionEntity,

    #[derivative(Debug = "ignore")]
    pub callback: Option<Arc<dyn Fn(ReactionResult) + Send + Sync>>,
}

impl BeginReactionEvent {
    pub fn new(
        reaction: ReactionDefinitionId,
        initiator: Option<ReactionEntity>,
        target: ReactionEntity,
        callback: Option<Arc<dyn Fn(ReactionResult) + Send + Sync>>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().as_u128(),
            reaction,
            initiator,
            target,
            callback,
        }
    }
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ActiveReactionComponent {
    reaction: ReactionDefinitionId,
    started: f64,

    duration: Duration,
    progress: Duration,

    cancellable: bool,
    event: BeginReactionEvent,

    #[derivative(Debug = "ignore")]
    blackboard: Blackboard,

    // Active effect state
    #[derivative(Debug = "ignore")]
    active_effect: Option<Box<dyn ReactionEffect>>,
}

pub fn bundle(
    _: &mut World,
    resources: &mut Resources,
    builder: &mut DispatcherBuilder,
) -> Result<(), anyhow::Error> {
    resources.insert(Channel::<BeginReactionEvent>::default());

    builder.add_thread_local_fn(Stage::Logic, build_execute_reaction_system);

    Ok(())
}

pub fn build_execute_reaction_system(
    _: &mut World,
    resources: &mut Resources,
) -> Box<dyn FnMut(&mut World, &mut Resources)> {
    let listener_id = resources
        .get_mut::<Channel<BeginReactionEvent>>()
        .unwrap()
        .bind_listener(64);

    let mut remove_components = SmallVec::<[Entity; 16]>::default();
    let active_query = <(Write<ActiveReactionComponent>, Write<Option<ProgressBar>>)>::query();

    // Generate the effects table and store it
    resources.insert(create_effect_table());

    Box::new(move |world: &mut World, resources: &mut Resources| {
        let (time, reaction_defs, channel) = <(
            Read<Time>,
            Read<DefinitionStorage<ReactionDefinition>>,
            Read<Channel<BeginReactionEvent>>,
        )>::fetch(&resources);

        // Tick first
        for (entity, (mut active_reaction, mut progress_bar)) in
            unsafe { active_query.iter_entities_unchecked(world) }
        {
            active_reaction.progress += time.world_delta;
            progress_bar.as_mut().unwrap().progress =
                active_reaction.progress.as_secs_f64() / active_reaction.duration.as_secs_f64();

            if active_reaction.progress >= active_reaction.duration {
                *progress_bar = None;

                let def = reaction_defs.get(active_reaction.reaction).unwrap();

                let mut res = ReactionResult::Failure;
                if let Ok(entities) = def.check(
                    GameStateRef { world, resources },
                    Target::Entity(active_reaction.event.initiator.unwrap().entity()),
                    Target::Entity(active_reaction.event.target.entity()),
                ) {
                    if let Some(_effect) = active_reaction.active_effect.as_mut() {
                        // TODO: just tick the current effect
                        // TODO: we need to change this for series of effects....
                        panic!("Unsupported");
                    } else {
                        res = def.produce_products(
                            GameStateRef { world, resources },
                            &active_reaction,
                            &entities,
                        );
                        if res == ReactionResult::Success {
                            res = def.consume_reagents(
                                GameStateRef { world, resources },
                                &active_reaction,
                                &active_reaction.event,
                                &entities,
                            );
                            if res == ReactionResult::Success {
                                res = def.apply(
                                    GameStateRef { world, resources },
                                    &active_reaction,
                                    &active_reaction.event,
                                );
                            }
                        }
                    }
                };

                if res.is_complete() {
                    if let Some(callback) = &active_reaction.event.callback {
                        (callback)(res);
                    }

                    remove_components.push(entity);
                }
            }
        }

        // Clear removals
        for entity in remove_components.drain(..) {
            world
                .remove_component::<ActiveReactionComponent>(entity)
                .unwrap();
        }

        // Add new ones
        while let Some(event) = channel.read(listener_id) {
            // Trigger the reaction as complete for now
            let def = reaction_defs.get(event.reaction).unwrap();

            let target = event.target.entity();

            world
                .add_component(
                    target,
                    ActiveReactionComponent {
                        reaction: event.reaction,
                        started: time.world_time,
                        duration: Duration::from_secs_f64(def.duration),
                        progress: Duration::from_secs_f64(0.0),
                        cancellable: false,
                        event,
                        blackboard: Blackboard::default(),
                        active_effect: None,
                    },
                )
                .unwrap();
            world
                .add_component(target, Some(ProgressBar::default()))
                .unwrap();
        }
    })
}

pub trait ReactionExecution {
    fn check(
        &self,
        state: GameStateRef,
        source: Target,
        target: Target,
    ) -> Result<FxHashMap<Reagent, Target>, Reagent>;

    fn check_designate(
        &self,
        state: GameStateRef,
        source: Target,
        target: Target,
    ) -> Result<FxHashMap<Reagent, Target>, Reagent>;

    fn apply(
        &self,
        state: GameStateRef,
        component: &ActiveReactionComponent,
        event: &BeginReactionEvent,
    ) -> ReactionResult;

    fn produce_products(
        &self,
        state: GameStateRef,
        component: &ActiveReactionComponent,
        reagent_entities: &FxHashMap<Reagent, Target>,
    ) -> ReactionResult;

    fn consume_reagents(
        &self,
        state: GameStateRef,
        component: &ActiveReactionComponent,
        event: &BeginReactionEvent,
        reagent_entities: &FxHashMap<Reagent, Target>,
    ) -> ReactionResult;
}

impl ReactionExecution for ReactionDefinition {
    #[allow(unreachable_patterns)]
    fn produce_products(
        &self,
        state: GameStateRef,
        component: &ActiveReactionComponent,
        _reagent_entities: &FxHashMap<Reagent, Target>,
    ) -> ReactionResult {
        use rl_core::rand::{thread_rng, Rng};

        if let Some(product) = self.product.as_ref() {
            let map = state.resources.get::<Map>().unwrap();
            let spatial_map = state.resources.get::<SpatialMap>().unwrap();
            let spawn_channel = state.resources.get::<Channel<SpawnEvent>>().unwrap();
            let src_coord = **state
                .world
                .get_component::<PositionComponent>(component.event.target.entity())
                .unwrap();

            let reaction_target_material = if let Some(material) = state
                .world
                .get_component::<MaterialComponent>(component.event.target.entity())
            {
                *material
            } else {
                // TODO: how do we discern the task source material?
                // TODO: task can have a tile target or something or reagent for reaction?
                MaterialComponent::new(map.get(src_coord).material.into(), MaterialState::Solid)
            };

            let mut pos_cache = SmallVec::<[Vec3i; 10]>::with_capacity(product.count);

            // TODO: seed
            let mut rng = thread_rng();

            for _ in 0..product.count {
                // TODO: check material limits
                // just pull from src for now

                if let Some(random) = product.random.as_ref() {
                    if rng.gen_range(0.0, 1.0) >= random.chance {
                        continue;
                    }
                }

                let target_coord = {
                    let target_coord = src_coord;

                    // For channeling only: we drop the item 1 z-level below, and do it here so no damage is applied.
                    // TODO: special case like this feels dirty

                    // Spawn the item. It should spawn on the same tile as the target.
                    // If theres a container here, it should enter the container.

                    // If the tile is occupied, we should walk around the tile until we find an empty one

                    if spatial_map
                        .locate_at_point(&PositionComponent::new(target_coord))
                        .is_none()
                        && !pos_cache.contains(&target_coord)
                    {
                        pos_cache.push(target_coord);
                        target_coord
                    } else {
                        // Find a neighbor
                        let mut neighbor_coord = None;
                        let mut search_coord = target_coord;
                        while neighbor_coord.is_none() {
                            for neighbor in map.neighbors_3d(&search_coord).iter() {
                                if !map.get(*neighbor).is_walkable() {
                                    continue;
                                }

                                let any_item = spatial_map
                                    .locate_all_at_point(&PositionComponent::new(*neighbor))
                                    .find(|entry| {
                                        state.world.has_component::<ItemComponent>(entry.entity)
                                    });

                                if any_item.is_none() && !pos_cache.contains(&neighbor) {
                                    neighbor_coord = Some(*neighbor);
                                    break;
                                }
                            }
                            if neighbor_coord.is_none() {
                                // TODO: do a proper circle range around instead
                                search_coord += Vec3i::new(1, 0, 0);
                            }
                        }

                        pos_cache.push(neighbor_coord.unwrap());
                        neighbor_coord.unwrap()
                    }
                };

                match &product.kind {
                    ProductKind::Item(item_ref) => {
                        // TODO: call item spawners
                        spawn_channel
                            .write(SpawnEvent {
                                target: Target::Position(TargetPosition::Tile(target_coord)),
                                kind: SpawnArguments::Item {
                                    material: reaction_target_material,
                                },
                                id: item_ref.id().into(),
                                arguments: (),
                            })
                            .unwrap();
                    }
                    _ => unimplemented!("Havnt implemented other product types yet"),
                }
            }
        }

        ReactionResult::Success
    }

    #[allow(clippy::single_match)] // TODO: Finish these
    fn check(
        &self,
        state: GameStateRef,
        source: Target,
        target: Target,
    ) -> Result<FxHashMap<Reagent, Target>, Reagent> {
        let mut entities =
            FxHashMap::with_capacity_and_hasher(self.reagents.len(), FxBuildHasher::default());

        for reagent in &self.reagents {
            if rl_core::condition::check(
                state.world,
                state.resources,
                source,
                target,
                &reagent.conditions,
                false,
            )
            .is_err()
            {
                return Err(reagent.clone());
            } else {
                entities.insert(reagent.clone(), target);
            }
        }

        Ok(entities)
    }

    #[allow(clippy::single_match)] // TODO: Finish these
    fn check_designate(
        &self,
        state: GameStateRef,
        source: Target,
        target: Target,
    ) -> Result<FxHashMap<Reagent, Target>, Reagent> {
        let mut entities =
            FxHashMap::with_capacity_and_hasher(self.reagents.len(), FxBuildHasher::default());

        for reagent in &self.reagents {
            if rl_core::condition::check(
                state.world,
                state.resources,
                source,
                target,
                &reagent.conditions,
                true,
            )
            .is_err()
            {
                return Err(reagent.clone());
            } else {
                entities.insert(reagent.clone(), target);
            }
        }

        Ok(entities)
    }

    #[allow(clippy::never_loop)] // TODO: Below
    fn apply(
        &self,
        state: GameStateRef,
        component: &ActiveReactionComponent,
        event: &BeginReactionEvent,
    ) -> ReactionResult {
        let effect_table = state.resources.get::<ReactionEffectTable>().unwrap();

        match self.check(
            state,
            Target::Entity(event.initiator.unwrap().entity()),
            Target::Entity(event.target.entity()),
        ) {
            Ok(reagent_entities) => {
                for effect in &self.effects {
                    let construct_reaction_fn = effect_table
                        .get(&fnv_hash(effect.name.as_str()))
                        .unwrap_or_else(|| panic!("Failed to find effect: {}", &effect.name));
                    let mut reaction_obj =
                        (construct_reaction_fn)(state, self, component, event, &reagent_entities);
                    // TODO:
                    return reaction_obj.tick(state, self, component, event, &reagent_entities);
                }
                ReactionResult::Success
            }
            Err(_) => ReactionResult::Failure,
        }
    }

    fn consume_reagents(
        &self,
        state: GameStateRef,
        _component: &ActiveReactionComponent,
        _event: &BeginReactionEvent,
        reagent_entities: &FxHashMap<Reagent, Target>,
    ) -> ReactionResult {
        for reagent in &self.reagents {
            if reagent.consume_chance > 0 {
                if let Some(target) = reagent_entities.get(&reagent) {
                    state
                        .resources
                        .get_mut::<GlobalCommandBuffer>()
                        .unwrap()
                        .add_component(target.entity().unwrap(), Destroy::default());
                }
            }
        }

        ReactionResult::Success
    }
}

pub type ReactionEffectProducer = fn(
    GameStateRef,
    &ReactionDefinition,
    &ActiveReactionComponent,
    &BeginReactionEvent,
    &FxHashMap<Reagent, Target>,
) -> Box<dyn ReactionEffect>;

pub type ReactionEffectTable =
    HashMap<u64, ReactionEffectProducer, std::hash::BuildHasherDefault<rl_core::FnvHasher>>;

pub struct ReactionEffectRegistration {
    name: String,
    producer: ReactionEffectProducer,
}
impl ReactionEffectRegistration {
    pub fn new(name: &str, producer: ReactionEffectProducer) -> Self {
        Self {
            name: name.to_string(),
            producer,
        }
    }
}

pub fn create_effect_table() -> ReactionEffectTable {
    let mut effects = ReactionEffectTable::default();

    for effect in effect_registration() {
        effects.insert(fnv_hash(&effect.name), effect.producer);
    }

    effects
}

#[derive(Default)]
pub struct ProduceItemEffect;
impl ReactionEffect for ProduceItemEffect {
    fn name() -> &'static str {
        "ProduceItemEffect"
    }

    fn tick(
        &mut self,
        _: GameStateRef,
        _: &ReactionDefinition,
        _: &ActiveReactionComponent,
        _: &BeginReactionEvent,
        _: &FxHashMap<Reagent, Target>,
    ) -> ReactionResult {
        ReactionResult::Success
    }
}

#[derive(Default)]
pub struct TreeChopEffect;
impl ReactionEffect for TreeChopEffect {
    fn name() -> &'static str {
        "TreeChopEffect"
    }

    fn tick(
        &mut self,
        state: GameStateRef,
        _reaction: &ReactionDefinition,
        _component: &ActiveReactionComponent,
        event: &BeginReactionEvent,
        _entities: &FxHashMap<Reagent, Target>,
    ) -> ReactionResult {
        use rl_core::components::VirtualTaskTag;
        let tree_entity = {
            if state
                .world
                .get_tag::<VirtualTaskTag>(event.target.entity())
                .is_some()
            {
                // It was a virtual task target, the target entity is in the same tile.
                let spatial_map = state.resources.get::<SpatialMap>().unwrap();
                let position = state
                    .world
                    .get_component::<PositionComponent>(event.target.entity())
                    .unwrap();

                spatial_map
                    .locate_all_at_point(&position)
                    .find_map(|entry| {
                        state
                            .world
                            .get_tag::<FoliageTag>(entry.entity)
                            .and_then(|foliage| {
                                if **foliage == FoliageKind::Tree {
                                    Some(entry.entity)
                                } else {
                                    None
                                }
                            })
                    })
                    .unwrap()
            } else {
                event.target.entity()
            }
        };

        let command_buffer =
            unsafe { <Write<GlobalCommandBuffer>>::fetch_unchecked(state.resources) };

        command_buffer.add_component(tree_entity, Destroy::default());

        ReactionResult::Success
    }
}
