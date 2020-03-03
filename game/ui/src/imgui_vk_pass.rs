use crate::ImguiContextLock;
use ash::version::DeviceV1_0;
use imgui_winit_support::WinitPlatform;
use rl_core::{
    failure, legion::prelude::*, smallvec::SmallVec, winit::window::Window, GameState,
    ScreenDimensions,
};
use rl_render_vk::{
    alloc,
    alloc::AllocatorPtr,
    ash::{self, vk},
    data::{
        buffer::Buffer,
        texture::{Texture, TexturePtr},
        VulkanContext,
    },
    pass::{RenderArgs, VkPass},
    shader::{Shader, ShaderKind},
    utils, RenderContext,
};
use std::sync::Arc;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct ImguiPushConstant {
    scale: [f32; 2],
    translate: [f32; 2],
    texture_id: u32,
}

pub struct ImguiPass {
    imgui: ImguiContextLock,
    vertex: Shader,
    fragment: Shader,
    font_atlas: TexturePtr,

    desc_layout: vk::DescriptorSetLayout,

    index_buffers: SmallVec<[Buffer; 2]>,
    vertex_buffers: SmallVec<[Buffer; 2]>,

    pipeline_layout: vk::PipelineLayout,

    sampler: vk::Sampler,
}

impl ImguiPass {
    const QUAD_COUNT_PER_FRAME: usize = 64 * 1024;
    const VERTEX_COUNT_PER_FRAME: usize = 4 * Self::QUAD_COUNT_PER_FRAME;
    const INDEX_COUNT_PER_FRAME: usize = 6 * Self::QUAD_COUNT_PER_FRAME;
    const FRAME_COUNT: usize = 2;

    #[allow(clippy::too_many_lines)] // TODO:
    pub fn new(
        state: &mut GameState,
        vk: &VulkanContext,
        allocator: &AllocatorPtr,
    ) -> Result<Box<dyn VkPass>, failure::Error> {
        let (platform, window, dimensions, imgui_lock) = <(
            Read<WinitPlatform>,
            Read<Arc<Window>>,
            Read<ScreenDimensions>,
            Read<ImguiContextLock>,
        )>::fetch_mut(&mut state.resources);

        unsafe {
            let vertex =
                Shader::from_src_path(&vk.device, ShaderKind::Vertex, "assets/shaders/imgui.vert")?;
            let fragment = Shader::from_src_path(
                &vk.device,
                ShaderKind::Fragment,
                "assets/shaders/imgui.frag",
            )?;

            let (font_atlas, vertex_buffers, index_buffers) = {
                let imgui = &mut imgui_lock.lock().unwrap().context;

                imgui.io_mut().display_size =
                    [dimensions.size.width as f32, dimensions.size.height as f32];
                imgui.io_mut().display_framebuffer_scale =
                    [dimensions.dpi as f32, dimensions.dpi as f32];

                imgui
                    .io_mut()
                    .backend_flags
                    .insert(imgui::BackendFlags::RENDERER_HAS_VTX_OFFSET);

                let (font_atlas, vertex_buffers, index_buffers) =
                    Self::allocate_storages(vk, allocator, imgui)?;

                imgui.fonts().tex_id = imgui::TextureId::from(0);

                platform.prepare_frame(imgui.io_mut(), &window)?;
                crate::CURRENT_UI = Some(std::mem::transmute(imgui.frame()));

                (font_atlas, vertex_buffers, index_buffers)
            };

            let desc_pool = vk.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::builder()
                    .pool_sizes(&[vk::DescriptorPoolSize {
                        ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                        descriptor_count: 4,
                    }])
                    .max_sets(4)
                    .build(),
                None,
            )?;

            let desc_layout = vk.device.create_descriptor_set_layout(
                &vk::DescriptorSetLayoutCreateInfo::builder()
                    .bindings(&[vk::DescriptorSetLayoutBinding::builder()
                        .binding(0)
                        .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                        .descriptor_count(1)
                        .stage_flags(vk::ShaderStageFlags::FRAGMENT)
                        .build()])
                    .build(),
                None,
            )?;

            let mut mutex = imgui_lock.lock().unwrap();
            mutex.descriptor_pool = desc_pool;
            mutex.descriptor_layout = desc_layout;

            let texture_map = &mut mutex.textures;

            let font_atlas_tex_id = texture_map.insert(
                vk.device.allocate_descriptor_sets(
                    &vk::DescriptorSetAllocateInfo::builder()
                        .descriptor_pool(desc_pool)
                        .set_layouts(&[desc_layout, desc_layout])
                        .build(),
                )?[0],
            );

            let sampler = vk.device.create_sampler(
                &vk::SamplerCreateInfo::builder()
                    .mag_filter(vk::Filter::LINEAR)
                    .min_filter(vk::Filter::LINEAR)
                    .build(),
                None,
            )?;

            vk.device.update_descriptor_sets(
                &[vk::WriteDescriptorSet::builder()
                    .dst_set(*texture_map.get(font_atlas_tex_id).unwrap())
                    .dst_binding(0)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&[vk::DescriptorImageInfo {
                        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        image_view: font_atlas.view,
                        sampler,
                    }])
                    .build()],
                &[],
            );

            let pipeline_layout = vk.device.create_pipeline_layout(
                &vk::PipelineLayoutCreateInfo::builder()
                    .push_constant_ranges(&[vk::PushConstantRange {
                        stage_flags: vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                        offset: 0,
                        size: (std::mem::size_of::<ImguiPushConstant>()) as u32,
                    }])
                    .set_layouts(&[desc_layout, desc_layout])
                    .build(),
                None,
            )?;

            Ok(Box::new(Self {
                vertex,
                fragment,
                vertex_buffers,
                index_buffers,
                font_atlas,
                imgui: imgui_lock.clone(),
                pipeline_layout,
                sampler,
                desc_layout,
            }))
        }
    }

    #[allow(clippy::type_complexity, clippy::too_many_lines)]
    fn allocate_storages(
        vk: &VulkanContext,
        allocator: &AllocatorPtr,
        imgui: &mut imgui::Context,
    ) -> Result<(TexturePtr, SmallVec<[Buffer; 2]>, SmallVec<[Buffer; 2]>), failure::Error> {
        let font_atlas = Self::load_font_texture(vk, allocator, imgui)?;
        unsafe {
            font_atlas.upload(&vk.device, vk.setup_command_buffer)?;
        }

        let vertex_buffers = {
            let mut buffers = SmallVec::<[Buffer; 2]>::new();
            for _ in 0..Self::FRAME_COUNT {
                buffers.push(Buffer::new(
                    &allocator,
                    vk::BufferCreateInfo {
                        size: (Self::VERTEX_COUNT_PER_FRAME
                            * std::mem::size_of::<imgui::DrawVert>())
                            as vk::DeviceSize,
                        usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                        ..Default::default()
                    },
                    alloc::AllocationCreateInfo {
                        usage: alloc::MemoryUsage::CpuOnly,
                        flags: alloc::AllocationCreateFlags::MAPPED,
                        ..Default::default()
                    },
                )?)
            }
            buffers
        };

        let index_buffers = {
            let mut buffers = SmallVec::<[Buffer; 2]>::new();
            for _ in 0..Self::FRAME_COUNT {
                buffers.push(Buffer::new(
                    &allocator,
                    vk::BufferCreateInfo {
                        size: (Self::INDEX_COUNT_PER_FRAME * std::mem::size_of::<imgui::DrawIdx>())
                            as vk::DeviceSize,
                        usage: vk::BufferUsageFlags::INDEX_BUFFER,
                        ..Default::default()
                    },
                    alloc::AllocationCreateInfo {
                        usage: alloc::MemoryUsage::CpuOnly,
                        flags: alloc::AllocationCreateFlags::MAPPED,
                        ..Default::default()
                    },
                )?)
            }
            buffers
        };

        Ok((font_atlas, vertex_buffers, index_buffers))
    }

    fn load_font_texture(
        vk: &VulkanContext,
        allocator: &AllocatorPtr,
        imgui: &mut imgui::Context,
    ) -> Result<TexturePtr, failure::Error> {
        let mut fonts = imgui.fonts();
        let imgui::FontAtlasTexture {
            width: image_width,
            height: image_height,
            data: raw,
        } = fonts.build_rgba32_texture();

        Ok(Arc::new(Texture::from_slice(
            raw,
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
        )?))
    }
}
impl VkPass for ImguiPass {
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

    #[allow(clippy::too_many_lines)]
    unsafe fn rebuild(
        &mut self,
        context: &RenderContext,
        subpass: u32,
    ) -> std::result::Result<vk::Pipeline, failure::Error> {
        let vk = &context.vk;

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
                    .vertex_input_state(&vk::PipelineVertexInputStateCreateInfo {
                        vertex_attribute_description_count: 3,
                        p_vertex_attribute_descriptions: [
                            vk::VertexInputAttributeDescription {
                                location: 0,
                                binding: 0,
                                format: vk::Format::R32G32_SFLOAT,
                                offset: 0,
                            },
                            vk::VertexInputAttributeDescription {
                                location: 1,
                                binding: 0,
                                format: vk::Format::R32G32_SFLOAT,
                                offset: 8,
                            },
                            vk::VertexInputAttributeDescription {
                                location: 2,
                                binding: 0,
                                format: vk::Format::R8G8B8A8_UNORM,
                                offset: 16,
                            },
                        ]
                        .as_ptr(),
                        vertex_binding_description_count: 1,
                        p_vertex_binding_descriptions: [
                            vk::VertexInputBindingDescription {
                                binding: 0,
                                stride: std::mem::size_of::<imgui::DrawVert>() as u32,
                                input_rate: vk::VertexInputRate::VERTEX,
                            },
                            vk::VertexInputBindingDescription {
                                binding: 1,
                                stride: std::mem::size_of::<imgui::DrawVert>() as u32,
                                input_rate: vk::VertexInputRate::VERTEX,
                            },
                            vk::VertexInputBindingDescription {
                                binding: 2,
                                stride: std::mem::size_of::<imgui::DrawVert>() as u32,
                                input_rate: vk::VertexInputRate::VERTEX,
                            },
                        ]
                        .as_ptr(),
                        ..Default::default()
                    })
                    .depth_stencil_state(
                        &vk::PipelineDepthStencilStateCreateInfo::builder()
                            .depth_test_enable(false)
                            .depth_write_enable(false)
                            .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
                            .depth_bounds_test_enable(false)
                            .stencil_test_enable(true)
                            .build(),
                    )
                    .input_assembly_state(
                        &vk::PipelineInputAssemblyStateCreateInfo::builder()
                            .topology(vk::PrimitiveTopology::TRIANGLE_LIST),
                    )
                    .viewport_state(
                        &vk::PipelineViewportStateCreateInfo::builder()
                            .viewport_count(1)
                            .scissor_count(1),
                    )
                    .rasterization_state(
                        &vk::PipelineRasterizationStateCreateInfo::builder()
                            .polygon_mode(vk::PolygonMode::FILL)
                            .cull_mode(vk::CullModeFlags::NONE)
                            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
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
                    .dynamic_state(
                        &vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&[
                            vk::DynamicState::VIEWPORT,
                            vk::DynamicState::SCISSOR,
                        ]),
                    )
                    .layout(self.pipeline_layout)
                    .render_pass(context.renderpass)
                    .build()],
                None,
            )
            .unwrap()[0])
    }

    #[allow(
        clippy::too_many_lines,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap
    )]
    unsafe fn draw(
        &mut self,
        args: RenderArgs<'_>,
        _secondary_command_buffers: &mut Vec<vk::CommandBuffer>,
    ) -> Result<(), failure::Error> {
        let vk = &args.render_context.vk;
        let current_frame = args.current_frame;

        if let Ok(mut imgui) = self.imgui.lock() {
            //

            let [width, height] = imgui.context.io().display_size;
            let [scale_w, scale_h] = imgui.context.io().display_framebuffer_scale;

            let fb_width = width * scale_w;
            let fb_height = height * scale_h;

            let (window, platform) =
                <(Read<Arc<Window>>, Read<WinitPlatform>)>::fetch(&args.state.resources);

            let draw_data = {
                let vertex_buffer = &mut self.vertex_buffers[current_frame];
                let index_buffer = &mut self.index_buffers[current_frame];

                let ui = crate::CURRENT_UI.take();
                platform.prepare_render(ui.as_ref().unwrap(), &*window);
                let draw_data = ui.unwrap().render();

                vertex_buffer.grow(
                    draw_data.total_vtx_count as usize * std::mem::size_of::<imgui::DrawVert>(),
                )?;
                index_buffer.grow(
                    draw_data.total_idx_count as usize * std::mem::size_of::<imgui::DrawIdx>(),
                )?;

                let vtx_ptr = args
                    .render_context
                    .allocator
                    .map_memory(&vertex_buffer.allocation)?;

                let idx_ptr = args
                    .render_context
                    .allocator
                    .map_memory(&index_buffer.allocation)?;

                let mut vtx_offset: usize = 0;
                let mut idx_offset: usize = 0;

                for draw_list in draw_data.draw_lists() {
                    let vtx_len =
                        draw_list.vtx_buffer().len() * std::mem::size_of::<imgui::DrawVert>();
                    let idx_len =
                        draw_list.idx_buffer().len() * std::mem::size_of::<imgui::DrawIdx>();

                    std::ptr::copy_nonoverlapping(
                        draw_list.vtx_buffer().as_ptr() as *const u8,
                        vtx_ptr.add(vtx_offset),
                        vtx_len,
                    );

                    std::ptr::copy_nonoverlapping(
                        draw_list.idx_buffer().as_ptr() as *const u8,
                        idx_ptr.add(idx_offset),
                        idx_len,
                    );

                    vtx_offset += vtx_len;
                    idx_offset += idx_len;
                }

                args.render_context.allocator.flush_allocation(
                    &vertex_buffer.allocation,
                    0,
                    draw_data.total_vtx_count as usize * std::mem::size_of::<imgui::DrawVert>(),
                )?;
                args.render_context.allocator.flush_allocation(
                    &index_buffer.allocation,
                    0,
                    draw_data.total_idx_count as usize * std::mem::size_of::<imgui::DrawIdx>(),
                )?;

                args.render_context
                    .allocator
                    .unmap_memory(&vertex_buffer.allocation)?;

                args.render_context
                    .allocator
                    .unmap_memory(&index_buffer.allocation)?;

                draw_data
            };

            let viewports = [vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: fb_width,
                height: fb_height,
                min_depth: 0.0,
                max_depth: 1.0,
            }];
            vk.device
                .cmd_set_viewport(args.command_buffer, 0, &viewports);

            vk.device.cmd_bind_vertex_buffers(
                args.command_buffer,
                0,
                std::slice::from_ref(&self.vertex_buffers[current_frame].buffer),
                &[0],
            );
            vk.device.cmd_bind_index_buffer(
                args.command_buffer,
                self.index_buffers[current_frame].buffer,
                0,
                vk::IndexType::UINT16,
            );

            let mut constant = ImguiPushConstant {
                scale: [
                    2.0 / draw_data.display_size[0],
                    2.0 / draw_data.display_size[1],
                ],
                translate: [
                    -1.0 - draw_data.display_pos[0] * (2.0 / draw_data.display_size[0]),
                    -1.0 - draw_data.display_pos[1] * (2.0 / draw_data.display_size[1]),
                ],
                texture_id: 0,
            };

            let mut global_vtx_offset = 0;
            let mut global_idx_offset = 0;

            for draw_list in draw_data.draw_lists() {
                for cmd in draw_list.commands() {
                    match cmd {
                        imgui::DrawCmd::Elements {
                            count,
                            cmd_params:
                                imgui::DrawCmdParams {
                                    clip_rect,
                                    texture_id,
                                    idx_offset,
                                    vtx_offset,
                                },
                        } => {
                            #[allow(clippy::possible_missing_comma)]
                            let clip_rect = [
                                ((clip_rect[0] - draw_data.display_pos[0])
                                    * draw_data.framebuffer_scale[0])
                                    .max(0.0),
                                ((clip_rect[1] - draw_data.display_pos[1])
                                    * draw_data.framebuffer_scale[1])
                                    .max(0.0),
                                (clip_rect[2] - draw_data.display_pos[0])
                                    * draw_data.framebuffer_scale[0],
                                (clip_rect[3] - draw_data.display_pos[1])
                                    * draw_data.framebuffer_scale[1],
                            ];

                            vk.device.cmd_bind_descriptor_sets(
                                args.command_buffer,
                                vk::PipelineBindPoint::GRAPHICS,
                                self.pipeline_layout,
                                0,
                                &[*(imgui.textures.get(texture_id).unwrap())],
                                &[],
                            );

                            constant.texture_id = texture_id.id() as u32;
                            vk.device.cmd_push_constants(
                                args.command_buffer,
                                self.pipeline_layout,
                                vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT,
                                0,
                                utils::cast_slice(&[constant]),
                            );

                            let scissors = [vk::Rect2D {
                                offset: vk::Offset2D {
                                    x: (clip_rect[0] as i32).max(0),
                                    y: (clip_rect[1] as i32).max(0),
                                },
                                extent: vk::Extent2D {
                                    width: (clip_rect[2] - clip_rect[0]) as u32,
                                    height: (clip_rect[3] - clip_rect[1]) as u32,
                                },
                            }];
                            vk.device.cmd_set_scissor(args.command_buffer, 0, &scissors);

                            vk.device.cmd_draw_indexed(
                                args.command_buffer,
                                count as u32,
                                1,
                                global_idx_offset + idx_offset as u32,
                                global_vtx_offset + vtx_offset as i32,
                                0,
                            );
                        }
                        _ => panic!("Unsupported Draw Command in ImGui pass"),
                    }
                }

                global_idx_offset += draw_list.idx_buffer().len() as u32;

                global_vtx_offset += draw_list.vtx_buffer().len() as i32;
            }

            platform.prepare_frame(imgui.context.io_mut(), &*window)?;
            crate::CURRENT_UI = Some(std::mem::transmute(imgui.context.frame()));
        }

        Ok(())
    }
}
