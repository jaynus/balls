use crate::{
    bitflags::*,
    bitflags_serial,
    defs::{
        building::BuildingComponent,
        material::{MaterialDefinition, MaterialDefinitionId},
        DefinitionStorage,
    },
    legion::prelude::*,
    map::Map,
    math::Vec3i,
};
use strum_macros::EnumString;

use rl_render_pod::{
    color::Color,
    sprite::{sprite_map, Sprite},
};

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, EnumString, serde::Serialize, serde::Deserialize,
)]
#[strum(serialize_all = "snake_case")]
#[repr(u8)]
pub enum TileKind {
    Empty = 0,

    Floor,

    RampUpNorth,
    RampUpSouth,
    RampUpEast,
    RampUpWest,

    Building,

    Solid = 0xFF,
}

bitflags_serial! {
    pub struct TileFlag: u8 {
        const HAS_Z_TRANSITION        =  0b0100_0000;
        const CLEAR_FLOOR             =  0b0100_0000;
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct TileLiquid {
    pub material: MaterialDefinitionId,
    pub depth: u8,
    pub created: f64,
    pub evap_acc: f64,
    pub soil_acc: f64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Tile {
    pub material: u16,
    pub flags: TileFlag,
    pub kind: TileKind,
    pub liquid: Option<TileLiquid>,

    pub building: Option<BuildingComponent>,
}
impl Clone for Tile {
    fn clone(&self) -> Self {
        Self {
            material: self.material,
            flags: self.flags,
            kind: self.kind,
            liquid: None,
            building: None,
        }
    }
}

impl Default for Tile {
    fn default() -> Self {
        Self {
            material: 0,
            flags: TileFlag::empty(),
            kind: TileKind::Empty,
            liquid: None,
            building: None,
        }
    }
}

impl Tile {
    pub fn add_liquid(&mut self, created: f64, material: MaterialDefinitionId, depth: u8) {
        if let Some(liquid) = self.liquid.as_mut() {
            assert_eq!(material, liquid.material);
            liquid.depth = liquid.depth.checked_add(depth).unwrap_or(liquid.depth);
        } else {
            self.liquid = Some(TileLiquid {
                material,
                depth,
                created,
                evap_acc: 0.0,
                soil_acc: 0.0,
            });
        }
    }

    pub fn remove_liquid(&mut self, depth: u8) -> bool {
        let (dirty, empty) = if let Some(liquid) = self.liquid.as_mut() {
            liquid.depth = liquid.depth.saturating_sub(depth);
            (true, liquid.depth == 0)
        } else {
            (false, true)
        };

        if empty {
            self.liquid = None;
        }

        dirty
    }

    pub fn make_empty(&mut self) {
        self.kind = TileKind::Empty;
        self.material = 0;
    }

    pub fn make_ramp(&mut self, kind: TileKind) {
        self.kind = kind;
        self.flags.insert(TileFlag::HAS_Z_TRANSITION);
    }

    pub fn make_floor(&mut self) {
        self.kind = TileKind::Floor;
        self.flags.remove(TileFlag::CLEAR_FLOOR);
    }

    #[inline]

    pub fn movement_cost(&self) -> Option<f32> {
        if self.is_walkable() {
            Some(100.0)
        } else {
            None
        }
    }

    #[inline]

    pub fn is_solid(&self) -> bool {
        self.kind != TileKind::Empty && self.kind != TileKind::Floor
    }

    #[inline]

    pub fn is_walkable(&self) -> bool {
        self.kind != TileKind::Empty && self.kind != TileKind::Solid
    }

    #[inline]

    pub fn is_floor(&self) -> bool {
        self.kind == TileKind::Floor
    }

    #[inline]

    pub fn is_empty(&self) -> bool {
        self.kind == TileKind::Empty
    }

    #[inline]
    pub fn liquid_depth(&self) -> u8 {
        if let Some(liquid) = self.liquid.as_ref() {
            liquid.depth
        } else {
            0
        }
    }

    pub fn sprite(
        &self,
        coord: &Vec3i,
        map: &Map,
        _world: &World,
        resources: &Resources,
    ) -> Option<Sprite> {
        // Wall - Some(((219, Color::brown())));
        // Floor - Some((178, Color::green()))

        let materials = resources
            .get::<DefinitionStorage<MaterialDefinition>>()
            .unwrap();

        let material = materials.get(self.material.into()).unwrap();

        // TODO: just pull the first state for now
        let state = material.states.values().nth(0).unwrap();
        let color: Color = state.map_sprite.color.into();

        let sprite = match self.kind {
            TileKind::Empty => None,
            TileKind::Floor => Some(Sprite::new(sprite_map::FLOOR, color)),
            TileKind::RampUpNorth
            | TileKind::RampUpSouth
            | TileKind::RampUpEast
            | TileKind::RampUpWest => Some(Sprite::new(sprite_map::RAMP_UP, color)),
            TileKind::Building => Some(Sprite::new(61, color)),
            TileKind::Solid => {
                // If we are next to NOT solid, we render. Otehrwise, we dont.
                map.neighbors(coord).drain(..).find_map(|neighbor| {
                    if map.get(neighbor).kind == TileKind::Solid {
                        None
                    } else {
                        Some(Sprite::new(sprite_map::WALL, color))
                    }
                })
            }
        };

        // Overload color with water
        if self.liquid.as_ref().map_or(false, |l| l.depth > 0) {
            let material = self.liquid.as_ref().unwrap().material.fetch(&materials);

            let state = material.states.values().nth(0).unwrap();
            let color: Color = state.map_sprite.color.into();

            if let Some(mut sprite) = sprite {
                sprite.color = color;
                Some(sprite)
            } else {
                Some(Sprite::new(sprite_map::FLOOR, color))
            }
        } else {
            sprite
        }
    }
}
impl PartialEq for Tile {
    fn eq(&self, rhv: &Self) -> bool {
        self.material == rhv.material
    }
}
impl Eq for Tile {}
