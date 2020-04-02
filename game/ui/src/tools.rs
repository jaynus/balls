#![allow(unused_variables)]
use crate::{
    imgui::{self, im_str, Condition},
    selection::{SelectionCategory, SelectionMode, SelectionState},
    UiWindowSet,
};
use enumflags2::BitFlags;
use rl_ai::{HasTasksComponent, Task, TaskKind};
use rl_core::defs::{
    item::{ItemKind, StockpileTileChildComponent},
    reaction::ReactionDefinition,
    workshop::{WorkshopComponent, WorkshopDefinition},
    Definition, DefinitionComponent, DefinitionStorage,
};
use rl_core::{
    components::{EntityMeta, PositionComponent, VirtualTaskTag},
    data::{Target, TargetPosition},
    event::Channel,
    input::{ActionBinding, DesignateAction, InputActionEvent},
    legion::prelude::*,
    map::{
        spatial::{SpatialMap, StaticSpatialMap},
        Map,
    },
    math::Vec3i,
    settings::Settings,
    time::Time,
    GameStateRef, Logging, ScreenDimensions,
};
use rl_reaction::ReactionExecution;
use rl_render_pod::{
    color::Color,
    sprite::{sprite_map, SparseSpriteArray, Sprite},
};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ActiveTool {
    None,
    Designate,
    Stockpile,
    Construct,
}
impl Default for ActiveTool {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Default)]
struct ToolOverlayState {
    active_designation: Option<DesignateAction>,
    active_tool: ActiveTool,
    show_selected: bool,
}

#[derive(Default)]
pub struct ActiveToolProperties {
    designate_stockpile: BitFlags<ItemKind>,
}

#[allow(clippy::too_many_lines, clippy::comparison_chain)]
pub fn build_tools_overlay(
    world: &mut World,
    resources: &mut Resources,
) -> Result<(), anyhow::Error> {
    let mut state = ToolOverlayState::default();

    let listener_id = resources
        .get_mut::<Channel<InputActionEvent>>()
        .unwrap()
        .bind_listener(64);

    resources.insert(ActiveToolProperties::default());

    UiWindowSet::create_with(
        world,
        resources,
        "toolOverlay",
        true,
        move |ui, _window_manager, world, resources, command_buffer| {
            let (_log, channel, dimensions, active_tools) = <(
                Read<Logging>,
                Read<Channel<InputActionEvent>>,
                Read<ScreenDimensions>,
                Read<ActiveToolProperties>,
            )>::fetch(resources);

            let mut got_event = false;
            while let Some(event) = channel.read(listener_id) {
                match event {
                    InputActionEvent::Pressed(ActionBinding::DoAction)
                    | InputActionEvent::Released(ActionBinding::Selection) => got_event = true,
                    _ => {}
                }
            }
            // Clear the state and remove all selections/popups if the user ever clicks outside the enu
            if got_event {
                if let Some(action) = &state.active_designation {
                    let (selection_state, map, static_spatial_map, spatial_map, reaction_defs) =
                        <(
                            Read<SelectionState>,
                            Read<Map>,
                            Read<StaticSpatialMap>,
                            Read<SpatialMap>,
                            Read<DefinitionStorage<ReactionDefinition>>,
                        )>::fetch(&resources);

                    if let Some(selection) = selection_state.last_selection.as_ref() {
                        match *action {
                            DesignateAction::Stockpile => {
                                spawn_stockpile(
                                    world,
                                    resources,
                                    &map,
                                    selection.tile_area.iter(),
                                    active_tools.designate_stockpile,
                                    command_buffer,
                                )
                                .unwrap();
                            }
                            DesignateAction::ChopTree => {
                                spawn_tasks_on_entity(
                                    world,
                                    resources,
                                    "Chop Tree",
                                    selection.tile_area.iter().flat_map(|coord| {
                                        static_spatial_map
                                            .locate_all_at_point(&PositionComponent::from(coord))
                                            .filter_map(|entry| {
                                                world
                                                    .get_component::<PositionComponent>(
                                                        entry.entity,
                                                    )
                                                    .map(|position| (entry.entity, **position))
                                            })
                                    }),
                                    command_buffer,
                                    &reaction_defs,
                                )
                                .unwrap();
                            }
                            DesignateAction::Channel => {
                                spawn_virtual_tasks(
                                    world,
                                    resources,
                                    &map,
                                    "Channel",
                                    selection.tile_area.iter().filter(|coord| {
                                        spatial_map
                                            .locate_all_at_point(&PositionComponent::from(*coord))
                                            .find(|entry| {
                                                world
                                                    .get_tag::<VirtualTaskTag>(entry.entity)
                                                    .is_some()
                                            })
                                            .is_none()
                                    }),
                                    command_buffer,
                                    &reaction_defs,
                                )
                                .unwrap();
                            }
                            DesignateAction::Dig => {
                                spawn_virtual_tasks(
                                    world,
                                    resources,
                                    &map,
                                    "Dig",
                                    selection.tile_area.iter().filter(|coord| {
                                        spatial_map
                                            .locate_all_at_point(&PositionComponent::from(*coord))
                                            .find(|entry| {
                                                world
                                                    .get_tag::<VirtualTaskTag>(entry.entity)
                                                    .is_some()
                                            })
                                            .is_none()
                                    }),
                                    command_buffer,
                                    &reaction_defs,
                                )
                                .unwrap();
                            }
                        }
                    }

                    state = ToolOverlayState::default();
                    resources.get_mut::<SelectionState>().unwrap().mode =
                        SelectionMode::EntityWorldBox;
                }
            }

            imgui::Window::new(im_str!("toolOverlay"))
                .bg_alpha(0.35)
                .movable(false)
                .no_decoration()
                .always_auto_resize(false)
                .save_settings(false)
                .focus_on_appearing(false)
                .position(
                    [0.0, dimensions.size.height as f32 - 50.0],
                    Condition::Always,
                )
                .size([dimensions.size.width as f32, 50.0], Condition::Always)
                .no_nav()
                .opened(&mut true)
                .build(ui, || {
                    ui.same_line(0.0);
                    if ui.button(im_str!("Selected"), [0.0, 0.0]) {
                        state.show_selected ^= true;
                    }
                    ui.same_line(0.0);
                    if ui.button(im_str!("Designate"), [0.0, 0.0]) {
                        if state.active_tool == ActiveTool::Designate {
                            state.active_tool = ActiveTool::None;
                        } else {
                            state.active_tool = ActiveTool::Designate;
                        }
                    }
                    ui.same_line(0.0);
                    if ui.button(im_str!("Stockpile"), [0.0, 0.0]) {
                        if state.active_tool == ActiveTool::Stockpile {
                            state.active_tool = ActiveTool::None;
                        } else {
                            state.active_tool = ActiveTool::Stockpile;
                        }
                    }
                    ui.same_line(0.0);
                    if ui.button(im_str!("Construct"), [0.0, 0.0]) {
                        if state.active_tool == ActiveTool::Construct {
                            state.active_tool = ActiveTool::None;
                        } else {
                            state.active_tool = ActiveTool::Construct;
                        }
                    }

                    if state.active_tool == ActiveTool::Construct {
                        imgui::Window::new(im_str!("toolConstruct"))
                            .bg_alpha(0.35)
                            .movable(false)
                            .no_decoration()
                            .always_auto_resize(false)
                            .save_settings(false)
                            .focus_on_appearing(false)
                            .position(
                                [0.0, dimensions.size.height as f32 - 150.0],
                                Condition::Always,
                            )
                            .size([dimensions.size.width as f32, 100.0], Condition::Always)
                            .no_nav()
                            .opened(&mut true)
                            .build(ui, || {
                                ui.text("HI");
                            });
                    }

                    if state.active_tool == ActiveTool::Stockpile {
                        imgui::Window::new(im_str!("toolStockpile"))
                            .bg_alpha(0.35)
                            .movable(false)
                            .no_decoration()
                            .always_auto_resize(false)
                            .save_settings(false)
                            .focus_on_appearing(false)
                            .position(
                                [0.0, dimensions.size.height as f32 - 150.0],
                                Condition::Always,
                            )
                            .size([dimensions.size.width as f32, 100.0], Condition::Always)
                            .no_nav()
                            .opened(&mut true)
                            .build(ui, || {
                                if ui.button(im_str!("Wood"), [0.0, 0.0]) {
                                    resources
                                        .get_mut::<ActiveToolProperties>()
                                        .unwrap()
                                        .designate_stockpile = ItemKind::Wood.into();

                                    resources.get_mut::<SelectionState>().unwrap().mode =
                                        SelectionMode::MapTileBox;
                                    state.active_designation = Some(DesignateAction::Stockpile);
                                }
                                if ui.button(im_str!("Anything"), [0.0, 0.0]) {
                                    resources
                                        .get_mut::<ActiveToolProperties>()
                                        .unwrap()
                                        .designate_stockpile = BitFlags::all();

                                    resources.get_mut::<SelectionState>().unwrap().mode =
                                        SelectionMode::MapTileBox;
                                    state.active_designation = Some(DesignateAction::Stockpile);
                                }
                            });
                    }

                    if state.active_tool == ActiveTool::Designate {
                        imgui::Window::new(im_str!("toolDesignate"))
                            .bg_alpha(0.35)
                            .movable(false)
                            .no_decoration()
                            .always_auto_resize(false)
                            .save_settings(false)
                            .focus_on_appearing(false)
                            .position(
                                [0.0, dimensions.size.height as f32 - 150.0],
                                Condition::Always,
                            )
                            .size([dimensions.size.width as f32, 100.0], Condition::Always)
                            .no_nav()
                            .opened(&mut true)
                            .build(ui, || {
                                if ui.button(im_str!("Channel"), [0.0, 0.0]) {
                                    resources.get_mut::<SelectionState>().unwrap().mode =
                                        SelectionMode::MapTileBox;
                                    state.active_designation = Some(DesignateAction::Channel);
                                }
                                if ui.button(im_str!("Dig"), [0.0, 0.0]) {
                                    resources.get_mut::<SelectionState>().unwrap().mode =
                                        SelectionMode::MapTileBox;
                                    state.active_designation = Some(DesignateAction::Dig);
                                }
                                if ui.button(im_str!("Chop Tree"), [0.0, 0.0]) {
                                    resources.get_mut::<SelectionState>().unwrap().mode =
                                        SelectionMode::MapTileBox;
                                    state.active_designation = Some(DesignateAction::ChopTree);
                                }
                                if ui.button(im_str!("Harvest"), [0.0, 0.0]) {}
                            });
                    }
                });

            //if state.show_selected {
            imgui::Window::new(im_str!("Selected"))
                .bg_alpha(0.35)
                .movable(true)
                .always_auto_resize(false)
                .save_settings(false)
                .focus_on_appearing(true)
                .position(
                    [0.0, dimensions.size.height as f32 - 450.0],
                    Condition::Appearing,
                )
                .size([400.0, 400.0], Condition::Appearing)
                .opened(&mut state.show_selected)
                .build(ui, || {
                    use rl_core::components::{CarryComponent, NameComponent};
                    use rl_core::defs::body::BodyComponent;
                    let selection_state = resources.get::<SelectionState>().unwrap();

                    if let Some(selection) = &selection_state.last_selection {
                        if selection.entities.len() == 1 {
                            let entity = selection.entities[0];

                            if selection.category == SelectionCategory::Pawn {
                                let name = world.get_component::<NameComponent>(entity).unwrap();
                                let position =
                                    world.get_component::<PositionComponent>(entity).unwrap();
                                let body_comp =
                                    world.get_component::<BodyComponent>(entity).unwrap();
                                let carrying =
                                    world.get_component::<CarryComponent>(entity).unwrap();

                                ui.text(&name.name);
                                ui.text_wrapped(&imgui::ImString::from(format!(
                                    "Body Traits: {:?}",
                                    body_comp.flags
                                )));
                                ui.text(&format!(
                                    "Carrying: \n\t{:?}\n\t{:?}",
                                    carrying.limbs[0].1, carrying.limbs[1].1,
                                ));
                            } else if let Some(workshop_comp) =
                                world.get_component::<WorkshopComponent>(entity)
                            {
                                let workshops = resources
                                    .get::<DefinitionStorage<WorkshopDefinition>>()
                                    .unwrap();

                                let reactions = resources
                                    .get::<DefinitionStorage<ReactionDefinition>>()
                                    .unwrap();

                                let workshop_def = workshop_comp.fetch(&workshops);

                                let tasks = unsafe {
                                    world
                                        .get_component_mut_unchecked::<HasTasksComponent>(entity)
                                        .unwrap()
                                };

                                ui.columns(2, im_str!("tasks"), true);
                                ui.text("Queued");
                                ui.next_column();
                                ui.text("Available");
                                ui.next_column();

                                ui.text(&workshop_def.name());
                                ui.separator();

                                for (n, (handle, task)) in
                                    tasks.storage.get().iter_all().enumerate()
                                {
                                    imgui::Selectable::new(&imgui::ImString::new(format!(
                                        "{}##{}",
                                        &task.reaction.fetch(&reactions).name(),
                                        n
                                    )))
                                    .build(ui);
                                }
                                ui.next_column();

                                let spawn_task = |reaction_id| {
                                    tasks.storage.get_mut().insert(Task::new(
                                        5,
                                        TaskKind::WOODCUTTING,
                                        reaction_id,
                                    ));
                                };

                                for reaction in &workshop_def.reactions {
                                    if imgui::Selectable::new(&imgui::ImString::new(
                                        &reaction.fetch(&reactions).unwrap().name().to_owned(),
                                    ))
                                    .build(ui)
                                    {
                                        spawn_task(reaction.id());
                                    }
                                }
                            }
                        }
                    }
                });
            //}
            true
        },
    );

    Ok(())
}

// TODO: BOTH OF THESE SHOULD USE THE CORRECT TASK KIND

pub fn spawn_stockpile(
    world: &World,
    resources: &Resources,
    map: &Map,
    selection_area: impl Iterator<Item = Vec3i>,
    stores: BitFlags<ItemKind>,
    command_buffer: &mut CommandBuffer,
) -> Result<(), anyhow::Error> {
    use rl_core::defs::item::StockpileComponent;
    use rl_core::map::tile::TileKind;
    use rl_render_pod::sprite::SpriteLayer;

    let tiles = selection_area
        .filter(|coord| {
            let tile = map.get(*coord);
            tile.kind == TileKind::Floor
        })
        .collect::<Vec<_>>();

    let stockpile_entity = command_buffer.insert(
        (SpriteLayer::Ground,),
        vec![(
            EntityMeta::new(resources.get::<Time>().unwrap().stamp()),
            StockpileComponent::new(stores, tiles.clone()),
            HasTasksComponent::default(),
        )],
    )[0];

    // Spawn all the children of this stockpile
    let color = resources.get::<Settings>().unwrap().palette().stockpile;
    let stamp = resources.get::<Time>().unwrap().stamp();
    let children = command_buffer.insert(
        (SpriteLayer::Ground,),
        tiles.into_iter().map(move |coord| {
            (
                EntityMeta::new(stamp),
                PositionComponent::from(coord),
                StockpileTileChildComponent {
                    parent: stockpile_entity,
                },
                Sprite::new(sprite_map::FLOOR, color),
            )
        }),
    );

    let children = children.to_vec();
    command_buffer.exec_mut(move |world| {
        world
            .get_component_mut::<StockpileComponent>(stockpile_entity)
            .unwrap()
            .children_tiles = children.clone()
    });

    Ok(())
}

pub fn spawn_tasks_on_entity(
    world: &World,
    resources: &Resources,
    reaction_name: &str,
    entities: impl Iterator<Item = (Entity, Vec3i)>,
    command_buffer: &mut CommandBuffer,
    reaction_defs: &DefinitionStorage<ReactionDefinition>,
) -> Result<(), anyhow::Error> {
    use std::iter::FromIterator;

    // Lets just add mining "task" entities to the tile, with a visual ?
    let def = reaction_defs.get_by_name(reaction_name).unwrap();

    entities.for_each(|(entity, coord)| {
        if def
            .check_designate(
                GameStateRef { world, resources },
                Target::None,
                Target::Entity(entity),
            )
            .is_ok()
        {
            if let Some(tasks) = world.get_component::<HasTasksComponent>(entity) {
                tasks
                    .storage
                    .get_mut()
                    .insert(Task::new(5, TaskKind::MINING, def.id()));
            } else {
                command_buffer.add_component(
                    entity,
                    HasTasksComponent::from_iter(vec![Task::new(5, TaskKind::MINING, def.id())]),
                );
            }
        }
    });

    Ok(())
}

pub fn spawn_virtual_tasks(
    world: &World,
    resources: &Resources,
    map: &Map,
    reaction_name: &str,
    selection_area: impl Iterator<Item = Vec3i>,
    command_buffer: &mut CommandBuffer,
    reaction_defs: &DefinitionStorage<ReactionDefinition>,
) -> Result<(), anyhow::Error> {
    // Lets just add mining "task" entities to the tile, with a visual ?
    let def = reaction_defs.get_by_name(reaction_name).unwrap();

    let color = resources
        .get::<Settings>()
        .unwrap()
        .palette()
        .task_designation;

    selection_area.for_each(|coord| {
        let tile = map.get(coord);
        if def
            .check_designate(
                GameStateRef { world, resources },
                Target::None,
                Target::Position(TargetPosition::Tile(coord)),
            )
            .is_ok()
        {
            use std::iter::FromIterator;
            command_buffer.insert(
                (VirtualTaskTag,),
                vec![(
                    PositionComponent::new(coord),
                    EntityMeta::new(resources.get::<Time>().unwrap().stamp()),
                    HasTasksComponent::from_iter(vec![Task::new(5, TaskKind::MINING, def.id())]),
                    Sprite::new(sprite_map::FLOOR, color & Color::a(0.5)),
                    SparseSpriteArray::default(),
                )],
            );
        }
    });

    Ok(())
}
