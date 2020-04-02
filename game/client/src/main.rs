#![feature(associated_type_defaults)]
#![deny(clippy::pedantic, clippy::all)]
#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::wildcard_imports,
    clippy::missing_safety_doc,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    clippy::cast_possible_truncation,
    dead_code,
    clippy::default_trait_access,
    clippy::module_name_repetitions
)]

use rl_core::{
    app::Application,
    camera::Camera,
    dispatcher::{DispatcherBuilder, RelativeStage, Stage},
    ecs_manager::EcsManagerBuilder,
    event::Channel,
    input::{ActionBinding, Binding, InputActionEvent, InputState},
    legion::prelude::*,
    map::{tile::Tile, Map},
    math::Vec3i,
    settings::Settings,
    transform::*,
    winit::{
        event::{MouseButton, VirtualKeyCode},
        event_loop::EventLoop,
    },
};

use rl_ui::imgui_manager::ImguiManager;

pub mod behavior;
pub mod inventory;
pub mod stockpile;
pub mod weather;

pub mod debug;
pub mod item_physics;
pub mod saveload;
pub mod spawners;
pub mod test;

fn test_bindings(input_state: &mut InputState) {
    input_state.bindings.insert(
        Binding::Key(VirtualKeyCode::W, None),
        ActionBinding::CameraUp,
    );
    input_state.bindings.insert(
        Binding::Key(VirtualKeyCode::S, None),
        ActionBinding::CameraDown,
    );
    input_state.bindings.insert(
        Binding::Key(VirtualKeyCode::A, None),
        ActionBinding::CameraLeft,
    );
    input_state.bindings.insert(
        Binding::Key(VirtualKeyCode::D, None),
        ActionBinding::CameraRight,
    );
    input_state.bindings.insert(
        Binding::Key(VirtualKeyCode::Q, None),
        ActionBinding::CameraZoomIn,
    );
    input_state.bindings.insert(
        Binding::Key(VirtualKeyCode::E, None),
        ActionBinding::CameraZoomOut,
    );

    input_state.bindings.insert(
        Binding::Key(VirtualKeyCode::Comma, None),
        ActionBinding::CameraZUp,
    );
    input_state.bindings.insert(
        Binding::Key(VirtualKeyCode::Period, None),
        ActionBinding::CameraZDown,
    );

    input_state.bindings.insert(
        Binding::Key(VirtualKeyCode::F, None),
        ActionBinding::DebugWriteMap,
    );

    input_state.bindings.insert(
        Binding::Mouse(MouseButton::Left, None),
        ActionBinding::Selection,
    );

    input_state.bindings.insert(
        Binding::Mouse(MouseButton::Right, None),
        ActionBinding::DoAction,
    );

    input_state.bindings.insert(
        Binding::Key(VirtualKeyCode::Delete, None),
        ActionBinding::Delete,
    );
}

fn build_camera_movement_system(_: &mut World, resources: &mut Resources) -> Box<dyn Schedulable> {
    // Prepare the action bindings

    {
        let mut input_state = resources.get_mut::<InputState>().unwrap();
        test_bindings(&mut input_state);
    }

    let listener_id = resources
        .get_mut::<Channel<InputActionEvent>>()
        .unwrap()
        .bind_listener(64);

    SystemBuilder::<()>::new("camera_movement_system")
        .with_query(<(Read<Camera>, Write<Translation>, Write<Scale>)>::query())
        .read_resource::<InputState>()
        .read_resource::<Channel<InputActionEvent>>()
        .write_resource::<Map>()
        .build(move |_, world, (input_state, channel, map), camera_query| {
            let (_camera, mut translation, mut scale) =
                camera_query.iter_mut(world).next().unwrap();

            while let Some(event) = channel.read(listener_id) {
                if let InputActionEvent::Released(action) = event {
                    match action {
                        ActionBinding::CameraZUp => {
                            if (translation.z.floor() as i32) < map.dimensions().z - 1 {
                                translation.z += 1.0;
                            }
                        }
                        ActionBinding::CameraZDown => {
                            if (translation.z.floor() as i32) > 0 {
                                translation.z -= 1.0;
                            }
                        }
                        ActionBinding::DebugWriteMap => {
                            use rl_core::rand::{self, Rng};
                            let mut rng = rand::thread_rng();

                            let coord = Vec3i::new(
                                rng.gen_range(0, map.dimensions().x),
                                rng.gen_range(0, map.dimensions().y),
                                0,
                            );
                            println!("Debug changing: {:?}", coord);
                            map.set(
                                coord,
                                Tile {
                                    material: rng.gen_range(1, 255),
                                    ..Default::default()
                                },
                            );
                        }
                        _ => {}
                    }
                }
            }

            if input_state.is_action_down(ActionBinding::CameraZoomIn) {
                **scale += 0.2;
            }
            if input_state.is_action_down(ActionBinding::CameraZoomOut) {
                **scale -= 0.2;
            }

            if input_state.is_action_down(ActionBinding::CameraUp) {
                translation.y -= 20.0;
            }
            if input_state.is_action_down(ActionBinding::CameraDown) {
                translation.y += 20.0;
            }

            if input_state.is_action_down(ActionBinding::CameraLeft) {
                translation.x -= 20.0;
            }
            if input_state.is_action_down(ActionBinding::CameraRight) {
                translation.x += 20.0;
            }
        })
}

fn main() -> Result<(), anyhow::Error> {
    let settings = std::fs::read_to_string("data/settings.toml")
        .ok()
        .map_or_else(Settings::default, |data| {
            rl_core::toml::from_str(&data).unwrap()
        });
    /*
        let mut logger_settings = game_metrics::LoggerSettings::default();
        logger_settings
            .targets
            .insert("behavior".to_string(), game_metrics::LogLevel::Trace);
        game_metrics::Logger::change_settings(logger_settings);
    */
    let event_loop = EventLoop::new();
    let mut app = Application::new(settings.clone())?;

    rl_core::defs::load_all_defs(&mut app.game_state.resources, None)?;
    // Add test pawns
    rl_ai::utility::prepare(&mut app.game_state.world, &mut app.game_state.resources)?;
    crate::behavior::prepare(&mut app.game_state.world, &mut app.game_state.resources)?;

    test::init_minimal_world(
        &mut app.game_state.world,
        &mut app.game_state.resources,
        &settings,
    )?;

    let input_manager = rl_core::input::InputManager::new(&mut app.context, &mut app.game_state)?;
    let imgui_manager = ImguiManager::new(&mut app.game_state)?;

    #[cfg(all(not(feature = "opengl")))]
    let render_manager = {
        rl_render_vk::manager::RenderManagerBuilder::new(
            &mut app.context,
            &mut app.game_state,
            &event_loop,
        )?
        .with_pass(rl_render_vk::pass::map::MapPass::new)
        .with_pass(rl_render_vk::pass::entities::EntitiesPass::new)
        .with_pass(rl_render_vk::pass::sparse_sprite::SparseSpritePass::new)
        .with_pass(rl_ui::imgui_vk_pass::ImguiPass::new)
        .with_pass(rl_render_vk::pass::debug::DebugPass::new)
        .build(app.state_mut())?
    };

    #[cfg(all(feature = "opengl", not(feature = "vulkan")))]
    let render_manager = {
        let app_context = &mut app.context;
        let state = &mut app.game_state;

        rl_render_glow::manager::RenderManagerBuilder::new()?
            .with_pass(rl_ui::imgui_gl_pass::ImguiPass::new)
            .with_pass(rl_render_glow::pass::debug::DebugLinesPass::new)
            .build(&event_loop, app_context, state)?
    };

    let ecs_manager = EcsManagerBuilder::default()
        .with_dispatcher(
            DispatcherBuilder::default()
                .with_system(Stage::Begin, build_camera_movement_system)
                .with_system(Stage::Begin, rl_core::input::build_mouse_world_state_system)
                .with_flush(Stage::Render)
                .with_system(Stage::Render, rl_core::systems::progress_bar::build)
                .with_thread_local_fn(Stage::End, crate::spawners::build_system)
                .with_system(Stage::End, rl_core::garbage_collector::build)
                .with_flush(RelativeStage(Stage::End, 500))
                .with_bundle(rl_ui::bundle),
            |_| true,
        )
        .with_dispatcher(
            DispatcherBuilder::default()
                .with_bundle(crate::inventory::bundle)
                .with_bundle(rl_ai::bundle)
                .with_bundle(rl_reaction::bundle)
                .with_bundle(item_physics::bundle)
                .with_flush(RelativeStage(Stage::AI, -500))
                .with_bundle(rl_core::map::systems::bundle)
                .with_bundle(crate::weather::bundle)
                .with_system(Stage::End, stockpile::build_stockpile_update_children),
            rl_core::is_game_tick,
        )
        .build(&mut app.context, &mut app.game_state)?;

    let world = &mut app.game_state.world;
    let resources = &mut app.game_state.resources;
    {
        debug::build_debug_overlay(world, resources)?;

        debug::ai::build_decisions_window(world, resources);
        rl_ui::tools::build_tools_overlay(world, resources)?;
        rl_ui::tasks::build_task_window(world, resources);

        #[cfg(all(not(feature = "opengl")))]
        {
            rl_render_vk::manager::RenderManager::restart_setup(resources);

            // Any extra render setups  happen here
            debug::build_sprite_selector(world, resources)?;

            rl_render_vk::manager::RenderManager::flush_setup(resources);
        }
    }
    //

    app.add_manager(imgui_manager);
    app.add_manager(input_manager);
    app.add_manager(ecs_manager);
    app.add_manager(render_manager);

    app.run(event_loop)
}
