//! inventory helpers
use rl_core::defs::item::ItemComponent;
use rl_core::{
    components::PositionComponent,
    dispatcher::{DispatcherBuilder, Stage},
    failure,
    legion::prelude::*,
    map::Map,
};

pub fn bundle(
    _: &mut World,
    _resources: &mut Resources,
    builder: &mut DispatcherBuilder,
) -> Result<(), failure::Error> {
    builder.add_system(Stage::Logic, build_items_fall_system);
    Ok(())
}

pub fn build_items_fall_system(_: &mut World, _: &mut Resources) -> Box<dyn Schedulable> {
    let mut updates = Vec::with_capacity(16);

    SystemBuilder::<()>::new("item_fall_system")
        .read_resource::<Map>()
        .write_component::<PositionComponent>()
        .with_query(<Read<PositionComponent>>::query().filter(component::<ItemComponent>()))
        .build(move |_, world, map, all_items_query| {
            game_metrics::scope!("item_fall_system");

            for (entity, position) in all_items_query.iter_entities(world) {
                let mut current_position = **position;

                if map.get(current_position).is_empty() {
                    loop {
                        if !map.get(current_position).is_empty() {
                            updates.push((entity, current_position));
                            break;
                        }
                        current_position.z += 1;
                    }
                }
            }

            updates.drain(..).for_each(|(entity, position)| {
                **world
                    .get_component_mut::<PositionComponent>(entity)
                    .unwrap() = position;
            });
        })
}
