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

/*
let src = map.world_to_tile((*translation).into());

                    let distance = (map.world_to_tile(**translation) - movecomp.destination)
                        .mag()
                        .abs();
M
                    if distance <= movecomp.distance as i32 {
                        command_buffer.remove_component::<MovementComponent>(entity);
                        let result = MovementResult {
                            entity,
                            request: *movecomp,
                            result: Ok(()),
                        };


                        result_channel.write(result).unwrap();
                        command_buffer.add_component(entity, result);

                        continue;
                    }

                    if distance == 1 {
                        let dst = map.tile_to_world(movecomp.destination);
                        let direction = (dst - **translation).normalized();

                        if (dst - **translation).mag() < direction.mag() {
                            **translation = dst;
                        } else {
                            **translation += direction;
                        }
                    } else {
                        if let Some(path) =
                            crate::pathfinding::astar_simple(src, movecomp.destination, &map)
                        {
                            // Draw the line for the current path
                            let mut last_step = path[0];
                            for step in &path {
                                let line_src = map.tile_to_world(last_step);
                                let line_dst = map.tile_to_world(*step);
                                debug_lines.add_line(
                                    Vec3::new(
                                        line_src.x - (map.sprite_dimensions.x as f32 / 2.0),
                                        line_src.y - (map.sprite_dimensions.y as f32 / 2.0),
                                        1.0,
                                    ),
                                    Vec3::new(
                                        line_dst.x + (map.sprite_dimensions.x as f32 / 2.0),
                                        line_dst.y + (map.sprite_dimensions.y as f32 / 2.0),
                                        1.0,
                                    ),
                                    Color::red(),
                                );
                                last_step = *step;
                            }

                            if path[0].z != src.z {
                                // Its a z transition, we ONLY move the z axis for a z-transition
                                translation.z = path[0].z as f32;
                            } else {
                                let direction =
                                    (map.tile_to_world(path[0]) - **translation).normalized();

                                // todo: movement speeds

                                **translation +=
                                    (direction * time.world_delta.as_secs_f32()) * 100.0;
                            }
                        } else {

                            command_buffer.remove_component::<MovementComponent>(entity);
                            let result = MovementResult {
                                entity,
                                request: *movecomp,
                                result: Err(MovementError::NoPath),
                            };
                            command_buffer.add_component(entity, result);
                            result_channel.write(result).unwrap();
                        }
                    }
*/
