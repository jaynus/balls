use bitflags::bitflags;
use glow::{Context, HasContext};
use num_derive::ToPrimitive;
use num_traits::ToPrimitive;
use std::ops::Range;

#[derive(Debug, PartialEq, Eq, ToPrimitive)]
#[repr(u32)]
pub enum BufferUsage {
    Array = glow::ARRAY_BUFFER,
    AtomicCounterB = glow::ATOMIC_COUNTER_BUFFER,
    CopyRead = glow::COPY_READ_BUFFER,
    CopyWRite = glow::COPY_WRITE_BUFFER,
    DrawIndirect = glow::DRAW_INDIRECT_BUFFER,
    DispatchIndirect = glow::DISPATCH_INDIRECT_BUFFER,
    ElementArray = glow::ELEMENT_ARRAY_BUFFER,
    PixelPack = glow::PIXEL_PACK_BUFFER,
    PixelUnpack = glow::PIXEL_UNPACK_BUFFER,
    Query = glow::QUERY_BUFFER,
    ShaderStorage = glow::SHADER_STORAGE_BUFFER,
    Texture = glow::TEXTURE_BUFFER,
    TransformFeedback = glow::TRANSFORM_FEEDBACK_BUFFER,
    Uniform = glow::UNIFORM_BUFFER,
}

bitflags! {

    pub struct BufferAccess: u32 {
        const NONE = 0;
        const MAP_READ_BIT = glow::MAP_READ_BIT;
        const MAP_WRITE_BIT = glow::MAP_WRITE_BIT;
        const MAP_PERSISTENT_BIT = glow::MAP_PERSISTENT_BIT;
        const MAP_COHERENT_BIT =  glow::MAP_COHERENT_BIT;
        const MAP_INVALIDATE_RANGE_BIT = glow::MAP_INVALIDATE_RANGE_BIT;
        const MAP_INVALIDATE_BUFFER_BIT = glow::MAP_INVALIDATE_BUFFER_BIT;
        const MAP_FLUSH_EXPLICIT_BIT = glow::MAP_FLUSH_EXPLICIT_BIT;
        const MAP_UNSYNCHRONIZED_BIT = glow::MAP_UNSYNCHRONIZED_BIT;
    }
}

pub struct CtxWrapper(*const Context);
impl CtxWrapper {
    pub fn new(ctx: &Context) -> Self {
        Self(ctx as *const Context)
    }
}

pub struct Buffer {
    ctx: CtxWrapper,
    buffer: <Context as HasContext>::Buffer,
    usage: BufferUsage,
    size: usize,
}
impl Buffer {
    pub fn new(
        gl: &glow::Context,
        usage: BufferUsage,
        data: Option<&[u8]>,
    ) -> Result<Self, anyhow::Error> {
        Ok(Self {
            ctx: CtxWrapper::new(gl),
            buffer: unsafe {
                let buffer = gl
                    .create_buffer()
                    .map_err(|e| anyhow::anyhow!("Failed to create buffer: {:?}", e))?;
                if let Some(data) = data {
                    gl.buffer_data_u8_slice(buffer, data, usage.to_u32().unwrap());
                }
                buffer
            },
            size: data.map_or(0, |v| v.len()),
            usage,
        })
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
    pub fn map_range_mut<F>(
        &mut self,
        gl: &Context,
        range: Range<usize>,
        access: BufferAccess,
        mut f: F,
    ) where
        F: FnMut(&[u8]),
    {
        let requested_size = (range.start + range.end) - range.start;
        assert!(self.size < requested_size);

        unsafe {
            let ptr = gl.map_buffer_range(
                self.usage.to_u32().unwrap(),
                range.start as i32,
                (range.start + range.end) as i32,
                access.bits(),
            );
            (f)(std::slice::from_raw_parts_mut(ptr, requested_size))
        }
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            (*self.ctx.0).delete_buffer(self.buffer);
        }
    }
}
