use super::nodes as general_nodes;
use crate::behavior::ExecuteReactionParameters;
use rl_ai::bt::{self, make, BehaviorNode, BehaviorStatus};
use rl_core::{
    components::{ItemContainerChildComponent, PositionComponent},
    data::bt::*,
    defs::{
        foliage::{FoliageComponent, FoliageDefinition},
        item::{ItemComponent, ItemDefinition, ItemProperty},
        material::{MaterialComponent, MaterialDefinition},
        needs::{NeedKind, ProvidesNutrition},
        reaction::ReactionDefinition,
        DefinitionComponent, DefinitionStorage,
    },
    fnv,
    legion::prelude::*,
    map::spatial::SpatialMap,
    GameStateRef,
};
use rl_reaction::ReactionEntity;
use std::{ops::Range, sync::Arc};

#[allow(clippy::too_many_lines)]
pub fn build(storage: &mut bt::BehaviorStorage) -> Result<(), anyhow::Error> {
    let try_eat = make::selector(&[make::sequence(&[make::selector(&[
        make::sequence(&[
            general_nodes::make_find_has_items_with_property(ItemProperty::IsEdible),
            make::closure(None, |state, args| {
                let found = args
                    .blackboard
                    .remove_get::<Vec<Entity>>(fnv!("FoundItemsProperty"))
                    .unwrap();

                for entity in found {
                    if let Some(nut) = get_nutrition_value(state, entity, NeedKind::Calories) {
                        if nut.start > 0 {
                            let reaction = state
                                .resources
                                .get::<DefinitionStorage<ReactionDefinition>>()
                                .unwrap()
                                .get_id("Consume (Any)")
                                .unwrap();

                            args.blackboard.insert(
                                fnv!("ExecuteReactionParameters"),
                                ExecuteReactionParameters {
                                    reaction,
                                    target: ReactionEntity::Any(entity),
                                },
                            );

                            return BehaviorStatus::success();
                        }
                    }
                }

                BehaviorStatus::failure()
            }),
        ]),
        make::sequence(&[
            // TODO: Check we can actually get it? LOL
            nodes::make_try_find_nearest_consumable(NeedKind::Calories),
            make::sub("pickup_item", storage),
            make::closure(None, |state, args| {
                if let Some(pickup) = args
                    .blackboard
                    .get::<PickupParameters>(fnv!("PickupParameters"))
                    .cloned()
                {
                    let reaction = state
                        .resources
                        .get::<DefinitionStorage<ReactionDefinition>>()
                        .unwrap()
                        .get_id("Consume (Any)")
                        .unwrap();

                    args.blackboard.insert(
                        fnv!("ExecuteReactionParameters"),
                        ExecuteReactionParameters {
                            reaction,
                            target: ReactionEntity::Any(pickup.target),
                        },
                    );
                    return BehaviorStatus::success();
                }

                BehaviorStatus::failure()
            }),
            make::closure(None, general_nodes::execute_reaction),
        ]),
    ])])]);
    storage.insert("try_eat", try_eat);

    let try_drink = make::selector(&[make::sequence(&[make::selector(&[
        make::sequence(&[
            general_nodes::make_find_has_items_with_property(ItemProperty::IsEdible),
            make::closure(None, |state, args| {
                let found = args
                    .blackboard
                    .remove_get::<Vec<Entity>>(fnv!("FoundItemsProperty"))
                    .unwrap();

                for entity in found {
                    if let Some(nut) = get_nutrition_value(state, entity, NeedKind::Hydration) {
                        if nut.start > 0 {
                            let reaction = state
                                .resources
                                .get::<DefinitionStorage<ReactionDefinition>>()
                                .unwrap()
                                .get_id("Consume (Any)")
                                .unwrap();

                            args.blackboard.insert(
                                fnv!("ExecuteReactionParameters"),
                                ExecuteReactionParameters {
                                    reaction,
                                    target: ReactionEntity::Any(entity),
                                },
                            );

                            return BehaviorStatus::success();
                        }
                    }
                }

                BehaviorStatus::failure()
            }),
        ]),
        make::sequence(&[
            // TODO: Check we can actually get it? LOL
            nodes::make_try_find_nearest_consumable(NeedKind::Hydration),
            make::sub("pickup_item", storage),
            make::closure(None, |state, args| {
                if let Some(pickup) = args
                    .blackboard
                    .get::<PickupParameters>(fnv!("PickupParameters"))
                    .cloned()
                {
                    let reaction = state
                        .resources
                        .get::<DefinitionStorage<ReactionDefinition>>()
                        .unwrap()
                        .get_id("Consume (Any)")
                        .unwrap();

                    args.blackboard.insert(
                        fnv!("ExecuteReactionParameters"),
                        ExecuteReactionParameters {
                            reaction,
                            target: ReactionEntity::Any(pickup.target),
                        },
                    );
                    return BehaviorStatus::success();
                }

                BehaviorStatus::failure()
            }),
            make::closure(None, general_nodes::execute_reaction),
        ]),
    ])])]);
    storage.insert("try_drink", try_drink);

    Ok(())
}

pub mod nodes {
    use super::*;

    pub fn make_try_find_nearest_consumable(kind: NeedKind) -> Arc<dyn BehaviorNode> {
        make::closure(None, move |state, args| {
            let spatial_map = state.resources.get::<SpatialMap>().unwrap();

            let position = state
                .world
                .get_component::<PositionComponent>(args.entity)
                .unwrap();

            // TODO: better selection, just pick closest for now
            if let Some(found) = spatial_map.nearest_neighbor_iter(&position).find(|entry| {
                if !state
                    .world
                    .has_component::<ItemContainerChildComponent>(entry.entity)
                    && state
                        .world
                        .get_component::<ItemComponent>(entry.entity)
                        .is_some()
                {
                    if let Some(nut) = get_nutrition_value(state, entry.entity, kind) {
                        if nut.start > 0 {
                            return true;
                        }
                    }
                }

                false
            }) {
                log::trace!(target: "behavior", "found consumption entity, attempting to pickup = {:?}", found.entity);

                args.blackboard.insert(
                    fnv!("PickupParameters"),
                    PickupParameters::new(found.entity),
                );

                return BehaviorStatus::success();
            }

            BehaviorStatus::failure()
        })
    }
}

pub fn get_nutrition_value(
    state: GameStateRef,
    entity: Entity,
    kind: NeedKind,
) -> Option<Range<i32>> {
    let (items, materials, foliages) = <(
        Read<DefinitionStorage<ItemDefinition>>,
        Read<DefinitionStorage<MaterialDefinition>>,
        Read<DefinitionStorage<FoliageDefinition>>,
    )>::fetch(state.resources);

    let item_comp = state.world.get_component::<ItemComponent>(entity);
    let foliage_comp = state.world.get_component::<FoliageComponent>(entity);

    let optional_nutrition = if let Some(comp) = &item_comp {
        &comp.fetch(&items).nutrition
    } else if let Some(comp) = &foliage_comp {
        &comp.fetch(&foliages).nutrition
    } else {
        return None;
    };

    let nutrition = match &optional_nutrition {
        ProvidesNutrition::FromMaterial => {
            if item_comp.is_some() {
                if let Some(material_comp) = state.world.get_component::<MaterialComponent>(entity)
                {
                    let material_state = material_comp.fetch_state(&materials);

                    &material_state.nutrition
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        ProvidesNutrition::Value(value) => value,
    };

    Some(match kind {
        NeedKind::Calories => nutrition.calories.clone(),
        NeedKind::Hydration => nutrition.hydration.clone(),
        _ => unimplemented!(),
    })
}
