use crate::spawners::{Spawnable, Target, TargetPosition};
use rl_core::data::SpawnPawnArguments;
use rl_core::{
    camera::Camera,
    components::*,
    data::{CollisionKind, SpawnArguments},
    defs::{
        creature::CreatureDefinition,
        foliage::FoliageDefinition,
        item::ItemDefinition,
        material::{MaterialComponent, MaterialDefinition, MaterialState},
        race::RaceDefinition,
        DefinitionStorage,
    },
    legion::prelude::*,
    map::{
        tile::{Tile, TileKind},
        Map,
    },
    math::{Aabbi, Vec3, Vec3i},
    rand, rand_xorshift,
    settings::{DisplayMode, Settings},
    time::Time,
    transform::Translation,
    transform::*,
};

pub fn init_camera(
    world: &mut World,
    resources: &mut Resources,
    translation: Vec3,
    settings: &Settings,
) -> Entity {
    {
        // Add the camera here because lol
        match settings.display_mode {
            DisplayMode::Windowed(w, h) => {
                world.insert(
                    (),
                    vec![(
                        EntityMeta::new(resources.get::<Time>().unwrap().stamp()),
                        Camera::new(w as f32, h as f32),
                        Translation(translation),
                        Scale::default(),
                    )],
                )[0]
            }
            _ => unimplemented!(),
        }
    }
}

pub fn init_empty_world(
    world: &mut World,
    resources: &mut Resources,
    settings: &Settings,
) -> Result<(), anyhow::Error> {
    let map = Map::new(Vec3i::new(1, 1, 1))?;
    resources.insert(map);

    init_camera(world, resources, Vec3::new(0.0, 0.0, 0.0), settings);

    Ok(())
}

#[allow(clippy::too_many_lines)]
pub fn init_minimal_world(
    world: &mut World,
    resources: &mut Resources,
    settings: &Settings,
) -> Result<(), anyhow::Error> {
    use rand::{Rng, SeedableRng};

    let (marble_id, soil_id, _water_id, _wood_id) = {
        let materials = resources
            .get::<DefinitionStorage<MaterialDefinition>>()
            .unwrap();
        (
            materials.get_id("marble").unwrap(),
            materials.get_id("soil").unwrap(),
            materials.get_id("water").unwrap(),
            materials.get_id("wood").unwrap(),
        )
    };

    let camera_entity = init_camera(
        world,
        resources,
        Vec3::new(-80.0 * 16.0, -80.0 * 24.0, 15.0),
        settings,
    );

    let mut map = Map::with_default(Vec3i::new(200, 200, 50), || Tile {
        material: marble_id.into(),
        kind: TileKind::Solid,
        ..Default::default()
    })?;
    // Lay ground at z-15
    Aabbi::new(
        Vec3i::new(0, 0, 15),
        Vec3i::new(map.dimensions().x, map.dimensions().y, 16),
    )
    .iter()
    .for_each(|coord| {
        let tile = map.get_mut_untracked(coord);
        tile.make_floor();
        tile.material = soil_id.into();
    });

    // Lay a 2-tile wide trench for giggles
    Aabbi::new(
        Vec3i::new(20, 20, 15),
        Vec3i::new(25, map.dimensions().y, 17),
    )
    .iter()
    .for_each(|coord| {
        map.get_mut_untracked(coord).make_empty();
    });

    // Lay a 2-tile wide trench for giggles
    Aabbi::new(
        Vec3i::new(20, 20, 17),
        Vec3i::new(25, map.dimensions().y, 18),
    )
    .iter()
    .for_each(|coord| {
        let tile = map.get_mut_untracked(coord);
        tile.make_floor();
        tile.material = marble_id.into();
    });

    // Clear skies below z=15
    Aabbi::new(
        Vec3i::new(0, 0, 0),
        Vec3i::new(map.dimensions().x, map.dimensions().y, 15),
    )
    .iter()
    .for_each(|coord| map.get_mut_untracked(coord).make_empty());

    map.recompute_height_map();

    resources.insert(map);

    let mut command_buffer = CommandBuffer::new(world);

    let mut rng =
        rand_xorshift::XorShiftRng::from_seed([5, 5, 5, 5, 1, 1, 5, 5, 5, 5, 5, 5, 5, 2, 3, 4]);
    let dimensions = DimensionsComponent::with_tiles(Vec3i::new(2, 2, 10));

    {
        use rl_core::map::spatial::SpatialMapEntry;
        use rl_core::rstar;

        let mut spatial_map = rstar::RTree::<SpatialMapEntry>::new();

        let map = resources.get::<Map>().unwrap();
        let foliages = resources
            .get::<DefinitionStorage<FoliageDefinition>>()
            .unwrap();
        let grass = foliages.get_by_name("grass").unwrap();
        let tree = foliages.get_by_name("oak tree").unwrap();

        (0..3000).for_each(|_| {
            let x = rng.gen_range(0, map.dimensions().x);
            let y = rng.gen_range(0, map.dimensions().y);

            let tile_coord = Vec3i::new(x, y, 15);

            let entry = SpatialMapEntry::new(
                camera_entity,
                &PositionComponent::from(tile_coord),
                &dimensions,
            );

            if spatial_map
                .locate_in_envelope_intersecting(&entry.aabb())
                .peekable()
                .peek()
                .is_some()
            {
                return;
            }

            spatial_map.insert(entry);

            tree.spawn(
                resources,
                &mut command_buffer,
                Target::Position(TargetPosition::Tile(tile_coord)),
                &SpawnArguments::Foliage {
                    dimensions: Some(dimensions),
                },
            )
            .unwrap();
        });

        Aabbi::new(
            Vec3i::new(0, 0, 15),
            Vec3i::new(map.dimensions().x, map.dimensions().y, 16),
        )
        .iter()
        .for_each(|coord| {
            if !map.get(coord).is_empty() && rng.gen_range(0, 100) < 70 {
                let entry = SpatialMapEntry::new_single(camera_entity, coord, CollisionKind::None);

                if spatial_map
                    .locate_in_envelope_intersecting(&entry.aabb())
                    .peekable()
                    .peek()
                    .is_some()
                {
                    return;
                }

                spatial_map.insert(entry);

                grass
                    .spawn(
                        resources,
                        &mut command_buffer,
                        Target::Position(TargetPosition::Tile(coord)),
                        &SpawnArguments::Foliage { dimensions: None },
                    )
                    .unwrap();
            }
        });
    }

    command_buffer.write(world);

    init_pawns(
        world,
        resources,
        [
            Vec3i::new(12, 4, 15),
            Vec3i::new(14, 4, 15),
            Vec3i::new(16, 4, 15),
        ]
        .iter(),
    )?;

    init_cows(
        world,
        resources,
        [
            Vec3i::new(12, 6, 15),
            Vec3i::new(14, 6, 15),
            Vec3i::new(16, 6, 15),
        ]
        .iter(),
    )?;

    let item_storage = <Read<DefinitionStorage<ItemDefinition>>>::fetch(resources);

    let mut command_buffer = CommandBuffer::new(world);

    // Spawn a few test items
    let coord = Vec3i::new(5, 4, 15);
    item_storage
        .get_by_name("Pickaxe")
        .unwrap()
        .spawn(
            resources,
            &mut command_buffer,
            Target::Position(TargetPosition::Tile(coord)),
            &SpawnArguments::Item {
                material: MaterialComponent::new(marble_id, MaterialState::Solid),
            },
        )
        .unwrap();

    let coord = Vec3i::new(5, 6, 15);
    item_storage
        .get_by_name("Axe")
        .unwrap()
        .spawn(
            resources,
            &mut command_buffer,
            Target::Position(TargetPosition::Tile(coord)),
            &SpawnArguments::Item {
                material: MaterialComponent::new(marble_id, MaterialState::Solid),
            },
        )
        .unwrap();

    command_buffer.write(world);

    Ok(())
}

pub fn init_full_world(
    world: &mut World,
    resources: &mut Resources,
    settings: &Settings,
) -> Result<(), anyhow::Error> {
    let z = 78;

    init_camera(world, resources, Vec3::new(0.0, 0.0, z as f32), settings);

    let map = rl_core::map::generate_region::from_heightmap(
        "worldgen/output/rbf_interp.png",
        world,
        resources,
    )?;
    let middle = map.dimensions() / 2;
    resources.insert(map);

    init_pawns(
        world,
        resources,
        [
            Vec3i::new(middle.x, middle.y, z),
            Vec3i::new(middle.x + 2, middle.y, z),
            Vec3i::new(middle.x + 4, middle.y, z),
        ]
        .iter(),
    )?;

    Ok(())
}

pub fn init_pawns<'a, I>(
    world: &mut World,
    resources: &mut Resources,
    positions: I,
) -> Result<(), anyhow::Error>
where
    I: Iterator<Item = &'a Vec3i> + std::iter::ExactSizeIterator,
{
    use crate::spawners::Pawn;

    let mut command_buffer = CommandBuffer::new(world);

    let human_id = resources
        .get::<DefinitionStorage<RaceDefinition>>()
        .unwrap()
        .get_id("human")
        .unwrap();

    positions.enumerate().for_each(|(n, coord)| {
        Pawn::default()
            .spawn(
                resources,
                &mut command_buffer,
                Target::Position(TargetPosition::Tile(*coord)),
                &SpawnArguments::Pawn {
                    arguments: SpawnPawnArguments {
                        name: format!("Pawn_{}", n),
                        race: human_id,
                    },
                },
            )
            .unwrap();
    });

    command_buffer.write(world);

    Ok(())
}

pub fn init_cows<'a, I>(
    world: &mut World,
    resources: &mut Resources,
    positions: I,
) -> Result<(), anyhow::Error>
where
    I: Iterator<Item = &'a Vec3i> + std::iter::ExactSizeIterator,
{
    let mut command_buffer = CommandBuffer::new(world);

    let creatures = resources
        .get::<DefinitionStorage<CreatureDefinition>>()
        .unwrap();
    let cow = creatures.get_by_name("Cow").unwrap();

    positions.enumerate().for_each(|(n, coord)| {
        cow.spawn(
            resources,
            &mut command_buffer,
            Target::Position(TargetPosition::Tile(*coord)),
            &SpawnArguments::Creature {
                name: Some(format!("Cow {}", n)),
            },
        )
        .unwrap();
    });

    command_buffer.write(world);

    Ok(())
}
