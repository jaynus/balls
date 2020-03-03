use crate::{
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
    RenderContext,
};
use ash::version::DeviceV1_0;
use rl_core::{
    camera::{make_camera_query, CameraQueryFn, CameraQueryResult},
    failure,
    legion::prelude::*,
    map::Map,
    rayon::prelude::*,
    settings::Settings,
    smallvec::SmallVec,
    GameState,
};
use rl_render_pod::{
    pod::{MapVert, SpriteProperties},
    sprite::Sprite,
    std140,
};
use std::sync::Arc;

pub struct MapPass {
    vertex: Shader,
    fragment: Shader,

    desc_sets: Vec<vk::DescriptorSet>,
    desc_layout: vk::DescriptorSetLayout,

    props_buffer: Buffer,

    vertex_buffer: Buffer,

    sprite_sheet: Vec<TexturePtr>,

    pipeline_layout: vk::PipelineLayout,

    last_map_version: u64,

    camera_query: CameraQueryFn,
    last_camera: CameraQueryResult,
    last_settings_version: u64,
}

impl MapPass {
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
    ) -> Result<Box<dyn VkPass>, failure::Error> {
        unsafe {
            let vertex =
                Shader::from_src_path(&vk.device, ShaderKind::Vertex, "assets/shaders/tiles.vert")?;
            let fragment = Shader::from_src_path(
                &vk.device,
                ShaderKind::Fragment,
                "assets/shaders/tiles.frag",
            )?;
            let map = state.resources.get::<Map>().unwrap();

            let last_settings_version = state.resources.get::<Settings>().unwrap().version;

            let (sprite_sheet, mut vertex_buffer) = Self::allocate_storages(vk, allocator, &map)?;

            for tex in sprite_sheet.iter() {
                tex.upload(&vk.device, vk.setup_command_buffer)?;
            }

            let props_buffer = Buffer::new(
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

            let mut camera_query = make_camera_query();
            let last_camera = camera_query(&state.world);

            Self::populate_map_buffer(
                vk,
                &mut vertex_buffer,
                &map,
                &state.world,
                &state.resources,
            )?;

            let last_map_version = map.version().version;

            let descriptor_pool = vk.device.create_descriptor_pool(
                &vk::DescriptorPoolCreateInfo::builder()
                    .pool_sizes(&[
                        vk::DescriptorPoolSize {
                            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                            descriptor_count: 1024,
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
                        .descriptor_count(512)
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
            println!("Created: {:?}", desc_sets);

            let sampler = vk.device.create_sampler(
                &vk::SamplerCreateInfo::builder()
                    .min_filter(vk::Filter::LINEAR)
                    .mag_filter(vk::Filter::LINEAR)
                    .build(),
                None,
            )?;

            let mut image_views = Vec::new();
            println!("Uploading {} sprites", sprite_sheet.len());
            for tex in &sprite_sheet {
                image_views.push(vk::DescriptorImageInfo {
                    image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    image_view: tex.view,
                    sampler,
                });
            }

            // Fill the rest with question marks
            for _ in sprite_sheet.len()..512 {
                image_views.push(vk::DescriptorImageInfo {
                    image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                    image_view: sprite_sheet[63].view,
                    sampler,
                });
            }

            let write_desc_sets = [
                vk::WriteDescriptorSet::builder()
                    .dst_set(desc_sets[0])
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&image_views)
                    .build(),
                vk::WriteDescriptorSet::builder()
                    .dst_set(desc_sets[1])
                    .dst_binding(1)
                    .descriptor_type(vk::DescriptorType::COMBINED_IMAGE_SAMPLER)
                    .image_info(&image_views)
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
                last_map_version,
                last_settings_version,
                pipeline_layout,
                desc_layout,
                last_camera,
                camera_query,
            }))
        }
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    fn get_tile_vertex(index: usize, map: &Map, world: &World, resources: &Resources) -> MapVert {
        game_metrics::scope!("render::map::get_tile_vertex");

        let coord = map.encoder().decode(index);
        let settings = resources.get::<Settings>().unwrap();

        let tile = map.get(coord);
        if let Some(Sprite {
            sprite_number,
            color,
            ..
        }) = tile.sprite(&coord, map, world, resources)
        {
            MapVert {
                index: index as u32,
                sprite_number,
                color: color.pack(),
                //color: settings.palette().red.pack(),
            }
        } else {
            MapVert {
                index: index as u32,
                sprite_number: rl_render_pod::sprite::sprite_map::WALL,
                color: settings.palette().blue_empty.pack(),
            }
        }
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn update_map_buffer(
        &mut self,
        map: &Map,
        world: &World,
        resources: &Resources,
    ) -> Result<usize, failure::Error> {
        game_metrics::scope!("render::map::update_map_buffer");

        let mut count = 0;

        if map.version().dirty.len() > 0 {
            let mut dirty = map
                .version()
                .dirty
                .iter()
                .map(|v| v.0)
                .collect::<SmallVec<[usize; 32]>>();
            dirty.sort();

            let mut flush_ranges = SmallVec::<[std::ops::Range<usize>; 6]>::default();

            self.vertex_buffer.map_mut_with(|slice: &mut [MapVert]| {
                let mut range = std::ops::Range {
                    start: dirty[0],
                    end: dirty[0],
                };

                for index in dirty.drain(..) {
                    count += 1;

                    if index > range.start + 2048 {
                        flush_ranges.push(range.clone());
                        range = std::ops::Range {
                            start: index,
                            end: index,
                        };
                    } else {
                        range.end = index;
                    }

                    slice[index as usize] = Self::get_tile_vertex(index, map, world, resources);
                }
            })?;

            for range in flush_ranges.drain(..) {
                self.vertex_buffer.flush(Some(range))?;
            }
        }
        self.last_map_version = map.version().version;

        Ok(count)
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    fn populate_map_buffer(
        _vk: &VulkanContext,
        buffer: &mut Buffer,
        map: &Map,
        world: &World,
        resources: &Resources,
    ) -> Result<(), failure::Error> {
        game_metrics::scope!("render::map::populate_map_buffer");

        buffer.map_mut_with(|slice: &mut [MapVert]| {
            (0..slice.len()).into_par_iter().for_each(|index| {
                let vert = unsafe { &mut *(slice.as_ptr() as *mut MapVert).add(index) };

                *vert = Self::get_tile_vertex(index as usize, map, world, resources);
            });
        })?;
        buffer.flush(None)?;

        Ok(())
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    fn allocate_storages(
        vk: &VulkanContext,
        allocator: &AllocatorPtr,
        map: &Map,
    ) -> Result<(Vec<TexturePtr>, Buffer), failure::Error> {
        let path = std::path::Path::new("assets/cp437.png");
        let (image_width, image_height) = rl_core::image::image_dimensions(&path)?;

        let image = rl_core::image::open(path)?.into_rgba();

        let sprite_count = (image_width as i32 / map.sprite_dimensions.x)
            * (image_height as i32 / map.sprite_dimensions.y);

        let rows = image_width as i32 / map.sprite_dimensions.x;

        let mut sprite_sheet = Vec::new();
        for n in 0..sprite_count {
            let mut sample = SmallVec::<[u8; 4 * 16 * 24]>::with_capacity(
                (map.sprite_dimensions.x * map.sprite_dimensions.y) as usize * 4,
            );

            let row = n % rows;
            let col = n / rows;

            for y in col * 24..(col + 1) * 24 {
                for x in row * 16..(row + 1) * 16 {
                    let pixel = image.get_pixel(x as u32, y as u32);
                    sample.push(pixel[0] as u8);
                    sample.push(pixel[1] as u8);
                    sample.push(pixel[2] as u8);
                    sample.push(pixel[3] as u8);
                }
            }

            sprite_sheet.push(Arc::new(Texture::from_slice(
                &sample,
                &vk.device,
                &allocator,
                vk::ImageCreateInfo::builder()
                    .image_type(vk::ImageType::TYPE_2D)
                    .format(vk::Format::R8G8B8A8_UNORM)
                    .extent(vk::Extent3D {
                        width: map.sprite_dimensions.x as u32,
                        height: map.sprite_dimensions.y as u32,
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
            )?));
        }

        let vertex_buffer = Buffer::new(
            &allocator,
            vk::BufferCreateInfo {
                size: u64::from(
                    map.dimensions().x as u32
                        * map.dimensions().y as u32
                        * map.dimensions().z as u32
                        * std::mem::size_of::<MapVert>() as u32,
                ),
                usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                ..Default::default()
            },
            alloc::AllocationCreateInfo {
                usage: alloc::MemoryUsage::CpuToGpu,

                ..Default::default()
            },
        )?;

        Ok((sprite_sheet, vertex_buffer))
    }
}
impl VkPass for MapPass {
    fn subpass_dependency(&self, _index: u32) -> vk::SubpassDependency {
        vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
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
        clippy::cast_sign_loss,
        clippy::too_many_lines
    )]
    unsafe fn rebuild(
        &mut self,
        context: &RenderContext,
        subpass: u32,
    ) -> std::result::Result<vk::Pipeline, failure::Error> {
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
                                    format: vk::Format::R32_UINT,
                                    offset: 0,
                                },
                                vk::VertexInputAttributeDescription {
                                    location: 1,
                                    binding: 0,
                                    format: vk::Format::R32_UINT,
                                    offset: 4,
                                },
                                vk::VertexInputAttributeDescription {
                                    location: 2,
                                    binding: 0,
                                    format: vk::Format::R8G8B8A8_UNORM,
                                    offset: 8,
                                },
                            ])
                            .vertex_binding_descriptions(&[vk::VertexInputBindingDescription {
                                binding: 0,
                                stride: (std::mem::size_of::<MapVert>()) as u32,
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
        clippy::cast_sign_loss,
        clippy::too_many_lines
    )]
    unsafe fn draw(
        &mut self,
        args: RenderArgs<'_>,
        _secondary_command_buffers: &mut Vec<vk::CommandBuffer>,
    ) -> Result<(), failure::Error> {
        game_metrics::scope!("render::map::draw");

        let vk = &args.render_context.vk;

        let map = <Read<Map>>::fetch(&args.state.resources);

        let camera = (self.camera_query)(&args.state.world);

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
                view_proj: self
                    .last_camera
                    .camera
                    .matrix(&camera.translation, camera.scale)
                    .as_slice()
                    .into(),
                camera_translation: camera.translation.as_slice().into(),
            }],
        )?;
        self.props_buffer.flush(None)?;

        let settings_version = args.state.resources.get::<Settings>().unwrap().version;

        if self.last_settings_version != settings_version {
            self.last_settings_version = settings_version;
            Self::populate_map_buffer(
                vk,
                &mut self.vertex_buffer,
                &map,
                &args.state.world,
                &args.state.resources,
            )?
        }

        if map.version().version != self.last_map_version {
            self.update_map_buffer(&map, &args.state.world, &args.state.resources)?;
        }
        let current_z = self.last_camera.translation.z.floor() as i32;

        self.last_camera = camera;

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
            std::slice::from_ref(&self.vertex_buffer.buffer),
            &[(current_z
                * map.dimensions().x
                * map.dimensions().y
                * std::mem::size_of::<MapVert>() as i32) as u64],
        );

        vk.device.cmd_bind_descriptor_sets(
            args.command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            self.pipeline_layout,
            0,
            &[self.desc_sets[args.current_frame]],
            &[],
        );

        vk.device.cmd_draw(
            args.command_buffer,
            4,
            map.dimensions().x as u32 * map.dimensions().y as u32,
            0,
            0,
        );

        Ok(())
    }
}
