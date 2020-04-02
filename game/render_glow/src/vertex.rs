use glow::HasContext;
use rl_render_pod::std140::*;
use std::any::TypeId;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct VertexTypeMapEntry {
    pub type_id: TypeId,
    pub kind: u32,
    pub width: usize,
}
impl VertexTypeMapEntry {
    pub const fn new<T: 'static + ReprStd140>(kind: u32, width: usize) -> Self {
        Self {
            type_id: TypeId::of::<T>(),
            width,
            kind,
        }
    }
}

const TYPE_MAP: [VertexTypeMapEntry; 10] = [
    VertexTypeMapEntry::new::<ivec2>(glow::INT, 2),
    VertexTypeMapEntry::new::<ivec3>(glow::INT, 3),
    VertexTypeMapEntry::new::<ivec4>(glow::INT, 4),
    VertexTypeMapEntry::new::<uvec2>(glow::UNSIGNED_INT, 2),
    VertexTypeMapEntry::new::<uvec3>(glow::UNSIGNED_INT, 3),
    VertexTypeMapEntry::new::<uvec4>(glow::UNSIGNED_INT, 4),
    VertexTypeMapEntry::new::<vec2>(glow::FLOAT, 2),
    VertexTypeMapEntry::new::<vec3>(glow::FLOAT, 3),
    VertexTypeMapEntry::new::<vec4>(glow::FLOAT, 4),
    VertexTypeMapEntry::new::<mat4x4>(glow::FLOAT, 16),
];

fn kind_from_std140<T: 'static + ReprStd140>() -> VertexTypeMapEntry {
    let src = TypeId::of::<T>();

    *TYPE_MAP
        .iter()
        .find(|entry| src == entry.type_id)
        .expect("Unimplemented type map")
}

pub struct VertexAttrib {
    pub name: &'static str,
    pub count: i32,
    pub kind: u32,
    pub size: u32,
    pub normalized: bool,
}
impl VertexAttrib {
    pub fn new(name: &'static str, count: i32, kind: u32, size: u32, normalized: bool) -> Self {
        Self {
            name,
            count,
            kind,
            size,
            normalized,
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub fn from_std140<T: 'static + ReprStd140>(name: &'static str) -> Self {
        let kind = kind_from_std140::<T>();
        Self {
            name,
            count: kind.width as i32,
            size: std::mem::size_of::<T>() as u32,
            kind: kind.kind,
            normalized: false,
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub fn from_std140_normalized<T: 'static + ReprStd140>(name: &'static str) -> Self {
        let kind = kind_from_std140::<T>();
        Self {
            name,
            count: kind.width as i32,
            size: std::mem::size_of::<T>() as u32,
            kind: kind.kind,
            normalized: false,
        }
    }
}

pub trait VertexDecl {
    fn desc() -> Vec<VertexAttrib>;
}

pub trait VertexAttribList {
    fn submit(&self, gl: &glow::Context);
}

impl<'a> VertexAttribList for &'a [VertexAttrib] {
    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    fn submit(&self, gl: &glow::Context) {
        unsafe {
            let stride = self.iter().map(|e| e.size).sum::<u32>() as i32;

            let mut offset = 0;
            for (n, attrib) in self.iter().enumerate() {
                gl.enable_vertex_attrib_array(n as u32);

                gl.vertex_attrib_pointer_f32(
                    n as u32,
                    attrib.count,
                    attrib.kind,
                    attrib.normalized,
                    stride,
                    offset,
                );

                offset += attrib.size as i32;
            }
        }
    }
}
