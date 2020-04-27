use crate::factory::Factory;
use crate::{alloc, context::Context, factory::AllocationError, Destroyable};
use ash::vk;
use derivative::Derivative;
use smallvec::SmallVec;
use std::{marker::PhantomData, ops::Range};

pub use alloc::{AllocationCreateFlags, MemoryUsage};
pub use vk::{BufferUsageFlags as BufferUsage, MemoryPropertyFlags as MemoryProperties};

#[derive(Debug)]
pub struct BufferSet {
    pub buffers: SmallVec<[Buffer; 3]>,
    pub grow_next_frame: SmallVec<[usize; 3]>,
}
impl BufferSet {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        context: &Context,
        size: usize,
        buffer_usage: BufferUsage,
        memory_usage: MemoryUsage,
        memory_properties: Option<MemoryProperties>,
        allocation_flags: Option<AllocationCreateFlags>,
        count: usize,
    ) -> Result<Self, AllocationError> {
        let mut buffers = SmallVec::<[Buffer; 3]>::default();
        let mut grow_next_frame = SmallVec::<[usize; 3]>::default();
        for _ in 0..count {
            buffers.push(Buffer::new(
                context,
                size,
                buffer_usage,
                memory_usage,
                memory_properties,
                allocation_flags,
            )?);
            grow_next_frame.push(0);
        }

        Ok(Self {
            buffers,
            grow_next_frame,
        })
    }

    pub fn grow(&mut self, new_size: usize) -> Result<(), anyhow::Error> {
        for grow_next_frame in &mut self.grow_next_frame {
            *grow_next_frame = new_size;
        }

        Ok(())
    }

    pub fn get(&self, frame: usize) -> &Buffer {
        &self.buffers[frame]
    }

    pub fn get_mut(&mut self, frame: usize) -> &mut Buffer {
        &mut self.buffers[frame]
    }

    pub fn maintain(&mut self) -> Result<(), anyhow::Error> {
        let grow = self.grow_next_frame.clone();
        for (n, grow_next_frame) in grow.iter().enumerate() {
            if *grow_next_frame > 0 {
                self.get_mut(n).grow(*grow_next_frame)?;
            }
        }

        self.grow_next_frame.iter_mut().for_each(|v| *v = 0);

        Ok(())
    }
}

#[derive(Debug)]
pub struct BufferInfo {
    pub buffer: vk::BufferCreateInfo,
    pub allocation: alloc::AllocationInfo,
    pub allocation_create: alloc::AllocationCreateInfo,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Buffer {
    pub buffer: vk::Buffer,
    pub allocation: alloc::Allocation,
    pub info: BufferInfo,

    #[derivative(Debug = "ignore")]
    pub allocator: alloc::AllocatorPtr,
}
impl Destroyable for Buffer {
    unsafe fn destroy(&mut self, factory: &Context) -> Result<(), crate::Error> {
        self.allocator
            .destroy_buffer(self.buffer, &self.allocation)
            .unwrap();
        self.buffer = vk::Buffer::null();

        Ok(())
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if self.buffer != vk::Buffer::null() {
            log::error!(target: "render::alloc", "Dropping a buffer without calling destroy, will leak");
        }
    }
}

impl Buffer {
    pub fn new(
        context: &Context,
        size: usize,
        buffer_usage: BufferUsage,
        memory_usage: MemoryUsage,
        memory_properties: Option<MemoryProperties>,
        allocation_flags: Option<AllocationCreateFlags>,
    ) -> Result<Self, AllocationError> {
        let buffer_create_info = vk::BufferCreateInfo::builder()
            .size(size as u64)
            .usage(buffer_usage)
            .build();

        let allocation_create = alloc::AllocationCreateInfo {
            usage: memory_usage,
            required_flags: memory_properties
                .unwrap_or(MemoryProperties::HOST_VISIBLE | MemoryProperties::HOST_COHERENT),
            flags: allocation_flags.unwrap_or(AllocationCreateFlags::empty()),
            ..Default::default()
        };

        let (buffer, allocation, allocation_info) = context
            .allocator
            .create_buffer(&buffer_create_info, &allocation_create)?;

        Ok(Self {
            buffer,
            allocation,
            allocator: context.allocator.clone(),
            info: BufferInfo {
                buffer: buffer_create_info,
                allocation: allocation_info,
                allocation_create,
            },
        })
    }

    fn info(&self) -> &BufferInfo {
        &self.info
    }

    unsafe fn raw(&self) -> vk::Buffer {
        self.buffer
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn len(&self) -> usize {
        self.info.buffer.size as usize
    }
    pub fn is_empty(&self) -> bool {
        self.info.buffer.size == 0
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn write<T>(&mut self, offset: usize, slice: &[T]) -> Result<(), anyhow::Error> {
        assert!(self.info.allocation.get_size() - offset >= slice.len() * std::mem::size_of::<T>());

        unsafe {
            let mapped = self.allocator.map_memory(&self.allocation)? as *mut T;
            std::ptr::copy_nonoverlapping(slice.as_ptr(), mapped, slice.len());

            self.allocator.unmap_memory(&self.allocation)?;
        }
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn flush(&mut self, range: Option<Range<usize>>) -> Result<(), anyhow::Error> {
        if let Some(range) = range {
            self.allocator
                .flush_allocation(&self.allocation, range.start, range.end - range.start)
                .map_err(|e| anyhow::anyhow!("Failed: {:?}", e))
        } else {
            self.allocator
                .flush_allocation(&self.allocation, 0, self.info.buffer.size as usize)
                .map_err(|e| anyhow::anyhow!("Failed: {:?}", e))
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn map<F, T>(&self, f: F) -> Result<(), anyhow::Error>
    where
        T: Sized,
        F: Fn(&[T]),
    {
        let ptr = self.allocator.map_memory(&self.allocation)?;
        (f)(unsafe {
            std::slice::from_raw_parts(
                ptr as *const T,
                self.info.buffer.size as usize / std::mem::size_of::<T>(),
            )
        });
        self.allocator.unmap_memory(&self.allocation)?;

        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn map_mut<F, T>(&mut self, mut f: F) -> Result<(), anyhow::Error>
    where
        T: Sized,
        F: FnMut(&mut [T]),
    {
        let ptr = self.allocator.map_memory(&self.allocation)?;
        (f)(unsafe {
            std::slice::from_raw_parts_mut(
                ptr as *mut T,
                self.info.buffer.size as usize / std::mem::size_of::<T>(),
            )
        });
        self.allocator.unmap_memory(&self.allocation)?;

        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn grow(&mut self, new_size: usize) -> Result<(), anyhow::Error> {
        if new_size as u64 <= self.info.buffer.size {
            return Ok(());
        }

        let mut new_buffer_info = self.info.buffer;
        new_buffer_info.size = new_size as u64;

        let (new_buffer, new_allocation, new_allocation_info) = self
            .allocator
            .create_buffer(&new_buffer_info, &self.info.allocation_create)?;

        let old_ptr = self.allocator.map_memory(&self.allocation)?;
        let new_ptr = self.allocator.map_memory(&new_allocation)?;

        // Copy from old to new
        unsafe {
            std::ptr::copy_nonoverlapping(old_ptr, new_ptr, self.info.allocation.get_size());
        }

        self.allocator.unmap_memory(&self.allocation)?;
        self.allocator.unmap_memory(&new_allocation)?;

        self.allocator
            .destroy_buffer(self.buffer, &self.allocation)?;

        self.allocation = new_allocation;
        self.info.allocation = new_allocation_info;
        self.info.buffer = new_buffer_info;
        self.buffer = new_buffer;

        Ok(())
    }
}
