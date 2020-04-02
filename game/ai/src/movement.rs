use rl_core::{
    components::{MovementComponent, MovementError, MovementResult, PositionComponent},
    debug::DebugLines,
    event::Channel,
    legion::prelude::*,
    map::{
        spatial::{SpatialMap, StaticSpatialMap},
        Map,
    },
    time::Time,
};

pub fn build_process_movement_system(
    _: &mut World,
    resources: &mut Resources,
) -> Box<dyn Schedulable> {
    resources.insert(Channel::<MovementResult>::default());

    SystemBuilder::<()>::new("process_movement_system")
        .read_resource::<Time>()
        .write_resource::<DebugLines>()
        .write_resource::<Map>()
        .read_resource::<Channel<MovementResult>>()
        .read_resource::<SpatialMap>()
        .read_resource::<StaticSpatialMap>()
        .with_query(<(Write<PositionComponent>, Write<MovementComponent>)>::query())
        .build(
            move |command_buffer,
                  world,
                  (time, _debug_lines, map, result_channel, spatial_map, static_spatial_map),
                  query| {
                for (entity, (mut position, mut movecomp)) in query.iter_entities_mut(world) {
                    game_metrics::scope!("process_movement_system");
                    if let Some(current) = movecomp.current {
                        // TODO: for now, just allow movement of 1 tile per "world 1 seconds"
                        movecomp.acc += time.world_delta.as_secs_f64();

                        if movecomp.acc >= 1.0 {
                            movecomp.acc = 0.0;

                            // Do move
                            if let Some(path) = crate::pathfinding::astar_simple(
                                **position,
                                current.destination,
                                &map,
                                &[&***static_spatial_map, &***spatial_map],
                            ) {
                                if !path.is_empty() {
                                    **position = path[0];
                                }
                            } else {
                                movecomp.current = None;

                                let result = MovementResult {
                                    request: current,
                                    result: Err(MovementError::NoPath),
                                };
                                command_buffer.add_component(entity, result);
                                result_channel.write(result).unwrap();
                            }
                        }

                        if **position == current.destination {
                            movecomp.current = None;

                            let result = MovementResult {
                                request: current,
                                result: Ok(()),
                            };
                            command_buffer.add_component(entity, result);
                            result_channel.write(result).unwrap();
                        }
                    }
                }
            },
        )
}
