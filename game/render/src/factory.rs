use crate::{
    context::Context,
    resources::{
        buffer::{self, Buffer},
        command::CommandBuffer,
        texture::Texture,
    },
    Destroyable,
};
use parking_lot::Mutex;
use rl_core::NamedSlotMap;
use shrinkwraprs::Shrinkwrap;

slotmap::new_key_type! { pub struct BufferHandle; }
#[derive(Shrinkwrap, Default)]
#[shrinkwrap(mutable)]
struct BufferStorage(pub NamedSlotMap<BufferHandle, ash::vk::Buffer>);

slotmap::new_key_type! { pub struct SemaphoreHandle; }
#[derive(Shrinkwrap, Default)]
#[shrinkwrap(mutable)]
struct SemaphoreStorage(pub NamedSlotMap<SemaphoreHandle, ash::vk::Semaphore>);

slotmap::new_key_type! { pub struct FenceHandle; }
#[derive(Shrinkwrap, Default)]
#[shrinkwrap(mutable)]
struct FenceStorage(pub NamedSlotMap<FenceHandle, ash::vk::Fence>);

#[derive(Debug, thiserror::Error)]
pub enum AllocationError {
    #[error("Unknown")]
    Unknown,
    #[error("Allocation failed in vk_mem: {}", 0)]
    AllocationFailed(vk_mem::error::Error),
}
impl From<vk_mem::error::Error> for AllocationError {
    fn from(other: vk_mem::error::Error) -> Self {
        Self::AllocationFailed(other)
    }
}

struct Resources {
    pub buffers: Mutex<Vec<ash::vk::Buffer>>,
}

pub struct Factory {
    res: Resources,
    command_pool: ash::vk::CommandPool,
}
impl Factory {
    pub fn make_buffer<T: Sized>(
        &self,
        context: &Context,
        size: usize,
        buffer_usage: buffer::BufferUsage,
        memory_usage: buffer::MemoryUsage,
        name: Option<&str>,
        memory_properties: Option<buffer::MemoryProperties>,
        allocation_flags: Option<buffer::AllocationCreateFlags>,
    ) -> Result<Buffer, AllocationError> {
        // calculate the real size and alignment? based on the type
        let size = size * std::mem::size_of::<T>();
        let buffer = Buffer::new(
            context,
            size,
            buffer_usage,
            memory_usage,
            memory_properties,
            allocation_flags,
        )?;

        self.res.buffers.lock().push(buffer.buffer);

        Ok(buffer)
    }
}
