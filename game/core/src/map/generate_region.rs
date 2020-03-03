use crate::{
    components::*,
    defs::foliage::FoliageKind,
    legion::prelude::*,
    map::{
        tile::{Tile, TileKind},
        Map,
    },
    math::Vec3i,
    time::Time,
    transform::Translation,
};
use rl_render_pod::{
    color::Color,
    sprite::{Sprite, SpriteLayer, StaticSpriteTag},
};

use rand::Rng;
use std::path::Path;

#[allow(clippy::cast_possible_truncation)]
pub fn smooth(heightmap: &mut image::GrayImage) {
    for x in 1..heightmap.dimensions().0 - 1 {
        for y in 1..heightmap.dimensions().1 - 1 {
            let sum = (u32::from(heightmap.get_pixel(x + 1, y)[0])
                + u32::from(heightmap.get_pixel(x, y + 1)[0])
                + u32::from(heightmap.get_pixel(x + 1, y + 1)[0])
                + u32::from(heightmap.get_pixel(x - 1, y)[0])
                + u32::from(heightmap.get_pixel(x, y - 1)[0])
                + u32::from(heightmap.get_pixel(x - 1, y - 1)[0])
                + u32::from(heightmap.get_pixel(x + 1, y - 1)[0])
                + u32::from(heightmap.get_pixel(x - 1, y + 1)[0]))
                / 8;
            heightmap.get_pixel_mut(x, y)[0] = sum as u8;
        }
    }
}

#[allow(clippy::too_many_lines)]
pub fn from_heightmap<P: AsRef<Path>>(
    path: P,
    world: &mut World,
    resources: &mut Resources,
) -> Result<Map, failure::Error> {
    let mut map = Map::new(Vec3i::new(1024, 1024, 128))?;

    // build a definite heightmap out of the src heightmap
    let mut heightmap = [[0 as u8; 1024]; 1024];

    let image = image::open(path)?;
    if let image::DynamicImage::ImageLuma8(mut src_heightmap) = image {
        // First smooth it
        for _ in 0..5 {
            smooth(&mut src_heightmap);
        }

        for pixel in src_heightmap.pixels_mut() {
            if pixel[0] < 100 {
                pixel[0] = 100;
            }
        }

        // Save smoothed for debug
        src_heightmap.save_with_format("target/smoothed.png", image::ImageFormat::Png)?;

        for (x, y, pixel) in src_heightmap.enumerate_pixels() {
            let normalized = f32::from(pixel[0]) / 255.0;
            #[allow(clippy::cast_possible_truncation)]
            let z = 128 - (128.0 * normalized) as i32;

            #[allow(clippy::cast_possible_wrap)]
            map.set_untracked(
                Vec3i::new(x as i32, y as i32, z),
                Tile {
                    kind: TileKind::Floor,
                    ..Default::default()
                },
            );

            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            {
                heightmap[x as usize][y as usize] = z as u8;
            }
            for n in z + 1..128 {
                #[allow(clippy::cast_possible_wrap)]
                map.set_untracked(
                    Vec3i::new(x as i32, y as i32, n),
                    Tile {
                        kind: TileKind::Solid,
                        ..Default::default()
                    },
                )
            }
        }

        let mut ramps = map
            .storage()
            .iter()
            .enumerate()
            .filter_map(|(n, tile)| {
                if !tile.is_floor() {
                    return None;
                }

                let coord = map.encoder().decode(n);
                if coord.x < 1 || coord.x >= 1023 || coord.y < 1 || coord.y >= 1023 {
                    return None;
                }

                let mut has_adj_floor = false;
                let mut has_adj_solid = false;
                for neighbor in map.neighbors(&coord).drain(..) {
                    let tile = map.get(neighbor);
                    if tile.is_floor() {
                        has_adj_floor = true;
                    }
                    if tile.kind == TileKind::Solid {
                        has_adj_solid = true;
                    }

                    if has_adj_floor && has_adj_solid {
                        break;
                    }
                }

                if has_adj_floor && has_adj_solid {
                    Some(coord)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        ramps.drain(..).for_each(|coord| {
            let tile = map.get_mut_untracked(coord);
            tile.kind = TileKind::RampUpNorth;
        });

        let mut rng = rand::thread_rng();

        let time = resources.get::<Time>().unwrap();

        // Randomly place trees along tghe heightmap
        world.insert(
            (
                StaticTag,
                StaticSpriteTag(Sprite::new(5, Color::default())),
                SpriteLayer::Foliage,
                FoliageTag(FoliageKind::Tree),
            ),
            (0..50000).map(|_| {
                let x = rng.gen_range(0, map.dimensions().x);
                let y = rng.gen_range(0, map.dimensions().y);

                #[allow(clippy::cast_sign_loss)]
                let tile_coord = Vec3i::new(x, y, i32::from(heightmap[x as usize][y as usize]));
                let world_coord = map.tile_to_world(tile_coord);

                (
                    EntityMeta::new(time.stamp()),
                    Translation(world_coord),
                    DimensionsComponent::with_tiles(Vec3i::new(2, 2, 10)),
                    PositionComponent::new(tile_coord),
                )
            }),
        );

        Ok(map)
    } else {
        panic!("balls")
    }
}
