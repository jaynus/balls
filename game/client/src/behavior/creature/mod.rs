use super::nodes as general_nodes;
use crate::behavior::ExecuteReactionParameters;
use rl_ai::bt::{self, make, BehaviorNode, BehaviorStatus};
use rl_core::{
    components::PositionComponent,
    data::bt::*,
    defs::{
        foliage::{FoliageComponent, FoliageDefinition},
        needs::NeedKind,
        reaction::ReactionDefinition,
        DefinitionComponent, DefinitionStorage,
    },
    failure, fnv,
    legion::prelude::*,
    map::spatial::StaticSpatialMap,
    GameStateRef,
};
use rl_reaction::ReactionEntity;
use std::sync::Arc;

pub fn build(storage: &mut bt::BehaviorStorage) -> Result<(), failure::Error> {
    let try_graze = make::sequence(&[
        make::selector(&[
            nodes::make_try_find_nearest_consumable_foliage(NeedKind::Calories),
            nodes::make_try_find_nearest_consumable_tile(NeedKind::Calories),
        ]),
        make::closure(None, general_nodes::move_to),
        nodes::make_consume_target(),
        make::closure(None, general_nodes::execute_reaction),
    ]);
    storage.insert("try_graze", try_graze);

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct CreatureConsumeParameters {
    target: Entity,
    kind: NeedKind,
}

pub mod nodes {
    use super::*;

    pub fn make_consume_target() -> Arc<dyn BehaviorNode> {
        make::closure(None, |state, args| {
            if let Some(parameters) = args
                .blackboard
                .get::<CreatureConsumeParameters>(fnv!("CreatureConsumeParameters"))
                .cloned()
            {
                log::trace!(target: "behavior", "Executing reaction");

                let reaction = state
                    .resources
                    .get::<DefinitionStorage<ReactionDefinition>>()
                    .unwrap()
                    .get_id("Consume (Foliage)")
                    .unwrap();

                args.blackboard.insert(
                    fnv!("ExecuteReactionParameters"),
                    ExecuteReactionParameters {
                        reaction,
                        target: ReactionEntity::Any(parameters.target),
                    },
                );

                return BehaviorStatus::success();
            }

            BehaviorStatus::failure()
        })
    }

    pub fn make_try_find_nearest_consumable_tile(_: NeedKind) -> Arc<dyn BehaviorNode> {
        make::closure(None, |_, _| BehaviorStatus::failure())
    }

    pub fn make_try_find_nearest_consumable_foliage(kind: NeedKind) -> Arc<dyn BehaviorNode> {
        make::closure(None, move |state, args| {
            let (static_spatial_map, foliage_defs) = <(
                Read<StaticSpatialMap>,
                Read<DefinitionStorage<FoliageDefinition>>,
            )>::fetch(&state.resources);

            let position = state
                .world
                .get_component::<PositionComponent>(args.entity)
                .unwrap();

            let has_target = args.blackboard.contains(fnv!("CreatureConsumeParameters"));

            if !has_target {
                // TODO: better selection, just pick closest for now
                if let Some(found) =
                    static_spatial_map
                        .nearest_neighbor_iter(&position)
                        .find(|entry| {
                            if let Some(comp) =
                                state.world.get_component::<FoliageComponent>(entry.entity)
                            {
                                if let Some(_) = crate::behavior::needs::get_nutrition_value(
                                    state,
                                    entry.entity,
                                    NeedKind::Calories,
                                ) {
                                    return true;
                                }
                            }

                            false
                        })
                {
                    log::trace!(target: "behavior", "found consumption entity, moving to target = {:?}", found.entity);

                    args.blackboard.insert(
                        fnv!("CreatureConsumeParameters"),
                        CreatureConsumeParameters {
                            target: found.entity,
                            kind,
                        },
                    );

                    args.blackboard.insert(
                        fnv!("MoveParameters"),
                        MoveParameters::new_tile(found.position()),
                    );

                    return BehaviorStatus::success();
                }

                BehaviorStatus::failure()
            } else {
                // Validate its still a valid target
                let target = args
                    .blackboard
                    .get::<CreatureConsumeParameters>(fnv!("CreatureConsumeParameters"))
                    .unwrap()
                    .target;
                if !state.world.is_alive(target) {
                    args.blackboard.remove(fnv!("CreatureConsumeParameters"));

                    BehaviorStatus::failure()
                } else {
                    BehaviorStatus::success()
                }
            }
        })
    }
}
