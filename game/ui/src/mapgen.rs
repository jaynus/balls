use crate::{ImguiContextLock, UiWindowSet};
use imgui::{self, im_str};
use rl_core::{image, legion::prelude::*};
use rl_render_vk::{
    alloc,
    ash::{version::DeviceV1_0, vk},
    data::texture::Texture,
    RenderContext,
};
use std::sync::Arc;

pub mod settings {
    pub struct Embark {
        pub size: [f32; 2],
    }
    impl Default for Embark {
        fn default() -> Self {
            Self { size: [20.0, 20.0] }
        }
    }
}

#[derive(Default)]
pub struct WorldSettings {
    pub embark: settings::Embark,
}

#[allow(clippy::cast_precision_loss)]
pub fn build(world: &mut World, resources: &mut Resources) -> Result<(), anyhow::Error> {
    let path = std::path::Path::new("assets/rbf_interp.png");
    let (image_width, image_height) = image::image_dimensions(&path)?;

    let (imgui_lock, render_context) =
        <(Write<ImguiContextLock>, Read<RenderContext>)>::fetch_mut(resources);

    let mut imgui = imgui_lock.lock().unwrap();

    let texture = Arc::new(Texture::from_slice(
        &image::open(path)?
            .as_flat_samples_u8()
            .ok_or_else(|| anyhow::anyhow!("Failed to open image"))?
            .as_slice(),
        &render_context.vk.device,
        &render_context.allocator,
        vk::ImageCreateInfo::builder()
            .image_type(vk::ImageType::TYPE_2D)
            .format(vk::Format::R8G8B8A8_UNORM)
            .extent(vk::Extent3D {
                width: image_width,
                height: image_height,
                depth: 1,
            })
            .mip_levels(1)
            .array_layers(1)
            .samples(vk::SampleCountFlags::TYPE_1)
            .usage(vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_DST)
            .build(),
        vk::ImageViewCreateInfo {
            view_type: vk::ImageViewType::TYPE_2D,
            format: vk::Format::R8G8B8A8_UNORM,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                level_count: 1,
                layer_count: 1,
                ..Default::default()
            },
            ..Default::default()
        },
        alloc::AllocationCreateInfo {
            usage: alloc::MemoryUsage::GpuOnly,
            ..Default::default()
        },
    )?);

    let texture_id = unsafe {
        texture.upload(
            &render_context.vk.device,
            render_context.vk.setup_command_buffer,
        )?;

        let desc_set = render_context.vk.device.allocate_descriptor_sets(
            &vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(imgui.descriptor_pool)
                .set_layouts(&[imgui.descriptor_layout])
                .build(),
        )?[0];
        render_context.vk.device.update_descriptor_sets(
            &[vk::WriteDescriptorSet::builder()
                .dst_set(desc_set)
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                .image_info(&[vk::DescriptorImageInfo {
                    image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    image_view: texture.view,
                    sampler: render_context.vk.device.create_sampler(
                        &vk::SamplerCreateInfo::builder()
                            .mag_filter(vk::Filter::LINEAR)
                            .min_filter(vk::Filter::LINEAR)
                            .build(),
                        None,
                    )?,
                }])
                .build()],
            &[],
        );

        imgui.textures.insert(desc_set)
    };

    let texture = texture;
    UiWindowSet::create_with(
        world,
        resources,
        "mapgen",
        true,
        move |ui, _window_manager, _world, _resources, _buffer| {
            let texture = texture.clone();
            imgui::Window::new(im_str!("mapgen##UI"))
                .position([0.0, 0.0], imgui::Condition::FirstUseEver)
                .size([500.0, 500.0], imgui::Condition::FirstUseEver)
                .build(ui, move || {
                    let _local_texture = texture.clone();
                    imgui::Image::new(texture_id, [500.0, 500.0]).build(ui);
                });

            true
        },
    );

    Ok(())
}
