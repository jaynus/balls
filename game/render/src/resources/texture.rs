#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::wildcard_imports,
    clippy::missing_safety_doc,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    clippy::use_self
)]
use crate::{
    alloc::{Allocation, AllocationCreateInfo, AllocationInfo, AllocatorPtr},
    context::Context,
    factory::AllocationError,
    resources::buffer::{Buffer, BufferUsage, MemoryProperties, MemoryUsage},
    Destroyable,
};
use ash::{version::DeviceV1_0, vk};
use derivative::Derivative;
use rl_core::slotmap;

slotmap::new_key_type! { pub struct TextureHandle; }

pub type TexturePtr = std::sync::Arc<Texture>;

#[derive(Derivative)]
#[derivative(Debug)]
pub struct Texture {
    pub raw: Vec<u8>,
    pub image_info: vk::ImageCreateInfo,
    pub image_view: vk::ImageViewCreateInfo,

    pub view: vk::ImageView,
    pub image: vk::Image,

    #[derivative(Debug = "ignore")]
    pub buffer: Buffer,
    pub allocation: Allocation,
    pub allocation_info: AllocationInfo,
}
impl Texture {
    pub unsafe fn upload(
        &self,
        device: &ash::Device,
        command_buffer: vk::CommandBuffer,
    ) -> Result<(), anyhow::Error> {
        // Upload the raw
        let texture_barrier = vk::ImageMemoryBarrier {
            dst_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            new_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            image: self.image,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                level_count: 1,
                layer_count: 1,
                ..Default::default()
            },
            ..Default::default()
        };

        device.cmd_pipeline_barrier(
            command_buffer,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[texture_barrier],
        );
        let buffer_copy_regions = vk::BufferImageCopy::builder()
            .image_subresource(
                vk::ImageSubresourceLayers::builder()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .layer_count(1)
                    .build(),
            )
            .image_extent(vk::Extent3D {
                width: self.image_info.extent.width,
                height: self.image_info.extent.height,
                depth: 1,
            });

        device.cmd_copy_buffer_to_image(
            command_buffer,
            self.buffer.buffer,
            self.image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            &[buffer_copy_regions.build()],
        );
        let texture_barrier_end = vk::ImageMemoryBarrier {
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            dst_access_mask: vk::AccessFlags::SHADER_READ,
            old_layout: vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            image: self.image,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                level_count: 1,
                layer_count: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        device.cmd_pipeline_barrier(
            command_buffer,
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            vk::DependencyFlags::empty(),
            &[],
            &[],
            &[texture_barrier_end],
        );

        Ok(())
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn from_slice(
        context: &Context,
        raw: &[u8],
        device: &ash::Device,
        alloc: &AllocatorPtr,
        image_info: vk::ImageCreateInfo,
        mut image_view: vk::ImageViewCreateInfo,
        create_alloc_info: AllocationCreateInfo,
    ) -> Result<Self, anyhow::Error> {
        // We mantain our own copy of the texture data
        let raw = raw.to_vec();

        let mut buffer = Buffer::new(
            context,
            raw.len(),
            BufferUsage::TRANSFER_SRC,
            MemoryUsage::GpuOnly,
            None,
            None,
        )?;
        //

        let (image, allocation, allocation_info) =
            alloc.create_image(&image_info, &create_alloc_info)?;

        image_view.image = image;

        let view = unsafe { device.create_image_view(&image_view, None)? };

        buffer.write(0, &raw)?;

        Ok(Self {
            raw,
            image,
            buffer,
            view,
            image_info,
            image_view,
            allocation,
            allocation_info,
        })
    }
}
impl Destroyable for Texture {
    unsafe fn destroy(&mut self, context: &Context) -> Result<(), crate::Error> {
        context.device.destroy_image_view(self.view, None);
        context
            .allocator
            .destroy_image(self.image, &self.allocation)
            .unwrap();

        Ok(())
    }
}
