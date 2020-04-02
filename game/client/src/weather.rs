#![allow(unused_variables, clippy::cast_sign_loss)]

use rl_core::{
    crossbeam::queue::SegQueue,
    defs::{
        material::{MaterialDefinition, MaterialState},
        DefinitionStorage,
    },
    dispatcher::{DispatcherBuilder, RelativeStage, Stage},
    legion::prelude::*,
    map::Map,
    math::{Vec2i, Vec3i},
    rand,
    rand::Rng,
    rayon::prelude::*,
    time::Time,
};

pub struct Weather {
    pub rain_frequency: f64, // max rain per game second
}
impl Default for Weather {
    fn default() -> Self {
        Self {
            rain_frequency: 0.0,
        }
    }
}

pub fn bundle(
    _: &mut World,
    _resources: &mut Resources,
    builder: &mut DispatcherBuilder,
) -> Result<(), anyhow::Error> {
    builder.add_system(RelativeStage(Stage::Logic, 100), build_rain_system);
    builder.add_system(RelativeStage(Stage::Logic, 101), build_liquid_evap_system);
    builder.add_system(RelativeStage(Stage::Logic, 101), build_liquid_soil_system);
    builder.add_system(
        RelativeStage(Stage::Logic, 101),
        build_liquid_dynamics_system,
    );

    Ok(())
}

const MAX_LIQUID_DEPTH: u8 = 50;
const MIN_DEPTH_DISPERSION: u8 = 20;
const SOIL_DRAIN_PER_ITER: u8 = 10;

#[allow(clippy::too_many_lines)]
pub fn build_liquid_dynamics_system(
    _: &mut World,
    resources: &mut Resources,
) -> Box<dyn Schedulable> {
    use rand::seq::SliceRandom;
    let liquid_additions = SegQueue::default();
    let liquid_removals = SegQueue::default();

    SystemBuilder::<()>::new("liquid_dynamics_system")
        .read_resource::<Time>()
        .write_resource::<Map>()
        .build(move |_, world, (time, map), all_items_query| {
            game_metrics::scope!("liquid_dynamics_system");

            {
                game_metrics::scope!("liquid_dynamics_system::iter_liquids");
                map.par_iter_indices_mut(map.has_liquid.read().par_iter(), |coord, tile| {
                    if let Some(liquid) = tile.liquid {
                        let mut rng = rand::thread_rng();

                        // Are we empty, then drop to the tile below
                        if tile.is_empty()
                            && map.get(coord + Vec3i::new(0, 0, 1)).liquid_depth()
                                < MAX_LIQUID_DEPTH
                        {
                            let diff = MAX_LIQUID_DEPTH
                                - map.get(coord + Vec3i::new(0, 0, 1)).liquid_depth();

                            tile.remove_liquid(diff);
                            liquid_removals.push(coord);
                            liquid_additions.push((coord + Vec3i::new(0, 0, 1), diff, liquid));
                        }

                        if liquid.depth > MIN_DEPTH_DISPERSION {
                            // First, are any neighbors lower then us? Then, do they have a lower water level
                            let mut neighbors = map.neighbors(&coord);
                            neighbors.shuffle(&mut rng);
                            let empty = neighbors.iter().find(|c| {
                                let tile = map.get(**c);
                                tile.is_empty()
                                    && map
                                        .get(**c + Vec3i::new(0, 0, 1))
                                        .liquid
                                        .map_or(true, |l| l.depth < MAX_LIQUID_DEPTH)
                            });
                            return if let Some(empty) = empty {
                                let diff = MIN_DEPTH_DISPERSION.min(liquid.depth);
                                liquid_additions.push((*empty, diff, liquid));
                                tile.remove_liquid(diff);
                                liquid_removals.push(coord);
                                true
                            } else {
                                // Just use the first value, since we already shuffled
                                let mut did_move = false;
                                for target in &neighbors {
                                    let target_tile = map.get(*target);
                                    if target_tile.is_solid() {
                                        continue;
                                    }

                                    if let Some(liquid) = target_tile.liquid {
                                        if liquid.depth < MAX_LIQUID_DEPTH - MIN_DEPTH_DISPERSION {
                                            liquid_additions.push((
                                                *target,
                                                MIN_DEPTH_DISPERSION.min(liquid.depth),
                                                liquid,
                                            ));
                                            tile.remove_liquid(
                                                MIN_DEPTH_DISPERSION.min(liquid.depth),
                                            );
                                            liquid_removals.push(coord);
                                            did_move = true;
                                            break;
                                        }
                                    } else {
                                        let diff = MIN_DEPTH_DISPERSION.min(liquid.depth);
                                        liquid_additions.push((*target, diff, liquid));
                                        tile.remove_liquid(diff);
                                        liquid_removals.push(coord);
                                        did_move = true;
                                    }
                                }
                                if !did_move
                                    && liquid.depth > MAX_LIQUID_DEPTH
                                    && !map.get(coord - Vec3i::new(0, 0, 1)).is_solid()
                                {
                                    let diff = liquid.depth - MAX_LIQUID_DEPTH;
                                    // If we cant move to a neighbor, it means we must go UP
                                    liquid_additions.push((
                                        coord - Vec3i::new(0, 0, 1),
                                        diff,
                                        liquid,
                                    ));
                                    tile.remove_liquid(diff);
                                    liquid_removals.push(coord);
                                    true
                                } else {
                                    did_move
                                }
                            };
                        }
                    }
                    false
                });
            }

            {
                game_metrics::scope!("liquid_dynamics_system::apply_changes");
                while let Ok(coord) = liquid_removals.pop() {
                    map.update_liquid(coord);
                }

                while let Ok((coord, depth, src)) = liquid_additions.pop() {
                    map.get_mut(coord)
                        .add_liquid(src.created, src.material, depth);
                    map.add_liquid(coord);
                }
            }
        })
}

// TODO: We should calculate this off temperature
pub fn build_liquid_evap_system(_: &mut World, resources: &mut Resources) -> Box<dyn Schedulable> {
    let mut acc = 0.0;

    SystemBuilder::<()>::new("liquid_evap_system")
        .read_resource::<Time>()
        .write_resource::<Map>()
        .build(move |_, world, (time, map), all_items_query| {
            game_metrics::scope!("liquid_evap_system");

            acc += time.world_delta.as_secs_f64();

            // TODO: Lets just evaporate 1 from any SURFACE liquid tiles every game 100 seconds
        })
}

pub fn build_liquid_soil_system(_: &mut World, resources: &mut Resources) -> Box<dyn Schedulable> {
    SystemBuilder::<()>::new("liquid_soil_system")
        .read_resource::<Time>()
        .read_resource::<DefinitionStorage<MaterialDefinition>>()
        .write_resource::<Map>()
        .build(move |_, world, (time, materials, map), all_items_query| {
            game_metrics::scope!("liquid_soil_system");

            let coord_count = map.height_map.len();
            let encoder = map.encoder();
            map.par_iter_indices_mut(
                (0..coord_count).into_par_iter().map(|index| {
                    let xy = encoder.decode(index);
                    encoder.encode(Vec3i::new(
                        xy.x,
                        xy.y,
                        map.height_at(Vec2i::new(xy.x, xy.y)),
                    ))
                }),
                |coord, tile| {
                    let mut rng = rand::thread_rng();

                    let remove = if let Some(liquid) = tile.liquid.as_mut() {
                        liquid.soil_acc += time.world_delta.as_secs_f64();
                        let material = materials.get(tile.material.into()).unwrap();
                        let drain_acc =
                            1010.0 - material.states[&MaterialState::Solid].permeability as f64;
                        if liquid.soil_acc > drain_acc {
                            // randomize acc to offset by frame a little bit.
                            liquid.soil_acc = rng.gen_range(-0.32, 0.0); // TODO: LOL
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if remove {
                        tile.remove_liquid(SOIL_DRAIN_PER_ITER);
                        map.update_liquid(coord);
                        true
                    } else {
                        false
                    }
                },
            );
        })
}

pub fn build_rain_system(_: &mut World, resources: &mut Resources) -> Box<dyn Schedulable> {
    resources.insert(Weather::default());

    let water_id = resources
        .get::<DefinitionStorage<MaterialDefinition>>()
        .unwrap()
        .get_id("water")
        .unwrap();

    SystemBuilder::<()>::new("rain_fall_system")
        .read_resource::<Time>()
        .read_resource::<Weather>()
        .read_resource::<DefinitionStorage<MaterialDefinition>>()
        .write_resource::<Map>()
        .build(
            move |_, world, (time, weather, materials, map), all_items_query| {
                game_metrics::scope!("rain_fall_system");
                if weather.rain_frequency < 0.01 {
                    return;
                }

                // Drop up to 1/10th of the map worth of the rain every frequency
                let count =
                    (time.world_delta.as_secs_f64() * weather.rain_frequency).floor() as usize;

                let dimensions = map.dimensions();

                map.par_iter_indices_mut(
                    (0..count).into_par_iter().map(|_| {
                        let mut rng = rand::thread_rng();
                        let x = rng.gen_range(0, dimensions.x - 1);
                        let y = rng.gen_range(0, dimensions.y - 1);
                        let z = map.height_at(Vec2i::new(x, y));
                        map.encoder().encode(Vec3i::new(x, y, z))
                    }),
                    |coord, tile| {
                        tile.add_liquid(time.world_time, water_id, 10);
                        map.add_liquid(coord);
                        true
                    },
                );
            },
        )
}

/*
pub fn spawn_liquid(mut self, coord: Vec3i, material: MaterialDefinitionId, depth: u8) -> Self {
        let tile = self.map.get_mut(coord);
        if let Some(liquid) = tile.liquid.as_mut() {
            assert_eq!(liquid.material, material);
            liquid.depth = liquid.depth.checked_add(depth).unwrap_or(liquid.depth);
        // TODO: silently stay full
        } else {
            tile.liquid = Some(TileLiquid { depth, material })
        }

        self.wrote_coords.push(coord);

        self
    }
*/
