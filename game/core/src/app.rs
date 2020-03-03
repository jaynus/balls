use crate::{
    event::{Channel, ListenerId},
    legion::prelude::*,
    settings::Settings,
    time::Time,
    GameState, GlobalCommandBuffer, Manager,
};

use std::sync::Arc;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

pub struct ApplicationContext {}

pub struct Application {
    pub context: ApplicationContext,
    pub game_state: GameState,

    pub managers: Vec<Box<dyn Manager>>,
    load_world_channel_listener_id: ListenerId,
}

#[derive(Clone)]
pub struct LoadWorldEvent {
    closure: Arc<dyn Fn(&mut World, &mut Resources) -> Result<(), failure::Error> + Send + Sync>,
}
impl LoadWorldEvent {
    pub fn new<F>(f: F) -> Self
    where
        F: 'static + Fn(&mut World, &mut Resources) -> Result<(), failure::Error> + Send + Sync,
    {
        Self {
            closure: Arc::new(f),
        }
    }
}

impl Application {
    pub fn new(settings: Settings) -> Result<Self, failure::Error> {
        let mut game_state = GameState::default();

        game_metrics::Logger::init().unwrap();

        game_state.resources.insert(crate::Random::new("balls"));

        game_state.resources.insert(game_metrics::Metrics::new(1));

        game_state.resources.insert(Time::default());
        game_state.resources.insert(crate::Logging::default());

        let global_buffer = GlobalCommandBuffer::new(&mut game_state.world);
        game_state.resources.insert(global_buffer);

        // Add the allocator
        game_state.resources.insert(crate::Allocators {
            frame_arena: purple::Arena::with_capacity(524_288_000), //500mb
            static_arena: purple::Arena::with_capacity(52_428_800), //50mb
        });

        let mut load_world_channel = Channel::<LoadWorldEvent>::default();
        let load_world_channel_listener_id = load_world_channel.bind_listener(1);
        game_state.resources.insert(load_world_channel);

        game_state.resources.insert(settings);

        Ok(Self {
            context: ApplicationContext {},
            managers: Vec::default(),
            game_state,
            load_world_channel_listener_id,
        })
    }

    pub fn run(mut self, event_loop: EventLoop<()>) -> Result<(), failure::Error> {
        let mut exit = false;

        event_loop.run(move |event, _, control_flow| {
            *control_flow = ControlFlow::Poll;

            // slog::trace!(&self.context.log, "{:?}", event);

            if let Event::WindowEvent { event, .. } = &event {
                match event {
                    WindowEvent::CloseRequested => {
                        exit = true;
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::KeyboardInput { input, .. } => match input {
                        KeyboardInput {
                            virtual_keycode,
                            state,
                            ..
                        } => {
                            if let (Some(VirtualKeyCode::Escape), ElementState::Pressed) =
                                (virtual_keycode, state)
                            {
                                exit = true;
                                *control_flow = ControlFlow::Exit;
                            }
                        }
                    },

                    _ => (),
                }
            }

            self.on_event(&event).unwrap();

            if let Event::MainEventsCleared = event {
                if exit {
                    self.destroy();
                } else {
                    self.game_state
                        .resources
                        .get::<Arc<Window>>()
                        .unwrap()
                        .request_redraw();
                }
            }

            if let Event::RedrawRequested(_) = event {
                if self.tick().is_err() {
                    *control_flow = ControlFlow::Exit;
                    return;
                }
            }
        });
    }

    pub fn destroy(&mut self) {
        for mut manager in self.managers.drain(..) {
            manager.destroy(&mut self.context, &mut self.game_state);
        }
    }

    pub fn on_event(&mut self, event: &Event<()>) -> Result<(), failure::Error> {
        for manager in &mut self.managers {
            if manager
                .on_event(&mut self.context, &mut self.game_state, event)?
                .is_none()
            {
                return Ok(());
            }
        }

        Ok(())
    }

    pub fn on_tick(&mut self) -> Result<(), failure::Error> {
        for manager in &mut self.managers {
            manager.tick(&mut self.context, &mut self.game_state)?;
        }

        Ok(())
    }

    #[game_metrics::instrument(name = "frame")]
    pub fn tick(&mut self) -> Result<(), failure::Error> {
        self.game_state.resources.get_mut::<Time>().unwrap().tick();

        self.on_tick()?;

        let mut global_cmd =
            <Write<GlobalCommandBuffer>>::fetch_mut(&mut self.game_state.resources);
        global_cmd.write(&mut self.game_state.world);

        self.game_state.world.defrag(None);

        Ok(())
    }

    pub fn add_manager<T: 'static + Manager>(&mut self, manager: T) {
        self.managers.push(Box::new(manager));
    }

    pub fn state(&self) -> &GameState {
        &self.game_state
    }
    pub fn state_mut(&mut self) -> &mut GameState {
        &mut self.game_state
    }
}
