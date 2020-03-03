use rl_core::defs::item::{StockpileComponent, StockpileSpatialMap};
use rl_core::{data::CollisionKind, legion::prelude::*, map::spatial::SpatialMapEntry, rstar};

pub fn build_stockpile_update_children(
    _: &mut World,
    resources: &mut Resources,
) -> Box<dyn Schedulable> {
    resources.insert(StockpileSpatialMap::default());

    SystemBuilder::<()>::new("stockpile_update_children_system")
        .write_resource::<StockpileSpatialMap>()
        .with_query(<Read<StockpileComponent>>::query())
        .build(move |_, world, stockpile_map, stockpiles_query| {
            game_metrics::scope!("stockpile_update_children_system");

            let count = stockpile_map.iter().count();
            let mut new_data = Vec::with_capacity(count);

            stockpiles_query
                .iter_entities(world)
                .for_each(|(entity, stockpile)| {
                    stockpile.tiles.iter().for_each(|coord| {
                        new_data.push(SpatialMapEntry::new_single(
                            entity,
                            *coord,
                            CollisionKind::Solid,
                        ));
                    })
                });

            ***stockpile_map = rstar::RTree::bulk_load(new_data);
        })
}
