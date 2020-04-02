/*
use crate::{
    alloc,
    alloc::AllocatorPtr,
    ash::{self, vk},
    data::{
        buffer::{Buffer, BufferSet},
        texture::{Texture, TextureHandle, TexturePtr},
        VulkanContext,
    },
    pass::{RenderArgs, VkPass},
    shader::{Shader, ShaderKind},
    RenderContext,
};
use ash::version::DeviceV1_0;
use rl_ai::HasTasksComponent;
use rl_core::{
    camera::{make_camera_query, CameraQueryFn, CameraQueryResult},
    components::{Destroy, DimensionsComponent, FoliageTag, PositionComponent, VirtualTaskTag},

    fxhash::{FxBuildHasher, FxHashMap},
    legion::{borrow::Ref, prelude::*},
    map::Map,
    math::{Aabbi, Vec3i},
    settings::Settings,
    smallvec::SmallVec,
    strum::IntoEnumIterator,
    transform::Translation,
    GameState, NamedSlotMap,
};
use rl_render_pod::{
    color::Color,
    pod::{SpriteProperties, SpriteVert},
    sprite::{sprite_map, Sprite, SpriteLayer, StaticSpriteTag},
    std140,
};
use std::sync::Arc;

pub struct SpinePass {
    vertex: Shader,
    fragment: Shader,

    desc_sets: Vec<vk::DescriptorSet>,
    desc_layout: vk::DescriptorSetLayout,

    props_buffer: Buffer,

    dynamic_entity_buffer: BufferSet,

    sprite_sheet: TexturePtr,

    pipeline_layout: vk::PipelineLayout,

    camera_query: CameraQueryFn,
}

impl SpinePass {
    const FRAME_COUNT: usize = 2;

    #[allow(clippy::too_many_lines)] // TODO:
    pub fn new(
        state: &mut GameState,
        vk: &VulkanContext,
        allocator: &AllocatorPtr,
    ) -> Result<Box<dyn VkPass>, anyhow::Error> {
        unsafe {
            let vertex = Shader::from_src_path(
                &vk.device,
                ShaderKind::Vertex,
                "assets/shaders/entities.vert",
            )?;
            let fragment = Shader::from_src_path(
                &vk.device,
                ShaderKind::Fragment,
                "assets/shaders/entities.frag",
            )?;

            let (sprite_sheet, dynamic_entity_buffer, static_entity_buffer) =
                Self::allocate_storages(vk, allocator)?;

            state
                .resources
                .get_mut::<NamedSlotMap<TextureHandle, TexturePtr>>()
                .unwrap()
                .insert("sprite_sheet", sprite_sheet.clone());

            sprite_sheet.upload(&vk.device, vk.setup_command_buffer)?;

            let mut props_buffer = Buffer::new(
                &allocator,
                vk::BufferCreateInfo {
                    size: std::mem::size_of::<SpriteProperties>() as u64,
                    usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
                    ..Default::default()
                },
                alloc::AllocationCreateInfo {
                    usage: alloc::MemoryUsage::CpuToGpu,

                    ..Default::default()
                },
            )?;

            // Fill vertex buffers

            let mut camera_query = make_camera_query();
            let camera = camera_query(&state.world);

            {
                props_buffer.write(
                    0,
                    &[SpriteProperties {
                        sheet_dimensions: std140::uvec2(
                            sprite_sheet.image_info.extent.width,
                            sprite_sheet.image_info.extent.height,
                        ),
                        map_dimensions: std140::uvec3(0, 0, 0),
                        sprite_dimensions: std140::uvec2(16, 24),
                        view_proj: camera
                            .camera
                            .matrix(&camera.translation, camera.scale)
                            .as_slice()
                            .into(),
                        camera_translation: camera.translation.as_slice().into(),
                    }],
                )?;
            }

            let descriptor_pool = vk.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::builder()
                    .pool_sizes(&[
                        vk::DescriptorPoolSize {
                            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                            descriptor_count: 2,
                        },
                        vk::DescriptorPoolSize {
                            ty: vk::DescriptorType::UNIFORM_BUFFER,
                            descriptor_count: 2,
                        },
                    ])
                    .max_sets(2)
                    .build(),
                None,
            )?;

            let desc_layout = vk.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::builder().bindings(&[
                    vk::DescriptorSetLayoutBinding::builder()
                        .binding(0)
                        .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::VERTEX)
                        .build(),
                    vk::DescriptorSetLayoutBinding::builder()
                        .binding(1)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                        .build(),
                ]),
                None,
            )?;

            let desc_sets = vk.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&[desc_layout, desc_layout])
                    .build(),
            )?;

            let sampler = vk.device.create_sampler(
                &vk::SamplerCreateInfo::builder()
                    .min_filter(vk::Filter::LINEAR)
                    .mag_filter(vk::Filter::NEAREST)
                    .build(),
                None,
            )?;

            let write_desc_sets = [
                vk::WriteDescriptorSet::builder()
                    .dst_set(desc_sets[0])
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[vk::DescriptorImageInfo {
                        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        image_view: sprite_sheet.view,
                        sampler,
                    }])
                    .build(),
                vk::WriteDescriptorSet::builder()
                    .dst_set(desc_sets[1])
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[vk::DescriptorImageInfo {
                        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        image_view: sprite_sheet.view,
                        sampler,
                    }])
                    .build(),
            ];
            vk.device.update_descriptor_sets(&write_desc_sets, &[]);

            let pipeline_layout = vk.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::builder()
                    .set_layouts(&[desc_layout, desc_layout])
                    .build(),
                None,
            )?;

            Ok(Box::new(Self {
                vertex,
                fragment,
                dynamic_entity_buffer,
                sprite_sheet,
                desc_sets,
                props_buffer,
                pipeline_layout,
                desc_layout,
                camera_query,
            }))
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn allocate_storages(
        _vk: &VulkanContext,
        _allocator: &AllocatorPtr,
    ) -> Result<(TexturePtr, BufferSet, Buffer), anyhow::Error> {
        unimplemented!()
    }

    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )] // TODO: too_many_lines
    fn build_dynamic_entity_buffer(
        &mut self,
        _world: &World,
        _resources: &Resources,
        _camera: &CameraQueryResult,
        _frame_number: usize,
    ) -> Result<usize, anyhow::Error> {
        game_metrics::scope!("render::entities::build_dynamic_entity_buffer");

        Ok(0)
    }
}
impl VkPass for SpinePass {
    fn subpass_dependency(&self, index: u32) -> vk::SubpassDependency {
        vk::SubpassDependency {
            src_subpass: index - 1,
            dst_subpass: index,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ
                | vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            ..Default::default()
        }
    }

    fn subpass(&self) -> ash::vk::SubpassDescription {
        vk::SubpassDescription::builder()
            .color_attachments(std::slice::from_ref(&vk::AttachmentReference {
                attachment: 0,
                layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            }))
            .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS)
            .build()
    }

    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    unsafe fn rebuild(
        &mut self,
        context: &RenderContext,
        subpass: u32,
    ) -> std::result::Result<vk::Pipeline, anyhow::Error> {
        let vk = &context.vk;

        let dimensions = &vk.surface.resolution;

        Ok(vk
            .device
            .create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[vk::GraphicsPipelineCreateInfo::builder()
                    .subpass(subpass)
                    .stages(&[
                        *vk::PipelineShaderStageCreateInfo::builder()
                            .module(self.vertex.module())
                            .stage(vk::ShaderStageFlags::VERTEX)
                            .name(self.vertex.entry_point()),
                        *vk::PipelineShaderStageCreateInfo::builder()
                            .module(self.fragment.module())
                            .stage(vk::ShaderStageFlags::FRAGMENT)
                            .name(self.fragment.entry_point()),
                    ])
                    .vertex_input_state(
                        &vk::PipelineVertexInputStateCreateInfo::builder()
                            .vertex_attribute_descriptions(&[
                                vk::VertexInputAttributeDescription {
                                    location: 0,
                                    binding: 0,
                                    format: vk::Format::R32G32B32_SFLOAT,
                                    offset: 0,
                                },
                                vk::VertexInputAttributeDescription {
                                    location: 1,
                                    binding: 0,
                                    format: vk::Format::R32_UINT,
                                    offset: 12,
                                },
                                vk::VertexInputAttributeDescription {
                                    location: 2,
                                    binding: 0,
                                    format: vk::Format::R8G8B8A8_UNORM,
                                    offset: 16,
                                },
                            ])
                            .vertex_binding_descriptions(&[vk::VertexInputBindingDescription {
                                binding: 0,
                                stride: (std::mem::size_of::<SpriteVert>()) as u32,
                                input_rate: vk::VertexInputRate::INSTANCE,
                            }]),
                    )
                    .depth_stencil_state(
                        &vk::PipelineDepthStencilStateCreateInfo::builder()
                            .depth_test_enable(true)
                            .depth_write_enable(false)
                            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
                            .depth_bounds_test_enable(false)
                            .stencil_test_enable(false)
                            .build(),
                    )
                    .input_assembly_state(
                        &vk::PipelineInputAssemblyStateCreateInfo::builder()
                            .topology(vk::PrimitiveTopology::TRIANGLE_STRIP),
                    )
                    .viewport_state(
                        &vk::PipelineViewportStateCreateInfo::builder()
                            .viewports(&[vk::Viewport {
                                x: 0.0,
                                y: 0.0,
                                width: dimensions.width as f32,
                                height: dimensions.height as f32,
                                min_depth: 0.0,
                                max_depth: 1.0,
                            }])
                            .scissors(&[vk::Rect2D {
                                offset: vk::Offset2D { x: 0, y: 0 },
                                extent: vk::Extent2D {
                                    width: dimensions.width,
                                    height: dimensions.height,
                                },
                            }]),
                    )
                    .rasterization_state(
                        &vk::PipelineRasterizationStateCreateInfo::builder()
                            .polygon_mode(vk::PolygonMode::FILL)
                            .cull_mode(vk::CullModeFlags::NONE)
                            .front_face(vk::FrontFace::CLOCKWISE)
                            .line_width(1.0),
                    )
                    .multisample_state(
                        &vk::PipelineMultisampleStateCreateInfo::builder()
                            .rasterization_samples(vk::SampleCountFlags::TYPE_1),
                    )
                    .color_blend_state(
                        &vk::PipelineColorBlendStateCreateInfo::builder().attachments(
                            std::slice::from_ref(
                                &vk::PipelineColorBlendAttachmentState::builder()
                                    .blend_enable(true)
                                    .src_color_blend_factor(vk::BlendFactor::SRC_ALPHA)
                                    .dst_color_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                                    .color_blend_op(vk::BlendOp::ADD)
                                    .src_alpha_blend_factor(vk::BlendFactor::ONE_MINUS_SRC_ALPHA)
                                    .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
                                    .alpha_blend_op(vk::BlendOp::ADD)
                                    .color_write_mask(vk::ColorComponentFlags::all()),
                            ),
                        ),
                    )
                    .layout(self.pipeline_layout)
                    .render_pass(context.renderpass)
                    .build()],
                None,
            )
            .unwrap()[0])
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    unsafe fn draw(
        &mut self,
        args: RenderArgs<'_>,
        _secondary_command_buffers: &mut Vec<vk::CommandBuffer>,
    ) -> Result<(), anyhow::Error> {
        game_metrics::scope!("render::entities::draw");

        let _vk = &args.render_context.vk;

        self.dynamic_entity_buffer.maintain()?;

        let _map = <Read<Map>>::fetch(&args.state.resources);

        let _ = (self.camera_query)(&args.state.world);

        Ok(())
    }
}
*/
