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

#[derive(Clone, Copy)]
pub struct StaticSpriteQueryResult<'a> {
    pub sprite: &'a StaticSpriteTag,
    pub positions: &'a [PositionComponent],
    pub dimensions: &'a [DimensionsComponent],
}

pub type StaticSpriteQueryChangedFn =
    Box<dyn FnMut(&World, &mut dyn FnMut(StaticSpriteQueryResult))>;
pub type StaticSpriteDestroyedFn = Box<dyn FnMut(&World, &mut dyn FnMut(Entity))>;

fn make_static_sprites_changed_query() -> StaticSpriteQueryChangedFn {
    let active_query = <(Read<PositionComponent>, Read<DimensionsComponent>)>::query().filter(
        tag::<StaticSpriteTag>()
            & !component::<Destroy>()
            & (changed::<PositionComponent>() | changed::<DimensionsComponent>()),
    );

    Box::new(move |world, f| {
        active_query.iter_chunks(world).for_each(|chunk| {
            let sprite = chunk.tag::<StaticSpriteTag>().unwrap();
            let positions = chunk.components::<PositionComponent>().unwrap();
            let dimensions = chunk.components::<DimensionsComponent>().unwrap();

            (f)(StaticSpriteQueryResult {
                sprite,
                positions: &positions,
                dimensions: &dimensions,
            })
        })
    })
}

fn make_static_sprites_destroy_query() -> StaticSpriteDestroyedFn {
    let active_query = <(Read<PositionComponent>, Read<DimensionsComponent>)>::query()
        .filter(tag::<StaticSpriteTag>() & component::<Destroy>());

    Box::new(move |world, f| {
        active_query
            .iter_entities(world)
            .for_each(|(entity, (_, _))| (f)(entity))
    })
}

pub struct EntitiesPass {
    vertex: Shader,
    fragment: Shader,

    desc_sets: Vec<vk::DescriptorSet>,
    desc_layout: vk::DescriptorSetLayout,

    props_buffer: Buffer,

    dynamic_entity_buffer: BufferSet,
    static_entity_buffer: Buffer,

    sprite_sheet: TexturePtr,

    pipeline_layout: vk::PipelineLayout,

    camera_query: CameraQueryFn,

    static_sprite_changed_query: StaticSpriteQueryChangedFn,
    static_sprite_destroyed_query: StaticSpriteDestroyedFn,

    static_entity_cache: FxHashMap<Entity, SmallVec<[usize; 4]>>,
    static_entity_count: usize,
    static_entity_freelist: Vec<usize>,
}

impl EntitiesPass {
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

            let (sprite_sheet, dynamic_entity_buffer, mut static_entity_buffer) =
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

            let mut static_entity_cache = FxHashMap::default();
            let static_entity_count = Self::init_static_entities(
                &state.world,
                &state.resources,
                &camera,
                &mut static_entity_buffer,
                &mut static_entity_cache,
            )?;

            Ok(Box::new(Self {
                vertex,
                fragment,
                dynamic_entity_buffer,
                static_entity_buffer,
                sprite_sheet,
                desc_sets,
                props_buffer,
                pipeline_layout,
                desc_layout,
                camera_query,
                static_sprite_changed_query: make_static_sprites_changed_query(),
                static_sprite_destroyed_query: make_static_sprites_destroy_query(),
                static_entity_cache,
                static_entity_count,
                static_entity_freelist: Vec::with_capacity(1024),
            }))
        }
    }

    #[allow(clippy::cast_possible_truncation)]
    fn allocate_storages(
        vk: &VulkanContext,
        allocator: &AllocatorPtr,
    ) -> Result<(TexturePtr, BufferSet, Buffer), anyhow::Error> {
        let path = std::path::Path::new("assets/cp437.png");
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

        let dynamic_entity_buffer = BufferSet::new(
            &allocator,
            vk::BufferCreateInfo {
                size: u64::from(1024 * std::mem::size_of::<SpriteVert>() as u32),
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

        let static_entity_buffer = Buffer::new(
            &allocator,
            vk::BufferCreateInfo {
                size: u64::from(1024 * std::mem::size_of::<SpriteVert>() as u32),
                usage: vk::BufferUsageFlags::VERTEX_BUFFER,
                ..Default::default()
            },
            alloc::AllocationCreateInfo {
                usage: alloc::MemoryUsage::CpuToGpu,
                required_flags: vk::MemoryPropertyFlags::HOST_VISIBLE
                    | vk::MemoryPropertyFlags::HOST_COHERENT,
                ..Default::default()
            },
        )?;

        Ok((sprite_sheet, dynamic_entity_buffer, static_entity_buffer))
    }

    /*
    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )] // TODO: too_many_lines
    fn draw_culled(
        &mut self,
        world: &World,
        resources: &Resources,
        camera: &CameraQueryResult,
        frame_number: usize,
    ) -> Result<usize, anyhow::Error> {
        let (map, spatial_map) = <(Read<Map>, Read<SpatialMap>)>::fetch(&resources);

        // Build the aabb from the camera.
        let view_area_f = (camera.camera.dimensions * *camera.scale) / 2.0;
        let view_area = Vec3i::new(
            view_area_f.x.floor() as i32,
            view_area_f.y.floor() as i32,
            0,
        );
        let camera_translation = map.world_to_tile(*camera.translation);

        self.dynamic_entity_buffer
            .get_mut(frame_number)
            .map_mut_with(|slice: &mut [SpriteVert]| {
                for entry in
                    spatial_map.locate_in_envelope_intersecting(&rstar::AABB::from_corners(
                        (camera_translation - view_area - Vec3i::new(1, 1, 0)).into(),
                        (camera_translation + view_area + Vec3i::new(1, 1, 0)).into(),
                    ))
                {}
            })?;

        Ok(0)
    }*/

    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )] // TODO: too_many_lines
    fn init_static_entities(
        world: &World,
        resources: &Resources,
        _camera: &CameraQueryResult,
        static_entity_buffer: &mut Buffer,
        static_entity_cache: &mut FxHashMap<Entity, SmallVec<[usize; 4]>>,
    ) -> Result<usize, anyhow::Error> {
        let map = resources.get::<Map>().unwrap();

        let query = <(Read<PositionComponent>, Read<DimensionsComponent>)>::query()
            .filter(tag::<StaticSpriteTag>() & !component::<Destroy>());

        let count = query.iter(world).count();
        *static_entity_cache = FxHashMap::with_capacity_and_hasher(count, FxBuildHasher::default());

        static_entity_buffer.grow(count * std::mem::size_of::<SpriteVert>() * 4)?;

        let mut index: usize = 0;
        static_entity_buffer.map_mut_with(|slice: &mut [SpriteVert]| {
            query.iter_chunks(world).for_each(|mut chunk| {
                let sprite = *chunk.tag::<StaticSpriteTag>().unwrap();
                for (entity, (position, dimensions)) in chunk.iter_entities_mut() {
                    for coord in dimensions.occupies_limit_z(**position).iter() {
                        static_entity_cache
                            .entry(entity)
                            .or_insert_with(SmallVec::default)
                            .push(index);

                        let mut translation = map.tile_to_world(coord);
                        translation.z += 0.99;

                        slice[index] = SpriteVert {
                            pos: *translation.as_array(),
                            sprite_number: sprite.sprite_number,
                            color: sprite.color.pack(),
                        };

                        index += 1;
                    }
                }
            });
        })?;

        Ok(index)
    }

    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )] // TODO: too_many_lines
    fn update_static_entities(
        &mut self,
        world: &World,
        _resources: &Resources,
        _camera: &CameraQueryResult,
        _frame_number: usize,
    ) -> Result<usize, anyhow::Error> {
        (self.static_sprite_changed_query)(world, &mut |_chunk| {
            // TODO: spawns and moves
        });

        let mut removal_entities = SmallVec::<[Entity; 64]>::default();
        (self.static_sprite_destroyed_query)(world, &mut |entity| {
            removal_entities.push(entity);
            println!("!! Removing entity: {:?}", entity);
        });

        let mut removals = SmallVec::<[usize; 128]>::default();
        removal_entities.drain(..).for_each(|entity| {
            removals.extend(self.static_entity_cache.remove(&entity).unwrap());
        });

        self.static_entity_buffer
            .map_mut_with(|slice: &mut [SpriteVert]| {
                removals.iter().for_each(|index| {
                    slice[*index].color = Color::a(0.0).pack();
                });
            })?;

        removals.drain(..).for_each(|index| {
            self.static_entity_buffer.flush(Some(index..index)).unwrap();
        });
        Ok(0)
    }

    #[allow(
        clippy::too_many_lines,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )] // TODO: too_many_lines
    fn build_dynamic_entity_buffer(
        &mut self,
        world: &World,
        resources: &Resources,
        camera: &CameraQueryResult,
        frame_number: usize,
    ) -> Result<usize, anyhow::Error> {
        game_metrics::scope!("render::entities::build_dynamic_entity_buffer");

        //self.draw_culled(world, camera, frame_number);

        let map = resources.get::<Map>().unwrap();

        let mut grow = false;
        let mut count = 0;
        self.dynamic_entity_buffer
            .get_mut(frame_number)
            .map_mut_with(|slice: &mut [SpriteVert]| {
                let mut do_entity = |_: Entity,
                                     position: Option<Ref<'_, PositionComponent>>,
                                     dimensions: Option<Ref<'_, DimensionsComponent>>,
                                     translation: Option<Ref<'_, Translation>>,
                                     sprite: &Sprite,
                                     layer: SpriteLayer| {
                    if grow {
                        return;
                    }

                    let dimensions = if let Some(dimensions) = dimensions {
                        *dimensions
                    } else {
                        DimensionsComponent::default()
                    };

                    let position = if let Some(position) = position {
                        **position
                    } else if let Some(translation) = translation {
                        map.world_to_tile(**translation)
                    } else {
                        return;
                    };

                    let z_count = 5;
                    let camera_z = camera.translation.z as i32;
                    let step = 1.0 / z_count as f32;
                    let mut difference = position.z as f32 - camera_z as f32;

                    if difference < 0.0 || difference > 5.0 {
                        return;
                    }

                    let mut color = sprite.color;
                    let tile_dimensions = dimensions.as_tiles();
                    if camera_z < position.z - tile_dimensions.z {
                        difference -= tile_dimensions.z as f32;
                        if difference > 0.0 && difference < 5.0 {
                            color.a = 1.0 - (step * difference);
                            color.b = step * difference;
                        }
                    }

                    for coord in dimensions.occupies_limit_z(position).iter() {
                        if difference > tile_dimensions.z as f32 {
                            difference -= tile_dimensions.z as f32;

                            if difference > 0.0 && difference < 5.0 {
                                for n in 1..=difference as i32 {
                                    if !map
                                        .get(
                                            (position - Vec3i::new(0, 0, tile_dimensions.z))
                                                - Vec3i::new(0, 0, n),
                                        )
                                        .is_empty()
                                    {
                                        continue;
                                    }
                                }
                            } else if difference > 5.0 || difference < 0.0 {
                                continue;
                            }
                        }

                        let mut translation = map.tile_to_world(coord);
                        translation.z += 1.0 - layer.into_f32();
                        slice[count] = SpriteVert {
                            pos: *translation.as_array(),
                            sprite_number: sprite.sprite_number,
                            color: color.pack(),
                        };

                        if count >= slice.len() - 1 {
                            grow = true;
                            return;
                        }
                        count += 1;
                    }
                    //}
                };

                // layered drawing
                // Build the aabb from the camera.
                let view_area_f = (camera.camera.dimensions * *camera.scale) / 2.0;
                let view_area = Vec3i::new(
                    view_area_f.x.floor() as i32,
                    view_area_f.y.floor() as i32,
                    0,
                );
                let camera_translation = map.world_to_tile(*camera.translation);

                let aabb = Aabbi::new(
                    camera_translation - view_area - Vec3i::new(1, 1, 0),
                    camera_translation + view_area + Vec3i::new(1, 1, 0),
                );

                for layer in SpriteLayer::iter() {
                    for (entity, (position, dimensions, translation, sprite)) in <(
                        TryRead<PositionComponent>,
                        TryRead<DimensionsComponent>,
                        TryRead<Translation>,
                        Read<Sprite>,
                    )>::query(
                    )
                    .filter(
                        tag_value::<SpriteLayer>(&layer)
                            & !component::<Destroy>()
                            & !tag::<StaticSpriteTag>(),
                    )
                    .iter_entities(world)
                    {
                        if let Some(position) = position.as_ref() {
                            if !aabb.contains(***position) {
                                continue;
                            }
                        }
                        do_entity(entity, position, dimensions, translation, &sprite, layer);
                    }
                }

                // Foliage layer for tasks on foliage
                // virtual tasks have their own sprite, while we need to draw a task sprite
                // over entities with tasks that arnt taged virtual

                // TODO: const hardcode this sprite value somewhere
                let task_color = resources
                    .get::<Settings>()
                    .unwrap()
                    .palette()
                    .task_designation;
                let task_sprite = Sprite::new(sprite_map::FLOOR, task_color & Color::a(0.5));
                for (entity, (position, dimensions, translation, tasks)) in <(
                    TryRead<PositionComponent>,
                    TryRead<DimensionsComponent>,
                    TryRead<Translation>,
                    Read<HasTasksComponent>,
                )>::query(
                )
                .filter(!tag::<VirtualTaskTag>() & !component::<Destroy>() & tag::<FoliageTag>())
                .iter_entities(world)
                {
                    if let Some(position) = position.as_ref() {
                        if !aabb.contains(***position) {
                            continue;
                        }
                    }

                    if !tasks.is_empty() {
                        do_entity(
                            entity,
                            position,
                            dimensions,
                            translation,
                            &task_sprite,
                            SpriteLayer::Overlay,
                        );
                    }
                }

                // No layer
                for (entity, (position, dimensions, translation, sprite)) in <(
                    TryRead<PositionComponent>,
                    TryRead<DimensionsComponent>,
                    TryRead<Translation>,
                    Read<Sprite>,
                )>::query(
                )
                .filter(!tag::<SpriteLayer>() & !component::<Destroy>() & !tag::<StaticSpriteTag>())
                .iter_entities(world)
                {
                    if let Some(position) = position.as_ref() {
                        if !aabb.contains(***position) {
                            continue;
                        }
                    }

                    do_entity(
                        entity,
                        position,
                        dimensions,
                        translation,
                        &sprite,
                        SpriteLayer::None,
                    );
                }
            })?;

        if grow {
            self.dynamic_entity_buffer.grow(
                self.dynamic_entity_buffer
                    .get(frame_number)
                    .buffer_info
                    .size as usize
                    * 2,
            )?;
        }

        self.dynamic_entity_buffer
            .get_mut(frame_number)
            .flush(None)?;

        Ok(count)
    }
}
impl VkPass for EntitiesPass {
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

        let vk = &args.render_context.vk;

        self.dynamic_entity_buffer.maintain()?;

        let map = <Read<Map>>::fetch(&args.state.resources);

        let camera = (self.camera_query)(&args.state.world);

        self.props_buffer.write(
            0,
            &[SpriteProperties {
                sheet_dimensions: std140::uvec2(
                    self.sprite_sheet.image_info.extent.width,
                    self.sprite_sheet.image_info.extent.height,
                ),
                map_dimensions: std140::uvec3(
                    map.dimensions().x as u32,
                    map.dimensions().y as u32,
                    map.dimensions().z as u32,
                ),
                sprite_dimensions: std140::uvec2(16, 24),
                view_proj: camera
                    .camera
                    .matrix(&camera.translation, camera.scale)
                    .as_slice()
                    .into(),
                camera_translation: camera.translation.as_slice().into(),
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

        // Draw static first
        let _ = self.update_static_entities(
            &args.state.world,
            &args.state.resources,
            &camera,
            args.current_frame,
        )?;

        vk.device.cmd_bind_vertex_buffers(
            args.command_buffer,
            0,
            std::slice::from_ref(&self.static_entity_buffer.buffer),
            &[0],
        );

        vk.device.cmd_draw(
            args.command_buffer,
            4,
            self.static_entity_count as u32,
            0,
            0,
        );

        // Then dynamic

        let dynamic_entity_count = self.build_dynamic_entity_buffer(
            &args.state.world,
            &args.state.resources,
            &camera,
            args.current_frame,
        )?;
        if dynamic_entity_count > 0 {
            vk.device.cmd_bind_vertex_buffers(
                args.command_buffer,
                0,
                std::slice::from_ref(&self.dynamic_entity_buffer.get(args.current_frame).buffer),
                &[0],
            );

            vk.device
                .cmd_draw(args.command_buffer, 4, dynamic_entity_count as u32, 0, 0);
        }

        Ok(())
    }
}
