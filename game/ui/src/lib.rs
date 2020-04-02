#![deny(clippy::pedantic, clippy::all)]
#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::wildcard_imports,
    clippy::missing_safety_doc,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    clippy::unused_self,
    clippy::default_trait_access,
    clippy::module_name_repetitions,
    clippy::cast_possible_truncation,
    dead_code
)]

use rl_core::{
    dispatcher::{DispatcherBuilder, Stage},
    legion::prelude::*,
};
use std::{
    collections::HashMap,
    marker::PhantomData,
    string::ToString,
    sync::{Arc, Mutex},
};

pub use clipboard;
pub use imgui;

pub mod imgui_manager;
pub mod mapgen;
pub mod placement;
pub mod selection;
pub mod tasks;
pub mod tools;

//pub mod imgui_gl_pass;
pub mod imgui_vk_pass;

pub struct ImguiContextWrapper {
    pub context: imgui::Context,
    pub textures: imgui::Textures<rl_render_vk::ash::vk::DescriptorSet>,
    pub descriptor_pool: rl_render_vk::ash::vk::DescriptorPool,
    pub descriptor_layout: rl_render_vk::ash::vk::DescriptorSetLayout,
}
impl ImguiContextWrapper {
    pub fn new(context: imgui::Context) -> Self {
        Self {
            context,
            textures: imgui::Textures::default(),
            descriptor_pool: rl_render_vk::ash::vk::DescriptorPool::null(),
            descriptor_layout: rl_render_vk::ash::vk::DescriptorSetLayout::null(),
        }
    }
}
unsafe impl Send for ImguiContextWrapper {}

pub type ImguiContextLock = Arc<Mutex<ImguiContextWrapper>>;

static mut CURRENT_UI: Option<imgui::Ui<'static>> = None;

pub fn with(f: impl FnOnce(&imgui::Ui)) {
    unsafe {
        if let Some(ui) = current_ui() {
            (f)(ui);
        }
    }
}

unsafe fn current_ui<'a>() -> Option<&'a imgui::Ui<'a>> {
    CURRENT_UI.as_ref()
}

pub trait Window: Send + Sync {
    fn draw(
        &mut self,
        ui: &imgui::Ui<'_>,
        window_manager: &mut UiWindowManager,
        world: &World,
        resources: &Resources,
        command_buffer: &mut CommandBuffer,
    ) -> bool;

    fn visible(&self) -> bool;
    fn set_visible(&mut self, value: bool);
}

pub struct WindowObject<'a, F>
where
    F: 'a
        + FnMut(&imgui::Ui<'_>, &mut UiWindowManager, &World, &Resources, &mut CommandBuffer) -> bool
        + Send
        + Sync,
{
    visible: bool,
    name: String,
    draw: F,
    _marker: PhantomData<&'a F>,
}
impl<'a, F> Window for WindowObject<'a, F>
where
    F: 'a
        + FnMut(&imgui::Ui<'_>, &mut UiWindowManager, &World, &Resources, &mut CommandBuffer) -> bool
        + Send
        + Sync,
{
    fn draw(
        &mut self,
        ui: &imgui::Ui<'_>,
        window_manager: &mut UiWindowManager,
        world: &World,
        resources: &Resources,
        command_buffer: &mut CommandBuffer,
    ) -> bool {
        (self.draw)(ui, window_manager, world, resources, command_buffer)
    }
    fn visible(&self) -> bool {
        self.visible
    }
    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

#[derive(Default)]
pub struct UiWindowManager {
    visible_commands: Vec<(String, bool)>,
}
impl UiWindowManager {
    pub fn show(&mut self, name: &str) {
        self.visible_commands.push((name.to_string(), true));
    }
    pub fn hide(&mut self, name: &str) {
        self.visible_commands.push((name.to_string(), false));
    }
}

#[derive(Default)]
pub struct UiWindowSet {
    active_windows: HashMap<String, Box<dyn Window>>,
}
impl UiWindowSet {
    pub fn clear(&mut self) {
        self.active_windows.clear();
    }

    pub fn hide_all(&mut self) {
        self.active_windows
            .iter_mut()
            .for_each(|(_, window)| window.set_visible(false));
    }

    pub fn hide(&mut self, name: &str) {
        self.active_windows
            .get_mut(name)
            .unwrap()
            .set_visible(false);
    }

    pub fn show(&mut self, name: &str) {
        self.active_windows.get_mut(name).unwrap().set_visible(true);
    }

    pub fn create_with<S, F>(_: &mut World, resources: &Resources, name: S, visible: bool, f: F)
    where
        S: ToString,
        F: 'static
            + FnMut(
                &imgui::Ui<'_>,
                &mut UiWindowManager,
                &World,
                &Resources,
                &mut CommandBuffer,
            ) -> bool
            + Send
            + Sync,
    {
        resources
            .get_mut::<Self>()
            .unwrap()
            .create(name, visible, f)
    }

    #[allow(clippy::needless_pass_by_value)]
    pub fn create<S, F>(&mut self, name: S, visible: bool, f: F)
    where
        S: ToString,
        F: 'static
            + FnMut(
                &imgui::Ui<'_>,
                &mut UiWindowManager,
                &World,
                &Resources,
                &mut CommandBuffer,
            ) -> bool
            + Send
            + Sync,
    {
        self.active_windows.insert(
            name.to_string(),
            Box::new(WindowObject {
                visible,
                name: name.to_string(),
                draw: f,
                _marker: Default::default(),
            }),
        );
    }
}
pub fn system(
    world: &mut World,
    resources: &mut Resources,
) -> Box<dyn FnMut(&mut World, &mut Resources)> {
    resources.insert(UiWindowSet::default());

    let mut command_buffer = CommandBuffer::new(world);

    let mut manager = UiWindowManager::default();

    Box::new(move |world: &mut World, resources: &mut Resources| {
        with(|ui| {
            {
                let resources = &resources;

                let mut window_storage = resources.get_mut::<UiWindowSet>().unwrap();
                window_storage
                    .active_windows
                    .iter_mut()
                    .for_each(|(_, window)| {
                        if window.visible() {
                            let visible = window.draw(
                                ui,
                                &mut manager,
                                world,
                                resources,
                                &mut command_buffer,
                            );
                            window.set_visible(visible);
                        }
                    });

                manager.visible_commands.drain(..).for_each(|command| {
                    if command.1 {
                        window_storage.show(&command.0);
                    } else {
                        window_storage.hide(&command.0);
                    }
                });
            }

            command_buffer.write(world);
        });
    })
}

pub fn bundle(
    _: &mut World,
    _: &mut Resources,
    builder: &mut DispatcherBuilder,
) -> Result<(), anyhow::Error> {
    builder.add_system(Stage::Logic, selection::build_mouse_selection_system);
    builder.add_system(Stage::Logic, tasks::build_delete_task_system);

    builder.add_thread_local_fn(Stage::Render, selection::build_mouse_action_system);
    builder.add_thread_local_fn(Stage::Render, placement::build_placement_system);
    builder.add_thread_local_fn(Stage::Render, system);

    Ok(())
}
