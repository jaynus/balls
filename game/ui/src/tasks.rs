use crate::{selection::SelectionState, UiWindowSet};
use imgui::{im_str, Condition};
use rl_ai::HasTasksComponent;
use rl_core::defs::{reaction::ReactionDefinition, DefinitionId, DefinitionStorage};
use rl_core::{
    components::{Destroy, PositionComponent, VirtualTaskTag},
    event::Channel,
    input::{ActionBinding, InputActionEvent},
    legion::prelude::*,
    map::Map,
    Logging,
};

pub fn build_task_window(world: &mut World, resources: &mut Resources) {
    let query = <(Read<PositionComponent>, Read<HasTasksComponent>)>::query();

    UiWindowSet::create_with(
        world,
        resources,
        "tasksWindow",
        true,
        move |ui, _window_manager, world, resources, _command_buffer| {
            let (_log, reaction_defs) =
                <(Read<Logging>, Read<DefinitionStorage<ReactionDefinition>>)>::fetch(&resources);

            imgui::Window::new(im_str!("tasksWindow"))
                .size([350.0, 300.0], Condition::Once)
                .build(ui, || {
                    ui.columns(4, im_str!("balls"), true);
                    ui.text("Position");
                    ui.next_column();
                    ui.text("Kind");
                    ui.next_column();
                    ui.text("Reaction");
                    ui.next_column();
                    ui.text("Status");

                    for (tile_pos, tasks) in query.iter(&world) {
                        for (handle, task) in tasks.storage.get().iter_all() {
                            ui.next_column();
                            ui.text(&format!("{},{},{}", tile_pos.x, tile_pos.y, tile_pos.z));
                            ui.next_column();
                            ui.text(&format!("{:?}", task.kind));
                            ui.next_column();
                            ui.text(&task.reaction.as_str(&reaction_defs).to_string());
                            ui.next_column();
                            ui.text(if tasks.storage.get().is_available(handle) {
                                "AVAIL"
                            } else {
                                "TAKEN"
                            });
                        }
                    }
                    ui.columns(1, im_str!("asdf"), false);
                });

            true
        },
    );
}

pub fn build_delete_task_system(_: &mut World, resources: &mut Resources) -> Box<dyn Schedulable> {
    let listener_id = resources
        .get_mut::<Channel<InputActionEvent>>()
        .unwrap()
        .bind_listener(64);

    SystemBuilder::<()>::new("delete_task_system")
        .read_resource::<SelectionState>()
        .read_resource::<Channel<InputActionEvent>>()
        .read_resource::<Map>()
        .with_query(<Read<HasTasksComponent>>::query().filter(tag::<VirtualTaskTag>()))
        .build(
            move |command_buffer,
                  world,
                  (selection_state, input_action_channel, _map),
                  virtual_task_query| {
                while let Some(action) = input_action_channel.read(listener_id) {
                    if let InputActionEvent::Released(ActionBinding::Delete) = action {
                        if let Some(selection) = selection_state.last_selection.as_ref() {
                            // Do we have tasks in this selection which we can delete?
                            for (entity, _) in virtual_task_query.iter_entities(world) {
                                if selection.entities.iter().any(|e| *e == entity) {
                                    command_buffer.add_component(entity, Destroy::default());
                                }
                            }
                        }
                    }
                }
            },
        )
}
