use rl_ai::bt::{self, make, BehaviorStatus};
use rl_core::{
    components::PositionComponent, data::Target, defs::reaction::ReactionDefinitionId, fnv,
    legion::prelude::*, smallvec::SmallVec, GameStateRef,
};

pub mod creature;
pub mod haul;
pub mod needs;
pub mod task;

#[derive(Copy, Clone, Debug)]
pub struct ExecuteReactionParameters {
    reaction: ReactionDefinitionId,
    target: rl_reaction::ReactionEntity,
}

pub fn prepare(_: &mut World, resources: &mut Resources) -> Result<(), anyhow::Error> {
    resources.insert(build()?);

    Ok(())
}

// Behaviors require being built and runtime and cant use inventory because of make::sub dependencies to copy an already registered node in
fn build() -> Result<bt::BehaviorStorage, anyhow::Error> {
    let mut storage = bt::BehaviorStorage::default();

    storage.insert(
        "Idle",
        make::sequence(&[make::closure(None, |_, _| {
            bt::BehaviorStatus::running(true)
        })]),
    );

    storage.insert("move_to", make::closure(None, nodes::move_to));

    haul::build(&mut storage)?;
    task::build(&mut storage)?;
    needs::build(&mut storage)?;
    creature::build(&mut storage)?;

    let do_work = make::selector(&[
        make::sub("do_task", &storage),
        make::sub("do_haul", &storage),
    ]);
    storage.insert("do_work", do_work);

    Ok(storage)
}

pub mod nodes {
    use super::*;
    use rl_ai::bt::*;
    use rl_core::{
        components::{MovementComponent, MovementRequest, MovementResult},
        data::bt::*,
        defs::{
            item::{ItemDefinition, ItemProperty},
            DefinitionComponent, DefinitionStorage,
        },
        event::Channel,
        AtomicResult,
    };
    use rl_reaction::{BeginReactionEvent, ReactionEntity, ReactionResult};
    use std::sync::Arc;

    pub use super::haul::nodes::*;
    pub use super::task::nodes::*;

    pub fn move_to(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        let source_entity = args.entity;
        if let Some(parameters) = args
            .blackboard
            .get_mut::<MoveParameters>(fnv!("MoveParameters"))
        {
            let target_tile = parameters.target.position(state.world).unwrap();

            let current_tile = **state
                .world
                .get_component::<PositionComponent>(source_entity)
                .unwrap();

            if current_tile == target_tile {
                args.command_buffer
                    .remove_component::<MovementResult>(source_entity);

                return BehaviorStatus::success();
            }

            let early_result = || {
                if let Some(our_request) = parameters.active_request {
                    if let Some(result) = state.world.get_component::<MovementResult>(source_entity)
                    {
                        if our_request == result.request {
                            if result.result.is_ok() {
                                return Some(BehaviorStatus::success());
                            } else {
                                return Some(BehaviorStatus::failure());
                            }
                        }
                    }
                }
                None
            };

            if let Some(early_result) = early_result() {
                args.command_buffer
                    .remove_component::<MovementResult>(source_entity);

                return early_result;
            }

            // We arnt there yet, are we moving there?
            let request = MovementRequest::with_distance(current_tile, target_tile, 0);
            let already_assigned = *state
                .world
                .get_component::<MovementComponent>(source_entity)
                .unwrap()
                == request;

            if !already_assigned {
                unsafe {
                    state
                        .world
                        .get_component_mut_unchecked::<MovementComponent>(source_entity)
                }
                .unwrap()
                .current = Some(request);

                parameters.active_request = Some(request);
            }
            return BehaviorStatus::running(false);
        } else {
        }

        BehaviorStatus::failure()
    }

    pub fn make_find_has_items_with_property(property: ItemProperty) -> Arc<dyn BehaviorNode> {
        make::sequence(&[
            make::closure(None, move |_, args| {
                args.blackboard
                    .insert::<ItemProperty>(fnv!("FindItemProperty"), property);
                BehaviorStatus::success()
            }),
            make::closure(None, find_has_items_with_property),
        ])
    }

    pub fn find_has_items_with_property(
        state: GameStateRef,
        args: &mut BehaviorArgs<'_>,
    ) -> BehaviorStatus {
        let items = state
            .resources
            .get::<DefinitionStorage<ItemDefinition>>()
            .unwrap();

        let mut found_items = Vec::default();

        if let Some(item_property) = args
            .blackboard
            .get::<ItemProperty>(fnv!("FindItemsProperty"))
            .cloned()
        {
            rl_core::inventory::for_all_items_recursive(
                args.entity,
                state.world,
                |_, (entity, comp)| {
                    let def = comp.fetch(&items);
                    if def.properties.contains(item_property) {
                        found_items.push(entity);
                    }
                },
            );
            args.blackboard.remove(fnv!("FindItemsProperty"));
        }

        let result = if found_items.is_empty() {
            BehaviorStatus::failure()
        } else {
            BehaviorStatus::success()
        };

        args.blackboard
            .insert(fnv!("FoundItemsProperty"), found_items);

        result
    }

    pub fn make_has_item_with_property(property: ItemProperty) -> Arc<dyn BehaviorNode> {
        make::sequence(&[
            make::closure(None, move |_, args| {
                args.blackboard
                    .insert::<ItemProperty>(fnv!("HasItemProperty"), property);
                BehaviorStatus::success()
            }),
            make::closure(None, has_item_with_property),
        ])
    }

    #[allow(clippy::block_in_if_condition_stmt)]
    pub fn has_item_with_property(
        state: GameStateRef,
        args: &mut BehaviorArgs<'_>,
    ) -> BehaviorStatus {
        let items = state
            .resources
            .get::<DefinitionStorage<ItemDefinition>>()
            .unwrap();

        if let Some(item_property) = args
            .blackboard
            .get::<ItemProperty>(fnv!("HasItemProperty"))
            .cloned()
        {
            let result = if rl_core::inventory::find_item_recursive(
                args.entity,
                state.world,
                |_, (_, comp)| {
                    let def = comp.fetch(&items);
                    def.properties.contains(item_property)
                },
            )
            .is_some()
            {
                BehaviorStatus::success()
            } else {
                BehaviorStatus::failure()
            };
            args.blackboard.remove(fnv!("HasItemProperty"));
            result
        } else {
            BehaviorStatus::failure()
        }
    }

    pub fn has_item(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        if let Some(item) = args.blackboard.get::<Entity>(fnv!("HasItem")).cloned() {
            let result = if rl_core::inventory::find_item_recursive(
                args.entity,
                state.world,
                |_, (item_entity, _)| item_entity == item,
            )
            .is_some()
            {
                BehaviorStatus::success()
            } else {
                BehaviorStatus::failure()
            };
            args.blackboard.remove(fnv!("HasItem"));
            result
        } else {
            BehaviorStatus::failure()
        }
    }

    pub fn try_get_reagent(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        use rl_core::{
            components::ItemContainerChildComponent,
            defs::{item::ItemComponent, reaction::Reagent},
            math::Vec3i,
            Distance,
        };

        // TODO: ACTUALLY SEARCH IN CORRECT ORDER
        //let stockpile_query = <Read<ItemComponent>>::query()
        //    .filter(component::<StockpileItemChildComponent>());
        //let spatial_map = state.resources.get::<SpatialMap>().unwrap();

        let solo_item_query = <(Read<PositionComponent>, Read<ItemComponent>)>::query()
            .filter(!component::<ItemContainerChildComponent>());

        let mut matches = SmallVec::<[(Vec3i, Entity); 1024]>::default();
        if let Some(reagent) = args
            .blackboard
            .get::<Reagent>(fnv!("missing_reagent"))
            .cloned()
        {
            for set in &reagent.conditions {
                if matches.is_empty() {
                    matches.extend(solo_item_query.iter_entities(state.world).filter_map(
                        |(entity, (position, _))| {
                            // TODO: we need to be able to treat the "target" as "self" for this kind of request.
                            // TODO: for now we check both. This needs to be better designed
                            rl_core::condition::check_set(
                                set,
                                state.world,
                                state.resources,
                                Target::Entity(args.entity),
                                Target::Entity(entity),
                                false,
                            )
                            .map_or_else(
                                |_| {
                                    rl_core::condition::check_set(
                                        set,
                                        state.world,
                                        state.resources,
                                        Target::Entity(entity),
                                        Target::Entity(args.entity),
                                        false,
                                    )
                                    .map_or(None, |_| Some((**position, entity)))
                                },
                                |_| Some((**position, entity)),
                            )
                        },
                    ));
                } else {
                    matches.retain(|(_, entity)| {
                        rl_core::condition::check_set(
                            set,
                            state.world,
                            state.resources,
                            Target::Entity(*entity),
                            Target::Entity(args.entity),
                            false,
                        )
                        .is_ok()
                    });
                }
            }
            if !matches.is_empty() {
                let src_position = **state
                    .world
                    .get_component::<PositionComponent>(args.entity)
                    .unwrap();

                matches.sort_by(|a, b| {
                    src_position
                        .distance(&a.0)
                        .partial_cmp(&src_position.distance(&b.0))
                        .unwrap()
                });
                args.blackboard.insert(
                    fnv!("PickupParameters"),
                    PickupParameters::new(matches[0].1),
                );
                args.blackboard.remove(fnv!("missing_reagent"));
                return BehaviorStatus::success();
            }
        }

        BehaviorStatus::failure()
    }

    pub fn execute_reaction(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        let parameters = args
            .blackboard
            .get::<ExecuteReactionParameters>(fnv!("ExecuteReactionParameters"))
            .cloned();

        if let Some(parameters) = parameters {
            let result = if let Some(result) = args
                .blackboard
                .get::<AtomicResult<ReactionResult>>(fnv!("do_task_atomic_flag"))
            {
                match result.get() {
                    ReactionResult::Created | ReactionResult::Running => {
                        BehaviorStatus::running(false)
                    }
                    ReactionResult::Success => BehaviorStatus::success(),
                    ReactionResult::Failure => BehaviorStatus::failure(),
                }
            } else {
                let flag = AtomicResult::<ReactionResult>::default();

                args.blackboard
                    .insert(fnv!("do_task_atomic_flag"), flag.clone());

                state
                    .resources
                    .get::<Channel<BeginReactionEvent>>()
                    .unwrap()
                    .write(BeginReactionEvent::new(
                        parameters.reaction,
                        Some(ReactionEntity::Pawn(args.entity)),
                        parameters.target,
                        Some(Arc::new(move |r| flag.set(r))),
                    ))
                    .unwrap();

                return BehaviorStatus::running(false);
            };

            if result != BehaviorStatus::running(false) {
                args.blackboard.remove(fnv!("do_task_atomic_flag"));
            }

            result
        } else {
            BehaviorStatus::failure()
        }
    }
}
