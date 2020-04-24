use crate::{
    alloc,
    alloc::AllocatorPtr,
    ash::{self, vk},
    data::{
        buffer::{Buffer, BufferSet},
        VulkanContext,
    },
    pass::{RenderArgs, VkPass},
    shader::{Shader, ShaderKind},
    RenderContext,
};
use ash::version::DeviceV1_0;
use rl_core::{
    camera::Camera,
    debug::{DebugLines, DebugVert},
    legion::prelude::*,
    map::Map,
    transform, GameState,
};
use rl_render_pod::{pod::SpriteProperties, std140};

pub struct DebugPass {
    vertex: Shader,
    fragment: Shader,

    desc_sets: Vec<vk::DescriptorSet>,
    desc_layout: vk::DescriptorSetLayout,

    props_buffer: Buffer,
    vertex_buffer: BufferSet,

    pipeline_layout: vk::PipelineLayout,
}

impl DebugPass {
    const FRAME_COUNT: usize = 2;

    #[allow(clippy::too_many_lines)] // TODO:
    #[allow(clippy::cast_possible_truncation)]
    pub fn new(
        state: &mut GameState,
        vk: &VulkanContext,
        allocator: &AllocatorPtr,
    ) -> Result<Box<dyn VkPass>, anyhow::Error> {
        let camera_query = <(
            Read<Camera>,
            Read<transform::Translation>,
            Read<transform::Scale>,
        )>::query();

        state.resources.insert(DebugLines::default());

        unsafe {
            let vertex =
                Shader::from_src_path(&vk.device, ShaderKind::Vertex, "assets/shaders/debug.vert")?;
            let fragment = Shader::from_src_path(
                &vk.device,
                ShaderKind::Fragment,
                "assets/shaders/debug.frag",
            )?;

            let vertex_buffer = BufferSet::new(
                &allocator,
                vk::BufferCreateInfo {
                    size: u64::from(1024 * std::mem::size_of::<DebugVert>() as u32),
                    usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                    ..Default::default()
                },
                alloc::AllocationCreateInfo {
                    usage: alloc::MemoryUsage::CpuToGpu,

                    ..Default::default()
                },
                3,
            )?;

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

            {
                if let Some(camera) = camera_query.iter(&state.world).next() {
                    props_buffer.write(
                        0,
                        &[SpriteProperties {
                            sheet_dimensions: std140::uvec2(0, 0),
                            map_dimensions: std140::uvec3(0, 0, 0),
                            sprite_dimensions: std140::uvec2(16, 24),
                            view_proj: camera.0.matrix(&camera.1, *camera.2).as_slice().into(),
                            camera_translation: camera.1.as_slice().into(),
                        }],
                    )?;
                }
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
                ]),
                None,
            )?;

            let desc_sets = vk.device.allocate_descriptor_sets(
                &vk::DescriptorSetAllocateInfo::builder()
                    .descriptor_pool(descriptor_pool)
                    .set_layouts(&[desc_layout, desc_layout])
                    .build(),
            )?;

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
                desc_sets,
                props_buffer,
                pipeline_layout,
                desc_layout,
            }))
        }
    }

    pub fn write_vertex_buffer(
        &mut self,
        lines: &mut DebugLines,
        frame_number: usize,
    ) -> Result<usize, anyhow::Error> {
        let count = lines.lines.len().min(1023);

        self.vertex_buffer.get_mut(frame_number).map_mut_with(
            |slice: &mut [DebugVert]| unsafe {
                std::ptr::copy_nonoverlapping(lines.lines.as_ptr(), slice.as_mut_ptr(), count);
            },
        )?;

        lines.clear();

        Ok(count)
    }
}
impl VkPass for DebugPass {
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
        clippy::too_many_lines
    )]
    // TODO: too_many_lines
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
                                    format: vk::Format::R8G8B8A8_UNORM,
                                    offset: 12,
                                },
                                vk::VertexInputAttributeDescription {
                                    location: 2,
                                    binding: 0,
                                    format: vk::Format::R32G32B32_SFLOAT,
                                    offset: 16,
                                },
                                vk::VertexInputAttributeDescription {
                                    location: 3,
                                    binding: 0,
                                    format: vk::Format::R8G8B8A8_UNORM,
                                    offset: 28,
                                },
                            ])
                            .vertex_binding_descriptions(&[vk::VertexInputBindingDescription {
                                binding: 0,
                                stride: (std::mem::size_of::<DebugVert>()) as u32,
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
                                    .blend_enable(false)
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
        let vk = &args.render_context.vk;

        self.vertex_buffer.maintain()?;

        let (map, mut debug_lines) =
            <(Read<Map>, Write<DebugLines>)>::fetch_unchecked(&args.state.resources);

        let count = self.write_vertex_buffer(&mut debug_lines, args.current_frame)?;
        if count < 1 {
            return Ok(());
        }

        let camera_query = <(
            Read<Camera>,
            Read<transform::Translation>,
            Read<transform::Scale>,
        )>::query();

        let camera = camera_query.iter(&args.state.world).next().unwrap();

        self.props_buffer.write(
            0,
            &[SpriteProperties {
                sheet_dimensions: std140::uvec2(0, 0),
                map_dimensions: std140::uvec3(
                    map.dimensions().x as u32,
                    map.dimensions().y as u32,
                    map.dimensions().z as u32,
                ),
                sprite_dimensions: std140::uvec2(16, 24),
                view_proj: camera.0.matrix(&camera.1, *camera.2).as_slice().into(),
                camera_translation: camera.1.as_slice().into(),
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

        vk.device.cmd_bind_vertex_buffers(
            args.command_buffer,
            0,
            std::slice::from_ref(&self.vertex_buffer.get(args.current_frame).buffer),
            &[0],
        );

        vk.device.cmd_bind_descriptor_sets(
            args.command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline_layout,
            0,
            &[self.desc_sets[args.current_frame]],
            &[],
        );

        // DRAW
        vk.device
            .cmd_draw(args.command_buffer, 4, count as u32, 0, 0);

        Ok(())
    }
}
