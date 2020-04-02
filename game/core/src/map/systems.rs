use crate::{
    components::{Destroy, DimensionsComponent, PositionComponent, StaticTag},
    data::CollisionKind,
    dispatcher::{DispatcherBuilder, RelativeStage, Stage},
    legion::prelude::*,
    map::{
        spatial::{SpatialMap, SpatialMapEntry, StaticSpatialMap},
        Map,
    },
    math::Vec3i,
    smallvec::SmallVec,
};

pub fn bundle(
    _: &mut World,
    _: &mut Resources,
    builder: &mut DispatcherBuilder,
) -> Result<(), anyhow::Error> {
    builder.add_system(Stage::Begin, build_sync_entity_rtree_system);
    builder.add_system(
        RelativeStage(Stage::Begin, -10000),
        build_maintain_maps_system,
    );

    Ok(())
}

pub fn build_maintain_maps_system(_: &mut World, _: &mut Resources) -> Box<dyn Schedulable> {
    SystemBuilder::<()>::new("maintain_maps_system")
        .write_resource::<Map>()
        .build(move |_, _, map, _| {
            game_metrics::scope!("maintain_maps_system");

            map.clear_dirty();
            //map.maintain();
        })
}

#[allow(clippy::too_many_lines)] // TODO:
pub fn build_sync_entity_rtree_system(
    world: &mut World,
    resources: &mut Resources,
) -> Box<dyn Schedulable> {
    use rstar::primitives::Rectangle;

    {
        let query = <(Read<PositionComponent>, TryRead<DimensionsComponent>)>::query()
            .filter(!tag::<StaticTag>());

        let mut add = std::collections::LinkedList::default();
        let spatial_map = SpatialMap(rstar::RTree::bulk_load(
            query
                .iter_entities(world)
                .map(|(entity, (position, dimensions))| {
                    let entry = if let Some(dimensions) = dimensions {
                        SpatialMapEntry::with_rect(
                            entity,
                            Rectangle::from_corners(
                                *position,
                                (**position
                                    + ((dimensions.as_tiles() - Vec3i::new(1, 1, 1))
                                        * Vec3i::new(1, 1, -1)))
                                .into(),
                            ),
                            dimensions.collision(),
                        )
                    } else {
                        SpatialMapEntry::with_rect(
                            entity,
                            Rectangle::from_corners(*position, *position),
                            dimensions.map_or(CollisionKind::None, |d| d.collision()),
                        )
                    };
                    add.push_front((entity, entry));

                    entry
                })
                .collect(),
        ));

        while let Some(entry) = add.pop_front() {
            world.add_component(entry.0, entry.1).unwrap();
        }

        resources.insert(spatial_map);
    }
    {
        let query = <(Read<PositionComponent>, TryRead<DimensionsComponent>)>::query()
            .filter(tag::<StaticTag>());

        let mut add = std::collections::LinkedList::default();
        let static_spatial_map = StaticSpatialMap(rstar::RTree::bulk_load(
            query
                .iter_entities(world)
                .map(|(entity, (position, dimensions))| {
                    let rect = if let Some(dimensions) = dimensions.as_ref() {
                        Rectangle::from_corners(
                            *position,
                            (**position
                                + ((dimensions.as_tiles() - Vec3i::new(1, 1, 1))
                                    * Vec3i::new(1, 1, -1)))
                            .into(),
                        )
                    } else {
                        Rectangle::from_corners(*position, *position)
                    };

                    let entry = SpatialMapEntry::with_rect(
                        entity,
                        rect,
                        dimensions.map_or(CollisionKind::None, |d| d.collision()),
                    );

                    add.push_front((entity, entry));

                    entry
                })
                .collect(),
        ));
        while let Some(entry) = add.pop_front() {
            world.add_component(entry.0, entry.1).unwrap();
        }

        resources.insert(static_spatial_map);
    }

    let mut remove_cache = SmallVec::<[SpatialMapEntry; 32]>::default();
    let mut static_remove_cache = SmallVec::<[SpatialMapEntry; 32]>::default();

    SystemBuilder::<()>::new("sync_entity_rtree_system")
        .with_query(
            <(
                Read<PositionComponent>,
                TryRead<DimensionsComponent>,
                TryWrite<SpatialMapEntry>,
            )>::query()
            .filter(
                (changed::<PositionComponent>() | changed::<DimensionsComponent>())
                    & !tag::<StaticTag>()
                    & !component::<Destroy>(),
            ),
        )
        .with_query(
            <(Read<PositionComponent>, TryRead<DimensionsComponent>)>::query()
                .filter(tag::<StaticTag>() & component::<Destroy>()),
        )
        .write_resource::<SpatialMap>()
        .write_resource::<StaticSpatialMap>()
        .build(
            move |command_buffer,
                  world,
                  (spatial_map, static_spatial_map),
                  (changed_query, static_destroy_query)| {
                crate::metrics::scope!("sync_entity_rtree_system");
                {
                    changed_query.iter_entities_mut(world).for_each(
                        |(entity, (position, dimensions, old_entry))| {
                            let dimensions = if let Some(dimensions) = dimensions {
                                *dimensions
                            } else {
                                DimensionsComponent::default()
                            };

                            let new_entry = SpatialMapEntry::with_rect(
                                entity,
                                Rectangle::from_corners(
                                    (**position).into(),
                                    (**position
                                        + ((dimensions.as_tiles() - Vec3i::new(1, 1, 1))
                                            * Vec3i::new(1, 1, -1)))
                                    .into(),
                                ),
                                dimensions.collision(),
                            );

                            if let Some(mut old_entry) = old_entry {
                                if *old_entry != new_entry {
                                    spatial_map.remove(&*old_entry);
                                    *old_entry = new_entry;
                                    spatial_map.insert(new_entry);
                                }
                            } else {
                                command_buffer.add_component(entity, new_entry);
                                spatial_map.insert(new_entry);
                            }
                        },
                    );

                    remove_cache.extend(spatial_map.iter().filter_map(|entry| {
                        if world.is_alive(entry.entity) {
                            None
                        } else {
                            Some(*entry)
                        }
                    }));
                }

                {
                    // Never update the static spatial map because they are.....static LOL
                    /*    static_changed_query.iter_entities_mut(world).for_each(
                        |(entity, (position, dimensions, old_entry))| {
                            let dimensions = if let Some(dimensions) = dimensions {
                                *dimensions
                            } else {
                                DimensionsComponent::default()
                            };

                            let new_entry = SpatialMapEntry {
                                entity,
                                rect: Rectangle::from_corners(
                                    (**position).into(),
                                    (**position
                                        + ((dimensions.as_tiles() - Vec3i::new(1, 1, 1))
                                            * Vec3i::new(1, 1, -1)))
                                    .into(),
                                ),
                            };

                            if let Some(mut old_entry) = old_entry {
                                static_spatial_map.remove(&*old_entry);
                                *old_entry = new_entry;
                            } else {
                                command_buffer.add_component(entity, new_entry);
                            }

                            static_spatial_map.insert(new_entry);
                        },
                    );
                    */
                    static_destroy_query.iter_entities(world).for_each(
                        |(entity, (position, dimensions))| {
                            let dimensions = if let Some(dimensions) = dimensions {
                                *dimensions
                            } else {
                                DimensionsComponent::default()
                            };

                            remove_cache.push(SpatialMapEntry::with_rect(
                                entity,
                                Rectangle::from_corners(
                                    (**position).into(),
                                    (**position
                                        + ((dimensions.as_tiles() - Vec3i::new(1, 1, 1))
                                            * Vec3i::new(1, 1, -1)))
                                    .into(),
                                ),
                                dimensions.collision(),
                            ));
                        },
                    );
                }
                remove_cache.drain(..).for_each(|entry| {
                    spatial_map.remove(&entry);
                });

                static_remove_cache.drain(..).for_each(|entry| {
                    static_spatial_map.remove(&entry);
                });
            },
        )
}
