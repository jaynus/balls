use rl_ai::bt::{self, make, BehaviorStatus};
use rl_core::{
    components::PositionComponent, defs::reaction::ReactionDefinitionId, failure, fnv,
    legion::prelude::*, GameStateRef,
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

pub fn prepare(_: &mut World, resources: &mut Resources) -> Result<(), failure::Error> {
    resources.insert(build()?);

    Ok(())
}

// Behaviors require being built and runtime and cant use inventory because of make::sub dependencies to copy an already registered node in
fn build() -> Result<bt::BehaviorStorage, failure::Error> {
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
            let target_tile = match parameters.target {
                Target::Entity(e) => **state.world.get_component::<PositionComponent>(e).unwrap(),
                Target::Tile(v) => v,
            };

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
        use rl_core::defs::reaction::Reagent;

        if let Some(kind) = args.blackboard.get::<Reagent>(fnv!("missing_reagent")) {
            /*
            match kind {
                Kind::Item(item_ref) => {
                    // Search stockpiles first
                    let stockpile_query = <Read<ItemComponent>>::query()
                        .filter(component::<StockpileItemChildComponent>());

                    // TODO: this needs to use spatial_map b distance instaed
                    for (entity, item_comp) in stockpile_query.iter_entities(state.world) {
                        if *item_ref == *item_comp {
                            let pickup_parameters = PickupParameters::new(entity);

                            args.blackboard
                                .insert(fnv!("PickupParameters"), pickup_parameters);
                            return BehaviorStatus::success();
                        }
                    }

                    // TODO: search random items too?
                }
                Kind::ItemAbility(ability) => {
                    // Ability, we can try to get a tool with this ability

                    let query = <Read<ItemComponent>>::query().filter(
                        component::<PositionComponent>()
                            & !component::<ItemContainerChildComponent>(),
                    );

                    let item_defs = state
                        .resources
                        .get::<DefinitionStorage<ItemDefinition>>()
                        .unwrap();

                    for (entity, item_comp) in query.iter_entities(state.world) {
                        let def = item_comp.fetch(&item_defs);
                        // TODO: item quality
                        if def.abilities.contains(&ability.kind.into()) {
                            let pickup_parameters = PickupParameters::new(entity);

                            args.blackboard
                                .insert(fnv!("PickupParameters"), pickup_parameters);
                            return BehaviorStatus::success();
                        }
                    }
                }
                _ => unimplemented!(),
            }
            */
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
