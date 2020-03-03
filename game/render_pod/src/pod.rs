use std140::*;

#[repr(C)]
pub struct SparseSpriteVert {
    pub position: [f32; 3],
    pub u_offset: [f32; 2],
    pub v_offset: [f32; 2],
    pub color: u32,
    pub dir_x: [f32; 2],
    pub dir_y: [f32; 2],
}
impl From<crate::sprite::SparseSprite> for SparseSpriteVert {
    fn from(rhv: crate::sprite::SparseSprite) -> Self {
        Self {
            position: rhv.position.into(),
            u_offset: rhv.u_offset.into(),
            v_offset: rhv.v_offset.into(),
            color: rhv.color.pack(),
            dir_x: rhv.dir_x.into(),
            dir_y: rhv.dir_y.into(),
        }
    }
}

#[repr(C)]
pub struct SpriteVert {
    pub pos: [f32; 3],
    pub sprite_number: u32,
    pub color: u32,
}

#[repr(C)]
pub struct MapVert {
    pub index: u32,
    pub sprite_number: u32,
    pub color: u32,
}

#[std140::repr_std140]
#[derive(Copy, Clone)]
pub struct SpriteProperties {
    pub sheet_dimensions: uvec2,
    pub map_dimensions: uvec3,
    pub sprite_dimensions: uvec2,
    pub view_proj: mat4x4,
    pub camera_translation: vec3,
}

#[std140::repr_std140]
#[derive(Copy, Clone)]
pub struct Properties {
    pub view_proj: mat4x4,
}
