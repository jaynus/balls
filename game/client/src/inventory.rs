//! inventory helpers
use rl_core::defs::{
    item::{ItemComponent, ItemDefinition},
    DefinitionComponent, DefinitionStorage,
};
use rl_core::{
    components::{
        CarryComponent, ItemContainerChildComponent, ItemContainerComponent, PositionComponent,
    },
    data::DimensionsVec,
    dispatcher::{DispatcherBuilder, Stage},
    legion::{prelude::*, systems::SubWorld},
    smallvec::SmallVec,
};
use rl_render_pod::sprite::Sprite;
use std::cmp::Ordering;

pub fn bundle(
    _: &mut World,
    _resources: &mut Resources,
    builder: &mut DispatcherBuilder,
) -> Result<(), anyhow::Error> {
    builder.add_system(Stage::Logic, build_containers_insert_system);
    builder.add_system(Stage::Logic, build_containers_update_children);
    Ok(())
}

pub fn build_containers_update_children(_: &mut World, _: &mut Resources) -> Box<dyn Schedulable> {
    SystemBuilder::<()>::new("clean_item_sprite_system")
        .with_query(
            <(Read<PositionComponent>, Write<ItemContainerComponent>)>::query()
                .filter(changed::<ItemContainerComponent>() | changed::<PositionComponent>()),
        )
        .with_query(
            <(Read<PositionComponent>, Write<CarryComponent>)>::query()
                .filter(changed::<CarryComponent>() | changed::<PositionComponent>()),
        )
        .write_component::<PositionComponent>()
        .build(
            move |_, world, _, (changed_containers_query, changed_carry_query)| {
                game_metrics::scope!("clean_item_sprite_system");

                changed_carry_query.iter_entities_mut(world).for_each(
                    |(_entity, (position, mut carrying))| {
                        carrying.limbs.iter_mut().for_each(|(_, v)| {
                            let mut clear = false;
                            if let Some(v) = v {
                                if !world.is_alive(*v) {
                                    clear = true;
                                }
                            }
                            if clear {
                                *v = None;
                            }
                        });

                        for child in carrying.iter() {
                            *unsafe {
                                world
                                    .get_component_mut_unchecked::<PositionComponent>(child)
                                    .unwrap()
                            } = *position;
                        }
                    },
                );

                changed_containers_query.iter_entities_mut(world).for_each(
                    |(_entity, (position, mut container))| {
                        container.inside.retain(|v| world.is_alive(*v));

                        for child in &container.inside {
                            *unsafe {
                                world
                                    .get_component_mut_unchecked::<PositionComponent>(*child)
                                    .unwrap()
                            } = *position;
                        }
                    },
                );
            },
        )
}

pub fn build_containers_insert_system(_: &mut World, _: &mut Resources) -> Box<dyn Schedulable> {
    SystemBuilder::<()>::new("containers_insert_system")
        .read_resource::<DefinitionStorage<ItemDefinition>>()
        .read_component::<ItemComponent>()
        .with_query(
            <Write<ItemContainerComponent>>::query().filter(changed::<ItemContainerComponent>()),
        )
        .with_query(
            <Read<ItemComponent>>::query()
                .filter(!component::<Sprite>() & !component::<ItemContainerChildComponent>()),
        )
        .build(
            move |command_buffer,
                  world,
                  item_defs,
                  (containers_query, item_without_sprite_query)| {
                fn update_container_capacity(
                    world: &SubWorld,
                    container: &mut ItemContainerComponent,
                    defs: &DefinitionStorage<ItemDefinition>,
                ) -> DimensionsVec {
                    container.consumed =
                        container
                            .inside
                            .iter()
                            .fold(DimensionsVec::new(0, 0, 0), |acc, item| {
                                acc + world
                                    .get_component::<ItemComponent>(*item)
                                    .unwrap()
                                    .fetch(defs)
                                    .dimensions
                            });
                    container.consumed
                }

                game_metrics::scope!("containers_insert_system");

                item_without_sprite_query
                    .iter_entities_mut(world)
                    .for_each(|(entity, item)| {
                        command_buffer
                            .add_component::<Sprite>(entity, item.fetch(&item_defs).sprite.make());
                    });

                for (_, mut changed_container) in containers_query.iter_entities_mut(world) {
                    let mut consumed =
                        update_container_capacity(world, &mut changed_container, &item_defs);
                    let capacity = changed_container.capacity;

                    let mut new_items = changed_container
                        .queued_inside
                        .drain(..)
                        .filter(|queued_item| {
                            let item_def = world
                                .get_component::<ItemComponent>(*queued_item)
                                .unwrap()
                                .fetch(&item_defs);

                            // Attempt to insert the items into the container
                            // Otherwise, we drop it
                            if let Ordering::Greater = rl_core::inventory::compare_volumes(
                                consumed + item_def.dimensions,
                                capacity,
                            ) {
                                command_buffer
                                    .remove_component::<ItemContainerChildComponent>(*queued_item);
                                command_buffer
                                    .add_component::<Sprite>(*queued_item, item_def.sprite.make());
                                // TODO: Drop it
                                false
                            } else {
                                command_buffer.remove_component::<Sprite>(*queued_item);

                                consumed += item_def.dimensions;
                                true
                            }
                        })
                        .collect::<SmallVec<[Entity; 3]>>();
                    new_items
                        .drain(..)
                        .for_each(|item| changed_container.inside.push(item));

                    changed_container.consumed = consumed;
                }
            },
        )
}
