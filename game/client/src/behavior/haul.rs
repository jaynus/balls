use super::nodes as general_nodes;
use rl_ai::bt::{self, make, BehaviorStatus};

pub fn build(storage: &mut bt::BehaviorStorage) -> Result<(), anyhow::Error> {
    storage.insert(
        "pickup_item",
        make::selector(&[
            make::closure(None, general_nodes::has_item),
            make::sequence(&[
                make::closure(None, nodes::can_pickup),
                make::closure(None, nodes::designate_pickup),
                make::if_else(
                    make::closure(None, general_nodes::move_to),
                    make::closure(None, nodes::do_pickup),
                    make::closure(None, nodes::cancel_pickup),
                ),
            ]),
        ]),
    );

    let haul_item_to_target = make::sequence(&[
        make::closure(None, nodes::prepare_haul_parameters),
        make::sub("pickup_item", &storage),
        make::closure(None, nodes::prepare_haul_movement_target),
        make::if_else(
            make::closure(None, general_nodes::move_to),
            make::sequence(&[
                make::closure(None, nodes::do_drop),
                make::closure(None, nodes::make_item_stockpile_child),
            ]),
            make::closure(None, nodes::cancel_haul_stockpile),
        ),
    ]);
    storage.insert("haul_item_to_target", haul_item_to_target);

    let do_haul = make::sequence(&[
        make::closure(None, nodes::find_item_for_stockpile),
        make::sub("haul_item_to_target", &storage),
    ]);
    storage.insert("do_haul", do_haul);

    Ok(())
}

pub mod nodes {
    use super::*;
    use rl_ai::bt::*;
    use rl_core::defs::{
        item::{
            ItemComponent, ItemDefinition, StockpileComponent, StockpileItemChildComponent,
            StockpileSpatialMap,
        },
        DefinitionComponent, DefinitionStorage,
    };
    use rl_core::{
        components::{
            ActivePickupComponent, CarryComponent, ItemContainerChildComponent,
            ItemContainerComponent, PositionComponent,
        },
        data::{bt::*, Target},
        fnv,
        legion::prelude::*,
        map::spatial::SpatialMap,
        map::Map,
        time::Time,
        GameStateRef,
    };

    pub fn find_item_for_stockpile(
        state: GameStateRef,
        args: &mut BehaviorArgs<'_>,
    ) -> BehaviorStatus {
        if args.blackboard.contains(fnv!("HaulParameters")) {
            return BehaviorStatus::success();
        } else {
            let (spatial_map, stockpile_map, items) = <(
                Read<SpatialMap>,
                Read<StockpileSpatialMap>,
                Read<DefinitionStorage<ItemDefinition>>,
            )>::fetch(state.resources);
            // Find the nearest item which also has an open stockpile slot
            let source_position = state
                .world
                .get_component::<PositionComponent>(args.entity)
                .unwrap();

            // TODO: with distance somehow, find the bets mix?
            for item_entry in spatial_map.nearest_neighbor_iter(&source_position) {
                if let Some(item_component) = state
                    .world
                    .get_component::<ItemComponent>(item_entry.entity)
                {
                    // skip if its already in a stockpile, or is a child of someone else, or is active
                    if state
                        .world
                        .has_component::<ActivePickupComponent>(item_entry.entity)
                        || state
                            .world
                            .has_component::<StockpileItemChildComponent>(item_entry.entity)
                        || state
                            .world
                            .has_component::<ItemContainerChildComponent>(item_entry.entity)
                        || state
                            .world
                            .has_component::<ActivePickupComponent>(item_entry.entity)
                    {
                        continue;
                    }

                    let item = item_component.fetch(&items);

                    for stockpile_entry in stockpile_map.nearest_neighbor_iter(&source_position) {
                        let mut stockpile = unsafe {
                            state
                                .world
                                .get_component_mut_unchecked::<StockpileComponent>(
                                    stockpile_entry.entity,
                                )
                                .unwrap()
                        };
                        if stockpile.tiles.is_empty() {
                            continue;
                        }

                        if stockpile.stores.contains(item.kind) {
                            let item_position = state
                                .world
                                .get_component::<PositionComponent>(item_entry.entity)
                                .unwrap();

                            if let Some(target_tile) = stockpile.tiles.pop_nearest(&**item_position)
                            {
                                // Found a stockpile to store the item
                                args.blackboard.insert(
                                    fnv!("HaulParameters"),
                                    HaulParameters {
                                        stockpile: stockpile_entry.entity,
                                        item: item_entry.entity,
                                        target_tile,
                                    },
                                );
                                args.blackboard.insert(
                                    fnv!("PickupParameters"),
                                    PickupParameters {
                                        target: item_entry.entity,
                                        destination: None,
                                    },
                                );

                                return BehaviorStatus::success();
                            }
                        }
                    }
                }
            }
        }

        BehaviorStatus::failure()
    }

    pub fn make_item_stockpile_child(
        _state: GameStateRef,
        args: &mut BehaviorArgs<'_>,
    ) -> BehaviorStatus {
        if let Some(parameters) = args
            .blackboard
            .get::<HaulParameters>(fnv!("HaulParameters"))
            .cloned()
        {
            args.command_buffer.add_component(
                parameters.item,
                StockpileItemChildComponent {
                    parent: parameters.stockpile,
                },
            );

            args.blackboard.remove(fnv!("HaulParameters"));

            return BehaviorStatus::success();
        }

        BehaviorStatus::failure()
    }

    pub fn prepare_haul_movement_target(
        _state: GameStateRef,
        args: &mut BehaviorArgs<'_>,
    ) -> BehaviorStatus {
        if let Some(parameters) = args
            .blackboard
            .get::<HaulParameters>(fnv!("HaulParameters"))
            .cloned()
        {
            args.blackboard.insert(
                fnv!("MoveParameters"),
                MoveParameters::new_tile(parameters.target_tile, None),
            );

            return BehaviorStatus::success();
        }

        BehaviorStatus::failure()
    }

    pub fn prepare_haul_parameters(
        _state: GameStateRef,
        args: &mut BehaviorArgs<'_>,
    ) -> BehaviorStatus {
        if let Some(parameters) = args
            .blackboard
            .get::<HaulParameters>(fnv!("HaulParameters"))
            .cloned()
        {
            args.blackboard.insert(fnv!("HasItem"), parameters.item);

            args.blackboard.insert(
                fnv!("PickupParameters"),
                PickupParameters::new(parameters.item),
            );

            args.blackboard.insert(
                fnv!("DropParameters"),
                DropParameters::with_target(
                    parameters.item,
                    Target::from_position(parameters.target_tile),
                ),
            );
            return BehaviorStatus::success();
        }

        BehaviorStatus::failure()
    }

    pub fn cancel_haul_stockpile(
        state: GameStateRef,
        args: &mut BehaviorArgs<'_>,
    ) -> BehaviorStatus {
        if let Some(parameters) = args
            .blackboard
            .get::<HaulParameters>(fnv!("HaulParameters"))
            .cloned()
        {
            unsafe {
                state
                    .world
                    .get_component_mut_unchecked::<StockpileComponent>(parameters.stockpile)
                    .unwrap()
                    .tiles
                    .push(parameters.target_tile)
                    .unwrap();
            }
            args.command_buffer
                .remove_component::<ActivePickupComponent>(parameters.item);

            args.blackboard.remove(fnv!("HaulParameters"));

            return BehaviorStatus::success();
        }

        BehaviorStatus::failure()
    }

    pub fn find_stockpile(_state: GameStateRef, _: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        //if let Some(target) = args.BlackboardComponent.get::<Entity>(fnv!("target")) {}

        unimplemented!()
    }

    pub fn can_carry(_state: GameStateRef, _: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        //if let Some(target) = args.BlackboardComponent.get::<Entity>(fnv!("target")) {}

        unimplemented!()
    }

    pub fn do_carry(_state: GameStateRef, _: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        //if let Some(target) = args.BlackboardComponent.get::<Entity>(fnv!("target")) {}

        unimplemented!()
    }

    pub fn can_pickup(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        if let Some(parameters) = args
            .blackboard
            .get_mut::<PickupParameters>(fnv!("PickupParameters"))
        {
            let items = state
                .resources
                .get::<DefinitionStorage<ItemDefinition>>()
                .unwrap();
            if let Some(item_comp) = state
                .world
                .get_component::<ItemComponent>(parameters.target)
            {
                let item = item_comp.fetch(&items);

                if state
                    .world
                    .has_component::<ItemContainerChildComponent>(parameters.target)
                {
                    return BehaviorStatus::failure();
                }

                if let Some(comp) = state
                    .world
                    .get_component::<ActivePickupComponent>(parameters.target)
                {
                    if comp.initiator != args.entity {
                        return BehaviorStatus::failure();
                    }
                }

                // Do we have either the capacity in our bag, or a limb to pick it up?
                // TODO: How do we pick which?
                // Default bag for non weapon/tool unless no choice
                // TODO: for now, we just carry everything first if we can
                if let Some(carrying) = state.world.get_component::<CarryComponent>(args.entity) {
                    // TODO: can carry check. For now we just check an emtpy limb
                    for limb in &carrying.limbs {
                        if limb.1.is_none() {
                            parameters.destination = Some(PickupDestination::Carry(limb.0));

                            return BehaviorStatus::success();
                        }
                    }
                }

                if let Some(container) = state
                    .world
                    .get_component::<ItemContainerComponent>(args.entity)
                {
                    if rl_core::inventory::can_contain_item(&container, item) {
                        parameters.destination = Some(PickupDestination::Container(args.entity));
                        return BehaviorStatus::success();
                    }
                }
            }
        }

        BehaviorStatus::failure()
    }

    pub fn designate_pickup(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        let move_params = if let Some(parameters) = args
            .blackboard
            .get::<PickupParameters>(fnv!("PickupParameters"))
        {
            // Only try to pick it up if no one else is.
            if let Some(comp) = state
                .world
                .get_component::<ActivePickupComponent>(parameters.target)
            {
                if comp.initiator == args.entity {
                    return BehaviorStatus::success();
                }
                panic!("Unreachable");
            } else {
                log::trace!(target: "behavior", "Adding pickup component: initiator = {:?}", args.entity);
                log::trace!(target: "behavior", "target = {:?}", parameters.target);
                args.command_buffer.add_component(
                    parameters.target,
                    ActivePickupComponent::new(
                        args.entity,
                        state.resources.get::<Time>().unwrap().world_time,
                    ),
                );

                Some(MoveParameters::new_entity(parameters.target, None))
            }
        } else {
            None
        };

        if let Some(params) = move_params {
            args.blackboard.insert(fnv!("MoveParameters"), params);

            BehaviorStatus::success()
        } else {
            BehaviorStatus::failure()
        }
    }

    pub fn do_pickup(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        let mut res = BehaviorStatus::failure();

        if let Some(parameters) = args
            .blackboard
            .get::<PickupParameters>(fnv!("PickupParameters"))
        {
            if let Some(pickup_dest) = parameters.destination {
                match pickup_dest {
                    PickupDestination::Container(entity) => {
                        unsafe {
                            state
                                .world
                                .get_component_mut_unchecked::<ItemContainerComponent>(entity)
                        }
                        .unwrap()
                        .push(parameters.target);
                    }
                    PickupDestination::Carry(part) => {
                        let mut carrying = unsafe {
                            state
                                .world
                                .get_component_mut_unchecked::<CarryComponent>(args.entity)
                                .unwrap()
                        };
                        carrying.limbs.iter_mut().find(|i| i.0 == part).unwrap().1 =
                            Some(parameters.target);
                    }
                }

                // Remove the sprite, add our child.
                args.command_buffer
                    .remove_component::<rl_render_pod::sprite::Sprite>(parameters.target);
                args.command_buffer
                    .add_component::<ItemContainerChildComponent>(
                        parameters.target,
                        ItemContainerChildComponent {
                            parent: args.entity,
                        },
                    );

                args.command_buffer.add_component(
                    parameters.target,
                    ItemContainerChildComponent {
                        parent: args.entity,
                    },
                );
            }

            // Make the target a child of the caller

            args.command_buffer
                .remove_component::<ActivePickupComponent>(parameters.target);
            // TODO: significy pickup? or just handle container child?

            res = BehaviorStatus::success();
        }

        res
    }

    pub fn cancel_pickup(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        if let Some(parameters) = args
            .blackboard
            .get::<PickupParameters>(fnv!("PickupParameters"))
        {
            if state
                .world
                .has_component::<ActivePickupComponent>(parameters.target)
            {
                log::trace!(target: "behavior", "Removing pickup component: initiator = {:?}", args.entity);
                log::trace!(target: "behavior", "target = {:?}", parameters.target);
                args.command_buffer
                    .remove_component::<ActivePickupComponent>(parameters.target);
                return BehaviorStatus::success();
            }
        }
        BehaviorStatus::failure()
    }

    pub fn do_drop(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        if let Some(parameters) = args
            .blackboard
            .get::<DropParameters>(fnv!("DropParameters"))
        {
            if rl_core::inventory::remove_item(state.world, args.entity, parameters.item) {
                if let Some(target) = parameters.target {
                    **unsafe {
                        state
                            .world
                            .get_component_mut_unchecked::<PositionComponent>(args.entity)
                            .unwrap()
                    } = target.from_map(&state.resources.get::<Map>().unwrap()).1
                }

                args.command_buffer
                    .remove_component::<ItemContainerChildComponent>(parameters.item);

                return BehaviorStatus::success();
            }
        }

        BehaviorStatus::failure()
    }
}
