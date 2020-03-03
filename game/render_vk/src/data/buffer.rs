#![allow(
    clippy::must_use_candidate,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    clippy::use_self
)]
use crate::alloc;
use ash::vk;
use derivative::Derivative;
use rl_core::{failure, smallvec::SmallVec};
use std::ops::Range;

#[derive(Debug)]
pub struct BufferSet {
    pub buffers: SmallVec<[Buffer; 3]>,
    pub grow_next_frame: SmallVec<[usize; 3]>,
}
impl BufferSet {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        allocator: &alloc::AllocatorPtr,
        buffer_info: vk::BufferCreateInfo,
        allocation_info: alloc::AllocationCreateInfo,
        num_frames: usize,
    ) -> Result<Self, failure::Error> {
        let mut buffers = SmallVec::<[Buffer; 3]>::default();
        let mut grow_next_frame = SmallVec::<[usize; 3]>::default();
        for _ in 0..num_frames {
            buffers.push(Buffer::new(
                allocator,
                buffer_info,
                allocation_info.clone(),
            )?);
            grow_next_frame.push(0);
        }

        Ok(Self {
            buffers,
            grow_next_frame,
        })
    }

    pub fn grow(&mut self, new_size: usize) -> Result<(), failure::Error> {
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

    pub fn maintain(&mut self) -> Result<(), failure::Error> {
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

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Buffer {
    pub buffer: vk::Buffer,
    pub buffer_info: vk::BufferCreateInfo,
    pub allocation: alloc::Allocation,
    pub allocation_info: alloc::AllocationInfo,
    pub allocation_create_info: alloc::AllocationCreateInfo,

    #[derivative(Debug = "ignore")]
    allocator: alloc::AllocatorPtr,
}
impl Buffer {
    #[allow(clippy::cast_possible_truncation)]
    pub fn len(&self) -> usize {
        self.buffer_info.size as usize
    }
    pub fn is_empty(&self) -> bool {
        self.buffer_info.size == 0
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn write<T>(&mut self, offset: usize, slice: &[T]) -> Result<(), failure::Error> {
        assert!(self.allocation_info.get_size() - offset >= slice.len() * std::mem::size_of::<T>());

        unsafe {
            let mapped = self.allocator.map_memory(&self.allocation)? as *mut T;
            std::ptr::copy_nonoverlapping(slice.as_ptr(), mapped, slice.len());

            self.allocator.unmap_memory(&self.allocation)?;
        }
        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn flush(&mut self, range: Option<Range<usize>>) -> Result<(), failure::Error> {
        if let Some(range) = range {
            self.allocator
                .flush_allocation(&self.allocation, range.start, range.end - range.start)
                .map_err(failure::err_msg)
        } else {
            self.allocator
                .flush_allocation(&self.allocation, 0, self.buffer_info.size as usize)
                .map_err(failure::err_msg)
        }
    }

    pub fn new(
        allocator: &alloc::AllocatorPtr,
        buffer_info: vk::BufferCreateInfo,
        allocation_create_info: alloc::AllocationCreateInfo,
    ) -> Result<Self, failure::Error> {
        let (buffer, allocation, allocation_info) =
            allocator.create_buffer(&buffer_info, &allocation_create_info)?;

        Ok(Self {
            buffer,
            buffer_info,
            allocation,
            allocation_info,
            allocation_create_info,
            allocator: allocator.clone(),
        })
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn map_with<F, T>(&self, f: F) -> Result<(), failure::Error>
    where
        T: Sized,
        F: Fn(&[T]),
    {
        let ptr = self.allocator.map_memory(&self.allocation)?;
        (f)(unsafe {
            std::slice::from_raw_parts(
                ptr as *const T,
                self.buffer_info.size as usize / std::mem::size_of::<T>(),
            )
        });
        self.allocator.unmap_memory(&self.allocation)?;

        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn map_mut_with<F, T>(&mut self, mut f: F) -> Result<(), failure::Error>
    where
        T: Sized,
        F: FnMut(&mut [T]),
    {
        let ptr = self.allocator.map_memory(&self.allocation)?;
        (f)(unsafe {
            std::slice::from_raw_parts_mut(
                ptr as *mut T,
                self.buffer_info.size as usize / std::mem::size_of::<T>(),
            )
        });
        self.allocator.unmap_memory(&self.allocation)?;

        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    pub fn grow(&mut self, new_size: usize) -> Result<(), failure::Error> {
        if new_size as u64 <= self.buffer_info.size {
            return Ok(());
        }

        let mut new_buffer_info = self.buffer_info;
        new_buffer_info.size = new_size as u64;

        let (new_buffer, new_allocation, new_allocation_info) = self
            .allocator
            .create_buffer(&new_buffer_info, &self.allocation_create_info)?;

        let old_ptr = self.allocator.map_memory(&self.allocation)?;
        let new_ptr = self.allocator.map_memory(&new_allocation)?;

        // Copy from old to new
        unsafe {
            std::ptr::copy_nonoverlapping(old_ptr, new_ptr, self.allocation_info.get_size());
        }

        self.allocator.unmap_memory(&self.allocation)?;
        self.allocator.unmap_memory(&new_allocation)?;

        self.allocator
            .destroy_buffer(self.buffer, &self.allocation)?;

        self.allocation = new_allocation;
        self.allocation_info = new_allocation_info;
        self.buffer_info = new_buffer_info;
        self.buffer = new_buffer;

        Ok(())
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        self.allocator
            .destroy_buffer(self.buffer, &self.allocation)
            .unwrap();
    }
}

unsafe impl Sync for Buffer {}
unsafe impl Send for Buffer {}
