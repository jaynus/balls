use crate::{
    alloc,
    alloc::AllocatorPtr,
    ash::{self, vk},
    data::{
        buffer::{Buffer, BufferSet},
        texture::{Texture, TexturePtr},
        VulkanContext,
    },
    pass::{RenderArgs, VkPass},
    shader::{Shader, ShaderKind},
    RenderContext,
};
use ash::version::DeviceV1_0;
use rl_core::{
    camera::{make_camera_query, CameraQueryFn, CameraQueryResult},
    components::PositionComponent,
    legion::prelude::*,
    map::Map,
    GameState,
};
use rl_render_pod::{
    pod::{Properties, SparseSpriteVert},
    sprite::{SparseSprite, SparseSpriteArray},
};
use std::sync::Arc;

pub struct SparseSpritePass {
    vertex: Shader,
    fragment: Shader,

    desc_sets: Vec<vk::DescriptorSet>,
    desc_layout: vk::DescriptorSetLayout,

    props_buffer: Buffer,

    vertex_buffer: BufferSet,

    sprite_sheet: TexturePtr,

    pipeline_layout: vk::PipelineLayout,

    camera_query: CameraQueryFn,
}

impl SparseSpritePass {
    const FRAME_COUNT: usize = 2;

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::too_many_lines,
        clippy::cast_sign_loss
    )]
    pub fn new(
        state: &mut GameState,
        vk: &VulkanContext,
        allocator: &AllocatorPtr,
    ) -> Result<Box<dyn VkPass>, anyhow::Error> {
        unsafe {
            let vertex = Shader::from_src_path(
                &vk.device,
                ShaderKind::Vertex,
                "assets/shaders/sparse_sprites.vert",
            )?;
            let fragment = Shader::from_src_path(
                &vk.device,
                ShaderKind::Fragment,
                "assets/shaders/sparse_sprites.frag",
            )?;

            let (sprite_sheet, vertex_buffer) = Self::allocate_storages(vk, allocator)?;

            sprite_sheet.upload(&vk.device, vk.setup_command_buffer)?;

            let mut props_buffer = Buffer::new(
                &allocator,
                vk::BufferCreateInfo {
                    size: std::mem::size_of::<Properties>() as u64,
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
                    &[Properties {
                        view_proj: camera
                            .camera
                            .matrix(&camera.translation, camera.scale)
                            .as_slice()
                            .into(),
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
                vertex_buffer,
                sprite_sheet,
                desc_sets,
                props_buffer,
                pipeline_layout,
                desc_layout,
                camera_query,
            }))
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::too_many_lines,
        clippy::cast_sign_loss
    )]
    fn allocate_storages(
        vk: &VulkanContext,
        allocator: &AllocatorPtr,
    ) -> Result<(TexturePtr, BufferSet), anyhow::Error> {
        let path = std::path::Path::new("assets/sparse.png");
        let (image_width, image_height) = rl_core::image::image_dimensions(&path)?;

        let sprite_sheet = Arc::new(Texture::from_slice(
            &rl_core::image::open(path)?
                .as_flat_samples_u8()
                .ok_or_else(|| anyhow::anyhow!("Failed to open image"))?
                .as_slice(),
            &vk.device,
            &allocator,
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

        let vertex_buffer = BufferSet::new(
            &allocator,
            vk::BufferCreateInfo {
                size: u64::from(1024 * std::mem::size_of::<SparseSpriteVert>() as u32),
                usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                ..Default::default()
            },
            alloc::AllocationCreateInfo {
                usage: alloc::MemoryUsage::CpuToGpu,
                required_flags: vk::MemoryPropertyFlags::HOST_VISIBLE
                    | vk::MemoryPropertyFlags::HOST_COHERENT,
                ..Default::default()
            },
            3,
        )?;

        Ok((sprite_sheet, vertex_buffer))
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::too_many_lines,
        clippy::cast_sign_loss
    )]
    fn build_vertex_buffer(
        &mut self,
        world: &World,
        map: &Map,
        camera: &CameraQueryResult,
        frame_number: usize,
    ) -> Result<usize, anyhow::Error> {
        let sparse_sprite_query = <(Read<PositionComponent>, Read<SparseSprite>)>::query();
        let sparse_array_query = <(Read<PositionComponent>, Read<SparseSpriteArray>)>::query();

        let current_z = camera.translation.z.floor() as i32;

        let mut grow = false;
        let mut count = 0;
        self.vertex_buffer.get_mut(frame_number).map_mut_with(
            |slice: &mut [SparseSpriteVert]| {
                for (position, sprite) in sparse_sprite_query.iter(world) {
                    let translation = map.tile_to_world(**position);

                    if position.z == current_z {
                        slice[count] = (*sprite).into();
                        // Make the position relative to the origin of the entity
                        slice[count].position = [
                            slice[count].position[0] + translation.x,
                            slice[count].position[1] + translation.y,
                            slice[count].position[2] + 2.0,
                        ];
                        count += 1;
                        if count == slice.len() {
                            // Bail, we will resize next frame for now
                            grow = true;
                            break;
                        }
                    }
                }
                for (position, sprite_array) in sparse_array_query.iter(world) {
                    let translation = map.tile_to_world(**position);

                    if position.z == current_z {
                        for sprite in sprite_array.values() {
                            slice[count] = (*sprite).into();
                            // Make the position relative to the origin of the entity
                            slice[count].position = [
                                slice[count].position[0] + translation.x,
                                slice[count].position[1] + translation.y,
                                slice[count].position[2] + 2.0,
                            ];
                            count += 1;
                            if count == slice.len() {
                                // Bail, we will resize next frame for now
                                grow = true;
                                break;
                            }
                        }
                    }
                }
            },
        )?;

        if grow {
            self.vertex_buffer
                .grow(self.vertex_buffer.get(frame_number).buffer_info.size as usize * 2)?;
        }

        self.vertex_buffer.get_mut(frame_number).flush(None)?;

        Ok(count)
    }
}
impl VkPass for SparseSpritePass {
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
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::too_many_lines,
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
                                    format: vk::Format::R32G32_SFLOAT,
                                    offset: 12,
                                },
                                vk::VertexInputAttributeDescription {
                                    location: 2,
                                    binding: 0,
                                    format: vk::Format::R32G32_SFLOAT,
                                    offset: 20,
                                },
                                vk::VertexInputAttributeDescription {
                                    location: 3,
                                    binding: 0,
                                    format: vk::Format::R8G8B8A8_UNORM,
                                    offset: 28,
                                },
                                vk::VertexInputAttributeDescription {
                                    location: 4,
                                    binding: 0,
                                    format: vk::Format::R32G32_SFLOAT,
                                    offset: 32,
                                },
                                vk::VertexInputAttributeDescription {
                                    location: 5,
                                    binding: 0,
                                    format: vk::Format::R32G32_SFLOAT,
                                    offset: 40,
                                },
                            ])
                            .vertex_binding_descriptions(&[vk::VertexInputBindingDescription {
                                binding: 0,
                                stride: (std::mem::size_of::<SparseSpriteVert>()) as u32,
                                input_rate: vk::VertexInputRate::INSTANCE,
                            }]),
                    )
                    .depth_stencil_state(
                        &vk::PipelineDepthStencilStateCreateInfo::builder()
                            .depth_test_enable(false)
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

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::too_many_lines,
        clippy::cast_sign_loss
    )]
    unsafe fn draw(
        &mut self,
        args: RenderArgs<'_>,
        _secondary_command_buffers: &mut Vec<vk::CommandBuffer>,
    ) -> Result<(), anyhow::Error> {
        let vk = &args.render_context.vk;

        self.vertex_buffer.maintain()?;

        let camera = (self.camera_query)(&args.state.world);

        let entity_count = self.build_vertex_buffer(
            &args.state.world,
            &args.state.resources.get::<Map>().unwrap(),
            &camera,
            args.current_frame,
        )?;
        if entity_count < 1 {
            return Ok(());
        }

        self.props_buffer.write(
            0,
            &[Properties {
                view_proj: camera
                    .camera
                    .matrix(&camera.translation, camera.scale)
                    .as_slice()
                    .into(),
            }],
        )?;
        self.props_buffer.flush(None)?;

        vk.device.update_descriptor_sets(
            &[vk::WriteDescriptorSet::builder()
                .dst_set(self.desc_sets[args.current_frame])
                .dst_binding(0)
                .descriptor_type(vk::DescriptorType::UNIFORM_BUFFER)
                .buffer_info(&[vk::DescriptorBufferInfo::builder()
                    .buffer(self.props_buffer.buffer)
                    .range(vk::WHOLE_SIZE)
                    .build()])
                .build()],
            &[],
        );

        vk.device.cmd_bind_descriptor_sets(
            args.command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline_layout,
            0,
            &[self.desc_sets[args.current_frame]],
            &[],
        );

        vk.device.cmd_bind_vertex_buffers(
            args.command_buffer,
            0,
            std::slice::from_ref(&self.vertex_buffer.get(args.current_frame).buffer),
            &[0],
        );

        vk.device
            .cmd_draw(args.command_buffer, 4, entity_count as u32, 0, 0);

        Ok(())
    }
}
