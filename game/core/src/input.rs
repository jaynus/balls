use crate::{
    app,
    event::Channel,
    legion::prelude::*,
    math::{Vec2, Vec3, Vec3i},
    GameState, Manager,
};
use fxhash::FxHashMap;
use rl_render_pod::sprite::Sprite;
use std::sync::Arc;
use winit::{
    event::{
        ElementState, Event, KeyboardInput, ModifiersState, MouseButton,
        MouseScrollDelta, TouchPhase, VirtualKeyCode, WindowEvent,
    },
    window::Window,
};

pub trait PlacementRequest: Send + Sync {
    fn on_draw(&self, world: &World, resources: &Resources) -> Sprite;
    fn on_complete(
        &self,
        world: &mut World,
        resources: &mut Resources,
        world_position: Vec3,
        tile_position: Vec3i,
    );
    fn on_cancel(&self, world: &World, resources: &Resources);
}

pub struct PlacementRequestImpl<D, C, F> {
    pub on_draw_fn: D,
    pub on_complete_fn: C,
    pub on_cancel_fn: F,
}
impl<D, C, F> PlacementRequestImpl<D, C, F>
where
    D: Fn(&World, &Resources) -> Sprite + Send + Sync,
    C: Fn(&mut World, &mut Resources, Vec3, Vec3i) + Send + Sync,
    F: Fn(&World, &Resources) + Send + Sync,
{
    pub fn new(on_draw_fn: D, on_complete_fn: C, on_cancel_fn: F) -> Self {
        Self {
            on_draw_fn,
            on_complete_fn,
            on_cancel_fn,
        }
    }
}
impl<D, C, F> PlacementRequest for PlacementRequestImpl<D, C, F>
where
    D: Fn(&World, &Resources) -> Sprite + Send + Sync,
    C: Fn(&mut World, &mut Resources, Vec3, Vec3i) + Send + Sync,
    F: Fn(&World, &Resources) + Send + Sync,
{
    fn on_draw(&self, world: &World, resources: &Resources) -> Sprite {
        (self.on_draw_fn)(world, resources)
    }
    fn on_complete(
        &self,
        world: &mut World,
        resources: &mut Resources,
        world_position: Vec3,
        tile_position: Vec3i,
    ) {
        (self.on_complete_fn)(world, resources, world_position, tile_position)
    }
    fn on_cancel(&self, world: &World, resources: &Resources) {
        (self.on_cancel_fn)(world, resources)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum DesignateAction {
    Channel,
    Dig,
    ChopTree,
    Stockpile,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ActionBinding {
    CameraLeft,
    CameraRight,
    CameraUp,
    CameraDown,
    CameraZoomIn,
    CameraZoomOut,

    CameraZUp,
    CameraZDown,

    Selection,
    DoAction,

    // Debug
    DebugWriteMap,

    //Keyboard mapping/specific designation mappings
    DesignateDig,
    DesignateMine,

    Delete,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Binding {
    Key(VirtualKeyCode, Option<ModifiersState>),
    Mouse(MouseButton, Option<ModifiersState>),
}

pub type BindingMap = FxHashMap<Binding, ActionBinding>;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum InputActionEvent {
    Released(ActionBinding),
    Pressed(ActionBinding),
}

#[derive(Debug, Copy, Clone)]
pub enum InputEvent {
    MouseMoved {
        position: Vec2,
        modifiers: ModifiersState,
    },
    MousePressed {
        button: MouseButton,
        position: Vec2,
        modifiers: ModifiersState,
    },
    MouseReleased {
        button: MouseButton,
        position: Vec2,
        modifiers: ModifiersState,
    },
    MouseWheelMoved {
        delta: MouseScrollDelta,
        phase: TouchPhase,
        modifiers: ModifiersState,
    },
    KeyPressed {
        key: VirtualKeyCode,
        modifiers: ModifiersState,
    },
    KeyReleased {
        key: VirtualKeyCode,
        modifiers: ModifiersState,
    },
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum InputStateKind {
    Selection,
    Placement,
}
impl Default for InputStateKind {
    fn default() -> Self {
        Self::Selection
    }
}

pub struct InputState {
    pub mouse_position: Vec2,
    pub mouse_state: [bool; 16], // maps to winit::MouseButton. Other is Other + u8
    pub key_state: [bool; 255],  // maps to winit::VirtualKeyCode
    pub modifiers_state: ModifiersState,
    pub action_state: [bool; 255],

    pub bindings: BindingMap,

    pub ignore_mouse: bool,
    pub ignore_keyboard: bool,

    pub mouse_world_position: Vec3,
    pub mouse_tile_position: Vec3i,

    placement: Option<Box<dyn PlacementRequest>>,

    pub state: InputStateKind,
}
impl InputState {
    #[inline]

    pub fn placement_request(&self) -> Option<&dyn PlacementRequest> {
        self.placement.as_ref().map(std::convert::AsRef::as_ref)
    }

    pub fn swap_placement_request<P: 'static + PlacementRequest>(
        &mut self,
        world: &World,
        resources: &Resources,
        request: P,
    ) {
        if self.placement.is_some() {
            self.placement.take().unwrap().on_cancel(world, resources);
        }
        self.state = InputStateKind::Placement;
        self.placement = Some(Box::new(request));
    }

    pub fn complete_placement_request(
        &mut self,
        world: &mut World,
        resources: &mut Resources,

        world_position: Vec3,
        tile_position: Vec3i,
    ) {
        if self.placement.is_some() {
            self.placement.take().unwrap().on_complete(
                world,
                resources,
                world_position,
                tile_position,
            );
        }
        self.state = InputStateKind::Selection;
        self.placement = None;
    }

    pub fn clear_placement_request(&mut self, world: &World, resources: &Resources) {
        if self.placement.is_some() {
            self.placement.take().unwrap().on_cancel(world, resources);
        }
        self.state = InputStateKind::Selection;
        self.placement = None;
    }

    pub fn is_key_down(&self, key: VirtualKeyCode) -> bool {
        self.key_state[key as usize]
    }

    pub fn is_action_down(&self, action: ActionBinding) -> bool {
        self.action_state[action as usize]
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            mouse_state: [false; 16],
            key_state: [false; 255],
            action_state: [false; 255],
            mouse_position: Vec2::default(),
            modifiers_state: ModifiersState::default(),
            bindings: FxHashMap::default(),
            ignore_mouse: false,
            ignore_keyboard: false,

            mouse_world_position: Vec3::default(),
            mouse_tile_position: Vec3i::default(),

            state: InputStateKind::default(),
            placement: None,
        }
    }
}

pub struct InputManager {}
impl InputManager {
    pub fn new(
        _app: &mut app::ApplicationContext,
        state: &mut crate::GameState,
    ) -> Result<Self, anyhow::Error> {
        state.resources.insert(InputState::default());
        state.resources.insert(Channel::<InputEvent>::default());
        state
            .resources
            .insert(Channel::<InputActionEvent>::default());

        Ok(Self {})
    }

    #[allow(clippy::cognitive_complexity, clippy::match_single_binding)]
    pub fn handle_action(
        event: &WindowEvent,
        input_state: &mut InputState,
        channel: &mut Channel<InputActionEvent>,
    ) -> Result<bool, anyhow::Error> {
        match event {
            WindowEvent::KeyboardInput { input, .. } => match input {
                KeyboardInput {
                    virtual_keycode,
                    state,
                    ..
                } => {
                    let virtual_keycode = virtual_keycode
                        .ok_or(anyhow::anyhow!("Failed to unwrap virtual keycode"))?;

                    if let Some(find) = input_state
                        .bindings
                        .get(&Binding::Key(virtual_keycode, None))
                    {
                        match state {
                            ElementState::Pressed => {
                                channel
                                    .write(InputActionEvent::Pressed(*find))
                                    .map_err(|e| anyhow::anyhow!("{:?}", e))?;
                                input_state.action_state[*find as usize] = true;
                            }
                            ElementState::Released => {
                                channel
                                    .write(InputActionEvent::Released(*find))
                                    .map_err(|e| anyhow::anyhow!("{:?}", e))?;
                                input_state.action_state[*find as usize] = false;
                            }
                        }
                        Ok(true)
                    } else {
                        Ok(false)
                    }
                }
            },
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(find) = input_state.bindings.get(&Binding::Mouse(*button, None)) {
                    match state {
                        ElementState::Pressed => {
                            channel
                                .write(InputActionEvent::Pressed(*find))
                                .map_err(|e| anyhow::anyhow!("{:?}", e))?;
                        }
                        ElementState::Released => {
                            channel
                                .write(InputActionEvent::Released(*find))
                                .map_err(|e| anyhow::anyhow!("{:?}", e))?;
                        }
                    }
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }
}

impl Manager for InputManager {
    #[allow(
        clippy::cognitive_complexity,
        clippy::too_many_lines,
        clippy::match_single_binding,
        clippy::single_match
    )]
    fn on_event<'a>(
        &mut self,
        _context: &mut app::ApplicationContext,
        game_state: &mut GameState,
        src_event: &'a winit::event::Event<()>,
    ) -> Result<Option<&'a winit::event::Event<'a, ()>>, anyhow::Error> {
        let (mut input_state, window) =
            <(Write<InputState>, Read<Arc<Window>>)>::fetch_mut(&mut game_state.resources);

        let input_channel = game_state
            .resources
            .get_mut::<Channel<InputEvent>>()
            .unwrap();

        let mut action_channel = game_state
            .resources
            .get_mut::<Channel<InputActionEvent>>()
            .unwrap();

        match &src_event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::ModifiersChanged(modifiers) => {
                    input_state.modifiers_state = *modifiers;
                },
                WindowEvent::KeyboardInput { input, .. } => match input {
                    KeyboardInput {
                        virtual_keycode,
                        state,
                        ..
                    } => {
                        if input_state.ignore_keyboard {
                            return Ok(Some(src_event));
                        }

                        if virtual_keycode.is_none() {
                            return Ok(Some(src_event));
                        }
                        let keycode = virtual_keycode.unwrap() as usize;

                        let last_state = input_state.key_state[keycode];
                        input_state.key_state[keycode] = match state {
                            ElementState::Pressed => true,
                            ElementState::Released => false,
                        };

                        if last_state != input_state.key_state[keycode] {
                            match state {
                                ElementState::Pressed => {
                                    let event = InputEvent::KeyPressed {
                                        key: virtual_keycode.unwrap(),
                                        modifiers: input_state.modifiers_state,
                                    };
                                    //slog::trace!(&self.log, "{:?}", event);
                                    input_channel
                                        .write(event)
                                        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
                                }
                                ElementState::Released => {
                                    let event = InputEvent::KeyReleased {
                                        key: virtual_keycode.unwrap(),
                                        modifiers: input_state.modifiers_state,
                                    };
                                    //slog::trace!(&self.log, "{:?}", event);
                                    input_channel
                                        .write(event)
                                        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
                                }
                            };

                            Self::handle_action(event, &mut input_state, &mut action_channel)?;
                        }
                    }
                },
                WindowEvent::MouseInput { state, button, .. } => {
                    if input_state.ignore_mouse {
                        return Ok(Some(src_event));
                    }

                    let mouse_index = match button {
                        MouseButton::Left => 0,
                        MouseButton::Right => 2,
                        MouseButton::Middle => 3,
                        MouseButton::Other(v) => 4 + *v as usize,
                    };

                    let last_state = input_state.mouse_state[mouse_index];
                    input_state.mouse_state[mouse_index] = match state {
                        ElementState::Pressed => true,
                        ElementState::Released => false,
                    };

                    if last_state != input_state.mouse_state[mouse_index] {
                        match state {
                            ElementState::Pressed => {
                                let event = InputEvent::MousePressed {
                                    button: *button,
                                    modifiers: input_state.modifiers_state,
                                    position: input_state.mouse_position,
                                };

                                //slog::trace!(&self.log, "{:?}", event);
                                input_channel
                                    .write(event)
                                    .map_err(|e| anyhow::anyhow!("{:?}", e))?;
                            }
                            ElementState::Released => {
                                let event = InputEvent::MouseReleased {
                                    button: *button,
                                    modifiers: input_state.modifiers_state,
                                    position: input_state.mouse_position,
                                };

                                //slog::trace!(&self.log, "{:?}", event);
                                input_channel
                                    .write(event)
                                    .map_err(|e| anyhow::anyhow!("{:?}", e))?;
                            }
                        };

                        Self::handle_action(event, &mut input_state, &mut action_channel)?;
                    }
                }
                #[allow(clippy::cast_possible_truncation)]
                WindowEvent::CursorMoved { position, .. } => {
                    input_state.mouse_position = Vec2::new(position.x as f32, position.y as f32)
                        / window.scale_factor() as f32;

                    input_channel
                        .write(InputEvent::MouseMoved {
                            modifiers: input_state.modifiers_state,
                            position: input_state.mouse_position,
                        })
                        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
                }
                WindowEvent::MouseWheel { delta, phase, .. } => {
                    let event = InputEvent::MouseWheelMoved {
                        modifiers: input_state.modifiers_state,
                        delta: *delta,
                        phase: *phase,
                    };

                    //slog::trace!(&self.log, "{:?}", event);
                    input_channel
                        .write(event)
                        .map_err(|e| anyhow::anyhow!("{:?}", e))?;
                }
                _ => (),
            },
            _ => (),
        }
        Ok(Some(src_event))
    }
}

pub fn build_mouse_world_state_system(
    _world: &mut World,
    _: &mut Resources,
) -> Box<dyn Schedulable> {
    use crate::{
        camera::Camera,
        transform::{Scale, Translation},
    };
    SystemBuilder::<()>::new("mouse_world_state_system")
        .read_resource::<crate::Logging>()
        .read_resource::<crate::map::Map>()
        .write_resource::<InputState>()
        .with_query(<(Read<Camera>, Read<Translation>, Read<Scale>)>::query())
        .build(move |_, world, (_log, map, input_state), camera_query| {
            let camera = camera_query.iter_mut(world).next().unwrap();

            input_state.mouse_world_position = camera
                .0
                .unproject(input_state.mouse_position.into(), (&camera.1, &camera.2));
            input_state.mouse_tile_position = map.world_to_tile(input_state.mouse_world_position);
        })
}
