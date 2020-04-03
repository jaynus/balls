#![allow(clippy::cast_possible_truncation, clippy::too_many_lines)]

use crate::{spawners::Spawnable, weather::Weather};
use rl_core::{
    camera::make_camera_query,
    components::*,
    data::{SpawnArguments, Target, TargetPosition},
    defs::{
        item::{ItemComponent, ItemDefinition},
        material::{MaterialComponent, MaterialDefinition, MaterialDefinitionId, MaterialState},
        workshop::{WorkshopComponent, WorkshopDefinition},
        Definition, DefinitionComponent, DefinitionStorage,
    },
    image,
    input::InputState,
    legion::prelude::*,
    map::{
        spatial::{SpatialMap, SpatialMapEntry, StaticSpatialMap},
        Map,
    },
    math::{Vec2, Vec3i},
    settings::Settings,
    time::Time,
    GlobalCommandBuffer, NamedSlotMap,
};
use rl_render_pod::sprite::Sprite;
use rl_render_vk::{
    alloc,
    ash::{version::DeviceV1_0, vk},
    data::texture::{Texture, TextureHandle, TexturePtr},
    RenderContext,
};
use rl_ui::{
    imgui::{self, im_str},
    selection::SelectionState,
    ImguiContextLock, UiWindowSet,
};
use std::sync::Arc;

pub mod ai;
pub mod iaus;
pub mod palette;
pub mod perf;

#[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
pub fn build_sprite_selector(
    world: &mut World,
    resources: &mut Resources,
) -> Result<(), anyhow::Error> {
    struct SpriteSelectorState {
        open: bool,
    }

    let path = std::path::Path::new("assets/cp437.png");
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

    resources.insert(SpriteSelectorState { open: true });

    let texture = texture;
    UiWindowSet::create_with(
        world,
        resources,
        "sprite_selector",
        false,
        move |ui, _window_manager, _world, resources, _buffer| {
            let (mut window_state, input_state, map, textures) = unsafe {
                <(
                    Write<SpriteSelectorState>,
                    Read<InputState>,
                    Read<Map>,
                    Read<NamedSlotMap<TextureHandle, TexturePtr>>,
                )>::fetch_unchecked(&resources)
            };

            if window_state.open {
                let texture = texture.clone();
                imgui::Window::new(im_str!("sprite_selector##UI"))
                    .position([0.0, 0.0], imgui::Condition::FirstUseEver)
                    .size([500.0, 500.0], imgui::Condition::FirstUseEver)
                    .opened(&mut window_state.open)
                    .build(ui, move || {
                        let sprite_dimensions = Vec2::new(
                            map.sprite_dimensions.x as f32,
                            map.sprite_dimensions.y as f32,
                        );

                        let (image_dimensions, sheet_dimensions) = {
                            let texture = textures.get_by_name("sprite_sheet").unwrap();

                            (
                                [
                                    texture.image_info.extent.width as f32,
                                    texture.image_info.extent.height as f32,
                                ],
                                Vec2::new(
                                    texture.image_info.extent.width as f32,
                                    texture.image_info.extent.height as f32,
                                ) / sprite_dimensions,
                            )
                        };

                        let _local_texture = texture.clone();
                        let cursor_screen_pos: Vec2 = ui.cursor_screen_pos().into();
                        imgui::Image::new(texture_id, image_dimensions).build(ui);

                        let window_mouse_pos = input_state.mouse_position - cursor_screen_pos;

                        if window_mouse_pos.x > 0.0 && window_mouse_pos.y > 0.0 {
                            ui.text(&format!(
                                "mouse_pos = {:.2}, {:.2}",
                                window_mouse_pos.x, window_mouse_pos.y
                            ));

                            let tile_pos = window_mouse_pos / sprite_dimensions;
                            let tile_index =
                                (tile_pos.y.floor() * sheet_dimensions.y) + tile_pos.x.floor();

                            ui.text(&format!("tile_index = {}", tile_index as i32));
                            ui.text(&format!("tile_pos ={:.2}, {:.2}", tile_pos.x, tile_pos.y));
                            ui.text(&format!(
                                "sheet_dimensions = {:.2}, {:.2}",
                                sprite_dimensions.x, sprite_dimensions.y
                            ));
                        } else {
                            ui.text("tile_index = N/A");
                            ui.text("tile_pos = N/A");
                            ui.text(&format!(
                                "sheet_dimensions = {:.2}, {:.2}",
                                sprite_dimensions.x, sprite_dimensions.y
                            ));
                        }
                    });
            }

            let visible = window_state.open;
            window_state.open = true;
            visible
        },
    );

    Ok(())
}

#[allow(clippy::similar_names)]
pub fn build_debug_overlay(
    world: &mut World,
    resources: &mut Resources,
) -> Result<(), anyhow::Error> {
    #[derive(Default)]
    struct DebugMenuState {
        pub selected_item: usize,
        pub selected_material: usize,
        pub selected_workshop: usize,
    };

    iaus::build(world, resources);
    palette::build(world, resources)?;
    perf::build(world, resources)?;

    build_pickup_debug(world, resources)?;

    let mut camera_query = make_camera_query();
    let mut window_state = DebugMenuState::default();

    UiWindowSet::create_with(
        world,
        resources,
        "debugMenu",
        true,
        move |ui, window_manager, world, resources, _buffer| {
            let placement_color = resources.get::<Settings>().unwrap().palette().placement;

            let selected_item = window_state.selected_item;
            let selected_material = window_state.selected_material;

            imgui::Window::new(im_str!("debugMenu"))
                .size([150.0, 300.0], imgui::Condition::FirstUseEver)
                .build(ui, || {
                    if imgui::CollapsingHeader::new(im_str!("Save/Load"))
                        .default_open(true)
                        .build(ui)
                    {
                        ui.text("Filler");
                        if ui.button(im_str!("Save"), [0.0, 0.0]) {
                            //let exported_world = crate::saveload::clone_world(world, resources);
                            //let serialized = crate::saveload::serialize_world(world).unwrap();
                            //println!("{}", serialized);
                        }
                    }

                    if imgui::CollapsingHeader::new( im_str!("Spawn Item"))
                        .default_open(true)
                        .build(ui)
                    {
                        let materials = {
                            resources
                                .get::<DefinitionStorage<MaterialDefinition>>()
                                .unwrap()
                                .keys()
                                .map(|name| imgui::ImString::from(name.clone()))
                                .collect::<Vec<_>>()
                        };

                        imgui::ComboBox::new(im_str!("Material##Selection")).build_simple_string(
                            ui,
                            &mut window_state.selected_material,
                            materials
                                .iter()
                                .filter(|_| true)
                                .collect::<Vec<_>>()
                                .as_slice(),
                        );

                        let items = {
                            resources
                                .get::<DefinitionStorage<ItemDefinition>>()
                                .unwrap()
                                .keys()
                                .map(|name| imgui::ImString::from(name.clone()))
                                .collect::<Vec<_>>()
                        };

                        imgui::ComboBox::new(im_str!("Item##Selection")).build_simple_string(
                            ui,
                            &mut window_state.selected_item,
                            items.iter().collect::<Vec<_>>().as_slice(),
                        );
                        if ui.button(im_str!("Spawn##Item"), [0.0, 0.0]) {
                            resources
                                .get_mut::<InputState>()
                                .unwrap()
                                .swap_placement_request(
                                    world,
                                    resources,
                                    rl_core::input::PlacementRequestImpl::new(
                                        move |_, _| Sprite {
                                            sprite_number: 7,
                                            color: placement_color,
                                            ..Default::default()
                                        },
                                        move |_world, resources, _, coord| {
                                            let (material_defs, item_defs, mut command_buffer) = unsafe {
                                                <(
                                                    Read<DefinitionStorage<MaterialDefinition>>,
                                                    Read<DefinitionStorage<ItemDefinition>>,
                                                    Write<GlobalCommandBuffer>,
                                                )>::fetch_unchecked(
                                                    resources
                                                )
                                            };

                                            item_defs
                                                .get_by_name(
                                                    item_defs.keys().nth(selected_item).unwrap(),
                                                )
                                                .unwrap()
                                                .spawn(
                                                    resources,
                                                    &mut command_buffer,
                                                    Target::Position(TargetPosition::Tile(
                                                        coord,
                                                    )),
                                                    &SpawnArguments::Item {
                                                        material: MaterialComponent::new(
                                                                material_defs.get_by_name(material_defs.keys().nth(selected_material).unwrap(), ).unwrap().id(),
                                                                MaterialState::Solid)
                                                    },
                                                ).unwrap();
                                        },
                                        |_, _| println!("PLACEMENT CANCELED"),
                                    ),
                                );
                        }
                    }
                    if imgui::CollapsingHeader::new(im_str!("Spawn Workshop"))
                        .default_open(true)
                        .build(ui)
                    {
                        let workshops = {
                            resources
                                .get::<DefinitionStorage<WorkshopDefinition>>()
                                .unwrap()
                                .keys()
                                .map(|name| imgui::ImString::from(name.clone()))
                                .collect::<Vec<_>>()
                        };

                        imgui::ComboBox::new(im_str!("Workshop##Selection")).build_simple_string(
                            ui,
                            &mut window_state.selected_workshop,
                            workshops.iter().collect::<Vec<_>>().as_slice(),
                        );
                        if ui.button(im_str!("Spawn##Workshop"), [0.0, 0.0]) {
                            let selected_workshop = window_state.selected_workshop;
                            resources
                                .get_mut::<InputState>()
                                .unwrap()
                                .swap_placement_request(
                                    world,
                                    resources,
                                    rl_core::input::PlacementRequestImpl::new(
                                        move |_, _| Sprite {
                                            sprite_number: 7,
                                            color: placement_color,
                                            ..Default::default()
                                        },
                                        move |_world, resources, _, coord| {
                                            let (material_defs, def_storage, mut command_buffer) = unsafe {
                                                <(
                                                    Read<DefinitionStorage<MaterialDefinition>>,
                                                    Read<DefinitionStorage<WorkshopDefinition>>,
                                                    Write<GlobalCommandBuffer>,
                                                )>::fetch_unchecked(
                                                    resources
                                                )
                                            };
                                            let name =
                                                def_storage.keys().nth(selected_workshop).unwrap();

                                            let _ = def_storage.get_by_name(name).unwrap().spawn(
                                                resources,
                                                &mut command_buffer,
                                                Target::Position(TargetPosition::Tile(coord)),
                                                &SpawnArguments::Workshop { material: MaterialComponent::new(material_defs.get_by_name(
                                                    material_defs.keys().nth(selected_material).unwrap(),
                                                ).unwrap().id(), MaterialState::Solid) },
                                            );
                                        },
                                        |_, _| println!("PLACEMENT CANCELED"),
                                    ),
                                );
                        }
                    }
                    if ui.button(im_str!("Pickup Viewer"), [0.0, 0.0]) {
                        window_manager.show("pickup_viewer");
                    }
                    if ui.button(im_str!("Perf Viewer"), [0.0, 0.0]) {
                        window_manager.show("perf_viewer");
                    }
                    if ui.button(im_str!("Open IAUS"), [0.0, 0.0]) {
                        window_manager.show("iaus_editor");
                    }
                    if ui.button(im_str!("Open Sprite Selector"), [0.0, 0.0]) {
                        window_manager.show("sprite_selector");
                    }
                    if ui.button(im_str!("Palette  Editor"), [0.0, 0.0]) {
                        window_manager.show("palette_editor");
                    }
                });

            true
        },
    );

    UiWindowSet::create_with(
        world,
        resources,
        "debugOverlay",
        true,
        move |ui, _window_manager, world, resources, _buffer| {
            imgui::Window::new(im_str!("debugOverlay"))
                .bg_alpha(0.35)
                .movable(false)
                .no_decoration()
                .always_auto_resize(false)
                .save_settings(false)
                .focus_on_appearing(false)
                .scroll_bar(true)
                .position([0.0, 50.0], imgui::Condition::Always)
                .size([0.0, 400.0], imgui::Condition::Always)
                .no_nav()
                .opened(&mut true)
                .build(ui, || {
                    let (
                        map,
                        spatial_map,
                        static_spatial_map,
                        input_state,
                        selection_state,
                        material_defs,
                        item_defs,
                        workshop_defs,
                        mut time,
                        mut weather,
                    ) = unsafe {
                        <(
                            Read<Map>,
                            Read<SpatialMap>,
                            Read<StaticSpatialMap>,
                            Read<InputState>,
                            Read<SelectionState>,
                            Read<DefinitionStorage<MaterialDefinition>>,
                            Read<DefinitionStorage<ItemDefinition>>,
                            Read<DefinitionStorage<WorkshopDefinition>>,
                            Write<Time>,
                            Write<Weather>,
                        )>::fetch_unchecked(&resources)
                    };

                    let camera = camera_query(world);

                    ui.text(&format!("FPS: {:.2}", time.current_fps));
                    ui.text(&format!("Real Time: {:.2}", time.real_time));
                    ui.text(&format!("World Time: {:.2}", time.world_time));
                    imgui::Slider::new(
                        im_str!("World Speed"),
                        std::ops::RangeInclusive::new(0.0, 20.0),
                    )
                    .build(ui, &mut time.world_speed);

                    if imgui::CollapsingHeader::new(im_str!("Weather"))
                        .default_open(true)
                        .build(ui)
                    {
                        imgui::Slider::new(
                            im_str!("rain_frequency"),
                            std::ops::RangeInclusive::new(0.0, 1000.0),
                        )
                        .build(ui, &mut weather.rain_frequency);
                    }

                    ui.text(&format!(
                        "Mouse Screen Pos: {:.2}, {:.2}",
                        input_state.mouse_position.x, input_state.mouse_position.y
                    ));
                    ui.text(&format!(
                        "Mouse World Pos: {:.2}, {:.2}",
                        input_state.mouse_world_position.x, input_state.mouse_world_position.y
                    ));
                    ui.text(&format!(
                        "Mouse Tile Pos: {}, {}, {}",
                        input_state.mouse_tile_position.x,
                        input_state.mouse_tile_position.y,
                        input_state.mouse_tile_position.z
                    ));

                    if map.get(input_state.mouse_tile_position).is_empty() {
                        ui.text("Tile Material: N/A");
                    } else {
                        let mat_id: MaterialDefinitionId =
                            map.get(input_state.mouse_tile_position).material.into();
                        let material = mat_id.fetch(&material_defs);
                        ui.text(&format!("Tile Material: {}", material.name()));
                    }

                    if let Some(liquid) = map.get(input_state.mouse_tile_position).liquid.as_ref() {
                        ui.text(&format!("Tile Liquid: {:?}", liquid.depth));
                    } else {
                        ui.text("Tile Liquid: N/A");
                    }

                    ui.text(&format!(
                        "Heightmap Z: {}",
                        map.height_map
                            .get(map.encoder().encode(Vec3i::new(
                                input_state.mouse_tile_position.x,
                                input_state.mouse_tile_position.y,
                                0
                            )))
                            .unwrap()
                    ));
                    ui.text(&format!(
                        "Current Z: {}",
                        camera.translation.z.floor() as i32
                    ));
                    ui.text(&format!(
                        "Camera: ({:.2}, {:.2}), s={:.2}",
                        camera.translation.x, camera.translation.y, *camera.scale
                    ));

                    if imgui::CollapsingHeader::new(im_str!("Selection"))
                        .default_open(true)
                        .build(ui)
                    {
                        if let Some(selection) = &selection_state.last_selection {
                            if !selection.entities.is_empty() {
                                ui.text("Selections:");
                                ui.columns(2, im_str!("selections"), true);

                                for entity in &selection.entities {
                                    if let Some(name) =
                                        world.get_component::<NameComponent>(*entity)
                                    {
                                        ui.text(&format!("\tP: {}", name.name));
                                    } else if let Some(item) =
                                        world.get_component::<ItemComponent>(*entity)
                                    {
                                        ui.text(&format!("\tI: {}", item.fetch(&item_defs).name()));
                                        ui.text(&format!("\tActivePickup: {}", world.has_component::<ActivePickupComponent>(*entity)));
                                    } else if let Some(workshop) =
                                        world.get_component::<WorkshopComponent>(*entity)
                                    {
                                        ui.text(&format!(
                                            "\tW: {}",
                                            workshop.fetch(&workshop_defs).name()
                                        ))
                                    }

                                    ui.next_column();
                                    if let Some(material_comp) =
                                        world.get_component::<MaterialComponent>(*entity)
                                    {
                                        ui.text(material_comp.fetch_state(&material_defs).name());
                                    }
                                    ui.next_column();

                                    // If the entity has items, display them here as well
                                    if let Some(container) =
                                        world.get_component::<ItemContainerComponent>(*entity)
                                    {
                                        if !container.inside.is_empty() {
                                            for item in &container.inside {
                                                let item_comp = world
                                                    .get_component::<ItemComponent>(*item)
                                                    .unwrap();
                                                ui.text(&format!(
                                                    "\t\t{}",
                                                    item_comp.fetch(&item_defs).name()
                                                ));
                                                ui.next_column();
                                                ui.next_column();
                                            }
                                        }
                                    }
                                }
                                ui.columns(1, im_str!("selections"), false);
                            }
                        }
                    }
                    if imgui::CollapsingHeader::new(im_str!("Pawns"))
                        .default_open(true)
                        .build(ui)
                    {
                        let query = <(
                            Read<PositionComponent>,
                            TryRead<SpatialMapEntry>,
                            Read<NameComponent>,
                        )>::query();
                        for (position, spatial, name) in query.iter(world) {
                            ui.text(&name.name);
                            ui.text(&format!("\t{}, {}, {}", position.x, position.y, position.z));
                            if let Some(spatial) = spatial.as_ref() {
                                ui.text(&format!(
                                    "\t{}, {}, {}",
                                    spatial.position().x,
                                    spatial.position().y,
                                    spatial.position().z
                                ));
                            } else {
                                ui.text("NO SPATIAL!");
                            }
                        }
                    }

                    if imgui::CollapsingHeader::new(im_str!("Tile Inspector"))
                        .default_open(true)
                        .build(ui)
                    {
                        let tile = map.get(input_state.mouse_tile_position);

                        // if theres any entities under the mouse, list them here
                        ui.text("Entities: ");
                        (*spatial_map)
                            .locate_all_at_point(&input_state.mouse_tile_position.into())
                            .for_each(|entry| {
                                ui.text(&format!("\t{:?}", entry.entity));
                            });

                        ui.text("Static Entities: ");
                        (*static_spatial_map)
                            .locate_all_at_point(&input_state.mouse_tile_position.into())
                            .for_each(|entry| {
                                ui.text(&format!("\t{:?}", entry.entity));
                            });

                        ui.text(&format!("kind: {:?}", tile.kind));
                        ui.text(&format!("flag: {:?}", tile.flags));
                    }
                });

            true
        },
    );

    Ok(())
}

#[allow(clippy::cast_precision_loss, clippy::too_many_lines)]
pub fn build_pickup_debug(
    world: &mut World,
    resources: &mut Resources,
) -> Result<(), anyhow::Error> {
    struct PickupWindowState {
        open: bool,
    }

    resources.insert(PickupWindowState { open: true });

    let query = <Read<ActivePickupComponent>>::query();

    UiWindowSet::create_with(
        world,
        resources,
        "pickup_viewer",
        false,
        move |ui, _, world, resources, command_buffer| {
            let mut window_state =
                unsafe { <Write<PickupWindowState>>::fetch_unchecked(resources) };
            if window_state.open {
                imgui::Window::new(im_str!("Active Pickups##UI"))
                    .position([0.0, 0.0], imgui::Condition::FirstUseEver)
                    .size([300.0, 200.0], imgui::Condition::FirstUseEver)
                    .opened(&mut window_state.open)
                    .build(ui, || {
                        if ui.button(im_str!("DEBUG CLEAR"), [0.0, 0.0]) {
                            let clear = query
                                .iter_entities(world)
                                .map(|(e, _)| e)
                                .collect::<Vec<_>>();
                            clear.iter().for_each(|e| {
                                command_buffer.remove_component::<ActivePickupComponent>(*e)
                            });
                        }

                        for (entity, active) in query.iter_entities(world) {
                            ui.text(&format!("{:?}", entity));
                            ui.text(&format!("\t{:?}", active.initiator));
                        }
                    });
            }

            let visible = window_state.open;
            window_state.open = true;
            visible
        },
    );

    Ok(())
}
