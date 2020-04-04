use rl_ai::bt::{BehaviorRoot, BehaviorStorage, BehaviorTreeComponent};
use rl_core::defs::item::{ItemComponent, StockpileComponent, StockpileTileChildComponent};
use rl_core::{
    components::{
        BlackboardComponent, DimensionsComponent, EntityMeta, ItemContainerChildComponent, PawnTag,
        PositionComponent, SelectedComponent,
    },
    data::bt::PickupParameters,
    debug::DebugLines,
    event::Channel,
    fnv,
    fxhash::FxHashMap,
    input::{ActionBinding, InputActionEvent, InputState, InputStateKind},
    legion::prelude::*,
    map::{
        spatial::{SpatialMap, StaticSpatialMap},
        Map,
    },
    math::{
        geometry::{Aabb, Aabbi},
        Vec3, Vec3i,
    },
    rstar,
    settings::Settings,
    smallvec::SmallVec,
    time::Time,
};
use rl_render_pod::sprite::Sprite;
use std::iter::FromIterator;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionCategory {
    Pawn,
    Item,
    Stockpile,
    Designation,
    Multi,
}
impl Default for SelectionCategory {
    fn default() -> Self {
        Self::Multi
    }
}

#[derive(Default, Debug, Clone)]
pub struct Selection {
    pub world_area: Aabb,
    pub tile_area: Aabbi,

    pub entities: SmallVec<[Entity; 32]>,
    pub category: SelectionCategory,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum SelectionMode {
    FreeBox,
    TileBox,
    TileSingle,
}
impl Default for SelectionMode {
    fn default() -> Self {
        Self::FreeBox
    }
}

#[derive(Default)]
pub struct SelectionState {
    pub active_selection: Option<Selection>,
    pub last_selection: Option<Selection>,

    pub start_world_position: Vec3,
    pub start_tile_position: Vec3i,

    pub mode: SelectionMode,
    pub overlay_fn: Option<Box<dyn Fn() -> Sprite + Send + Sync>>,
}

#[allow(clippy::too_many_lines, clippy::map_clone)]
pub fn build_mouse_selection_system(
    _: &mut World,
    resources: &mut Resources,
) -> Box<dyn Schedulable> {
    resources.insert(SelectionState::default());

    let listener_id = resources
        .get_mut::<Channel<InputActionEvent>>()
        .unwrap()
        .bind_listener(64);

    let mut overlay_cache: FxHashMap<Vec3i, Entity> = FxHashMap::default();

    SystemBuilder::<()>::new("mouse_selection_system")
        .write_resource::<DebugLines>()
        .write_resource::<SelectionState>()
        .read_resource::<Time>()
        .read_resource::<InputState>()
        .read_resource::<Channel<InputActionEvent>>()
        .read_resource::<Map>()
        .read_resource::<SpatialMap>()
        .read_resource::<StaticSpatialMap>()
        .read_resource::<Settings>()
        .read_component::<StockpileTileChildComponent>()
        .read_component::<StockpileComponent>()
        .read_component::<PositionComponent>()
        .read_component::<DimensionsComponent>()
        .with_query(<(
            Read<SelectedComponent>,
            Read<PositionComponent>,
            Read<DimensionsComponent>,
        )>::query())
        .with_query(<Read<PositionComponent>>::query().filter(tag::<PawnTag>()))
        .build(
            move |command_buffer,
                  world,
                  (
                debug_lines,
                selection_state,
                time,
                input_state,
                action_channel,
                map,
                spatial_map,
                static_spatial_map,
                settings,
            ),
                  (selected_query, pawn_query)| {
                if input_state.state == InputStateKind::Selection {
                    while let Some(action) = action_channel.read(listener_id) {
                        match action {
                            InputActionEvent::Pressed(ActionBinding::Selection) => {
                                // Begin selection
                                selection_state.active_selection = Some(Selection::default());

                                selection_state.start_world_position =
                                    input_state.mouse_world_position;
                                selection_state.start_tile_position =
                                    input_state.mouse_tile_position;
                            }
                            InputActionEvent::Released(ActionBinding::Selection) => {
                                // End selection
                                selection_state.last_selection =
                                    selection_state.active_selection.take();

                                overlay_cache
                                    .drain()
                                    .for_each(|(_, v)| command_buffer.delete(v));

                                selected_query
                                    .iter_entities(world)
                                    .for_each(|(e, (_, _, _))| {
                                        command_buffer.remove_component::<SelectedComponent>(e);
                                    });

                                // Does the selection have a pawn? if so, we filter to only pawns
                                let pawns = pawn_query
                                    .iter_entities(&world)
                                    .map(|(e, _)| e)
                                    .collect::<Vec<Entity>>();

                                if let Some(last_selection) =
                                    selection_state.last_selection.as_mut()
                                {
                                    //println!("Selecting World: {:?}", last_selection.world_area);
                                    //println!("Selecting Tile: {:?}", last_selection.tile_area);

                                    // Filter cleaning up the entities based on alive
                                    last_selection.entities.retain(|e| world.is_alive(*e));

                                    // If there are pawns, filter selection on ONLY pawns.z
                                    // TODO: Same with designations

                                    if last_selection.entities.iter().any(|e| pawns.contains(e)) {
                                        // filter it
                                        last_selection.entities.retain(|e| pawns.contains(e));
                                        last_selection.category = SelectionCategory::Pawn;
                                    } else if last_selection.entities.len() == 1 {
                                        if let Some(child) = world
                                            .get_component::<StockpileTileChildComponent>(
                                                last_selection.entities[0],
                                            )
                                        {
                                            last_selection.entities.clear();
                                            last_selection.entities.push(child.parent);
                                            last_selection.category = SelectionCategory::Stockpile;
                                        }
                                    }

                                    // If the selection area was 1 tile wide, only select 1 thing
                                    // Prefer pawns, however.
                                    if last_selection.tile_area.volume() == 1
                                        && !last_selection.entities.is_empty()
                                    {
                                        let save = last_selection.entities[0];
                                        last_selection.entities.clear();
                                        last_selection.entities.push(save);
                                    }

                                    last_selection.entities.iter().for_each(|e| {
                                        command_buffer.add_component(*e, SelectedComponent);
                                    });
                                }
                            }
                            _ => {}
                        }
                    }

                    let start_world_position = selection_state.start_world_position;
                    let start_tile_position = selection_state.start_tile_position;

                    let mode = selection_state.mode;
                    if let Some(active_selection) = selection_state.active_selection.as_mut() {
                        let tile_area =
                            make_tile_aabb(start_tile_position, input_state.mouse_tile_position);

                        match mode {
                            SelectionMode::FreeBox => {
                                *active_selection = Selection {
                                    world_area: make_world_aabb(
                                        start_world_position,
                                        input_state.mouse_world_position,
                                    ),

                                    entities: {
                                        let aabb = rstar::AABB::from_corners(
                                            tile_area.min.into(),
                                            (tile_area.max - Vec3i::new(1, 1, 1)).into(),
                                        );
                                        let mut r = SmallVec::from_iter(
                                            spatial_map
                                                .locate_in_envelope_intersecting(&aabb)
                                                .map(|entry| entry.entity),
                                        );
                                        r.extend(
                                            static_spatial_map
                                                .locate_in_envelope_intersecting(&aabb)
                                                .map(|entry| entry.entity),
                                        );

                                        r.dedup();
                                        r
                                    },

                                    tile_area,
                                    category: SelectionCategory::Multi,
                                };

                                debug_lines.add_rectangle_2d(
                                    active_selection.world_area.min,
                                    active_selection.world_area.max,
                                    1.0,
                                    settings.palette().red,
                                );
                            }
                            SelectionMode::TileBox => {
                                *active_selection = Selection {
                                    world_area: make_world_aabb(
                                        start_world_position,
                                        input_state.mouse_world_position,
                                    ),
                                    entities: SmallVec::default(),
                                    tile_area,
                                    category: SelectionCategory::Multi,
                                };

                                debug_lines.add_rectangle_2d(
                                    map.tile_to_world(active_selection.tile_area.min),
                                    map.tile_to_world(active_selection.tile_area.max),
                                    1.0,
                                    settings.palette().red,
                                );
                            }
                            _ => unimplemented!(),
                        }
                    }

                    // Draw overlay sprites if applicable
                    if let Some(active_selection) = &selection_state.active_selection {
                        if let Some(overlay_fn) = &selection_state.overlay_fn {
                            active_selection.tile_area.iter().for_each(|tile| {
                                // Add the overlay sprite temporarily
                                overlay_cache.entry(tile).or_insert_with(|| {
                                    command_buffer.insert(
                                        (),
                                        vec![(EntityMeta::new(time.stamp()), (overlay_fn)())],
                                    )[0]
                                });
                            });
                            overlay_cache.retain(|k, v| {
                                if active_selection.tile_area.iter().any(|tile| *k == tile) {
                                    true
                                } else {
                                    command_buffer.delete(*v);

                                    false
                                }
                            });
                        }
                    }

                    if let Some(selection) = selection_state.last_selection.as_mut() {
                        if !selection.entities.is_empty() {
                            if selection.category == SelectionCategory::Stockpile {
                                // make an aabb around all the children of this stockpile
                                let stockpile = world
                                    .get_component::<StockpileComponent>(selection.entities[0])
                                    .unwrap();
                                let aabb = make_entities_aabb(stockpile.children_tiles.iter().map(
                                    |entity| {
                                        (
                                            *world
                                                .get_component::<PositionComponent>(*entity)
                                                .unwrap(),
                                            world
                                                .get_component::<DimensionsComponent>(*entity)
                                                .map(|v| *v),
                                        )
                                    },
                                ));
                                debug_lines.add_rectangle_2d(
                                    map.tile_to_world(aabb.min),
                                    map.tile_to_world(aabb.max),
                                    1.0,
                                    settings.palette().red,
                                );
                            } else {
                                // Draw selections
                                selected_query.iter_mut(world).for_each(
                                    |(_, position, dimensions)| {
                                        // Compute the min/max box around the entity

                                        let min = map.tile_to_world(**position);
                                        let max =
                                            map.tile_to_world(dimensions.aabb().max + **position);
                                        debug_lines.add_rectangle_2d(
                                            min,
                                            max,
                                            1.0,
                                            settings.palette().red,
                                        );
                                    },
                                );
                            }
                        }
                    }
                } else {
                    while let Some(_) = action_channel.read(listener_id) {}
                }
            },
        )
}

pub fn make_world_aabb(start: Vec3, end: Vec3) -> Aabb {
    Aabb {
        min: Vec3::new(start.x.min(end.x), start.y.min(end.y), start.z.min(end.z)),
        max: Vec3::new(
            start.x.max(end.x) + 1.0,
            start.y.max(end.y) + 1.0,
            start.z.max(end.z) + 1.0,
        ),
    }
}

pub fn make_tile_aabb(start: Vec3i, end: Vec3i) -> Aabbi {
    Aabbi {
        min: Vec3i::new(start.x.min(end.x), start.y.min(end.y), start.z.min(end.z)),
        max: Vec3i::new(
            start.x.max(end.x) + 1,
            start.y.max(end.y) + 1,
            start.z.max(end.z) + 1,
        ),
    }
}

pub fn make_entities_aabb(
    iter: impl Iterator<Item = (PositionComponent, Option<DimensionsComponent>)>,
) -> Aabbi {
    // Find the min and max coords
    let mut min = Vec3i::new(i32::max_value(), i32::max_value(), i32::max_value());
    let mut max = Vec3i::default();
    for (position, dimensions) in iter {
        if position.x < min.x {
            min.x = position.x;
        }
        if position.y < min.y {
            min.y = position.y;
        }

        let new_max = {
            if let Some(dimensions) = dimensions {
                *position + dimensions.as_tiles()
            } else {
                *position + Vec3i::new(1, 1, 0)
            }
        };

        if new_max.x > max.x {
            max.x = new_max.x;
        }
        if new_max.y > max.y {
            max.y = new_max.y;
        }
    }

    Aabbi { min, max }
}

#[allow(clippy::too_many_lines)]
pub fn build_mouse_action_system(
    world: &mut World,
    resources: &mut Resources,
) -> Box<dyn FnMut(&mut World, &mut Resources)> {
    resources.insert(SelectionState::default());

    let listener_id = resources
        .get_mut::<Channel<InputActionEvent>>()
        .unwrap()
        .bind_listener(64);

    let mut command_buffer = CommandBuffer::new(world);

    Box::new(move |world: &mut World, resources: &mut Resources| {
        {
            let (map, spatial_map, selection_state, input_state, input_action_channel) =
                <(
                    Read<Map>,
                    Read<SpatialMap>,
                    Read<SelectionState>,
                    Read<InputState>,
                    Read<Channel<InputActionEvent>>,
                )>::fetch(&resources);

            if input_state.state == InputStateKind::Selection {
                while let Some(action) = input_action_channel.read(listener_id) {
                    if let InputActionEvent::Released(ActionBinding::DoAction) = action {
                        // if we have pawns selected, do stuff
                        if let Some(selection_state) = selection_state.last_selection.as_ref() {
                            if selection_state.category == SelectionCategory::Pawn {
                                let target = spatial_map
                                    .locate_all_at_point(&PositionComponent::new(
                                        input_state.mouse_tile_position,
                                    ))
                                    .find_map(|e| {
                                        if world.has_component::<ItemComponent>(e.entity)
                                            && !world.has_component::<ItemContainerChildComponent>(
                                                e.entity,
                                            )
                                        {
                                            Some(e.entity)
                                        } else {
                                            None
                                        }
                                    });

                                if let Some(target) = target {
                                    // Only pickup with the first entity
                                    // TODO: stacks? LOL
                                    let pawn = selection_state.entities.iter().next().unwrap();

                                    world
                                        .get_component_mut::<BlackboardComponent>(*pawn)
                                        .unwrap()
                                        .insert(
                                            fnv!("PickupParameters"),
                                            PickupParameters {
                                                target,
                                                destination: None,
                                            },
                                        );

                                    let behavior_id = resources
                                        .get::<BehaviorStorage>()
                                        .unwrap()
                                        .get_handle("pickup_item")
                                        .unwrap();

                                    world
                                        .get_component_mut::<BehaviorTreeComponent>(*pawn)
                                        .unwrap()
                                        .root = BehaviorRoot::Forced(behavior_id);
                                } else {
                                    let mut used_tiles = SmallVec::<[Vec3i; 12]>::default();

                                    selection_state.entities.iter().for_each(|entity| {
                                        let target_tile = if used_tiles
                                            .contains(&input_state.mouse_tile_position)
                                        {
                                            if let Some(dst) = rl_ai::pathfinding::neighbors(
                                                &map,
                                                &input_state.mouse_tile_position,
                                            )
                                            .into_iter()
                                            .find(|neighbor| !used_tiles.contains(&neighbor.0))
                                            {
                                                dst.0
                                            } else {
                                                // TODO: This should really expand around the units in a predictable manner
                                                return;
                                            }
                                        } else {
                                            input_state.mouse_tile_position
                                        };

                                        used_tiles.push(target_tile);

                                        {
                                            let mut blackboard_component = world
                                                .get_component_mut::<BlackboardComponent>(*entity)
                                                .unwrap();

                                            blackboard_component.clear();
                                            blackboard_component.insert(
                                                fnv!("MoveParameters"),
                                                rl_core::data::bt::MoveParameters::new_tile(
                                                    target_tile,
                                                ),
                                            );
                                        }

                                        let behavior_id = resources
                                            .get::<BehaviorStorage>()
                                            .unwrap()
                                            .get_handle("move_to")
                                            .unwrap();

                                        world
                                            .get_component_mut::<BehaviorTreeComponent>(*entity)
                                            .unwrap()
                                            .root = BehaviorRoot::Forced(behavior_id);
                                    });
                                }
                            }
                        }
                    }
                }
            } else {
                while let Some(_) = input_action_channel.read(listener_id) {}
            }
        }

        command_buffer.write(world);
    })
}
