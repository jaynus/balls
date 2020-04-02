use super::nodes as general_nodes;
use super::ExecuteReactionParameters;

use rl_ai::{
    bt::{self, make, BehaviorStatus},
    task::TaskCache,
};

pub fn build(storage: &mut bt::BehaviorStorage) -> Result<(), anyhow::Error> {
    let do_task = make::if_else(
        make::closure(None, nodes::find_task),
        make::sequence(&[
            make::selector(&[
                make::closure(None, general_nodes::move_to),
                make::not(make::closure(None, nodes::cancel_task)),
            ]),
            make::if_else(
                make::sequence(&[
                    make::closure(None, nodes::prepare_task_reaction_parameterss),
                    make::closure(None, general_nodes::execute_reaction),
                ]),
                make::closure(None, nodes::complete_task),
                make::not(make::closure(None, nodes::cancel_task)),
            ),
        ]),
        make::sequence(&[
            make::closure(None, general_nodes::try_get_reagent),
            make::sub("pickup_item", &storage),
        ]),
    );
    storage.insert("do_task", do_task);

    Ok(())
}

pub mod nodes {
    use super::*;
    use rl_ai::{
        bt::*,
        task::{HasTasksComponent, Task, TaskHandle, TaskResult},
        TaskPrioritiesComponent,
    };
    use rl_core::{
        components::PositionComponent, data::bt::*, fnv, legion::prelude::*, GameStateRef,
    };
    use rl_reaction::ReactionEntity;

    pub fn find_task(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        use rl_ai::task::FindBestTaskError;

        let cache = unsafe { <Write<TaskCache>>::fetch_unchecked(&state.resources) };

        if let Some(current_task) = args
            .blackboard
            .get::<(Entity, TaskHandle, Task)>(fnv!("current_task"))
        {
            // Validate the task is still valid
            if state.world.is_alive(current_task.0) {
                return BehaviorStatus::success();
            } else {
                return BehaviorStatus::failure();
            }
        }

        // Find the nearest task to us.
        if cache.is_empty() {
            // NO tasks available
            return BehaviorStatus::failure();
        }

        // TODO: better task selection
        // Take the first task we find for now

        let source_location = **state
            .world
            .get_component::<PositionComponent>(args.entity)
            .unwrap();

        match cache.find_best(
            state,
            args.entity,
            source_location,
            &state
                .world
                .get_component::<TaskPrioritiesComponent>(args.entity)
                .unwrap(),
        ) {
            Ok((task_location, entry, handle)) => {
                let task = entry.queue.get_mut().take(handle).unwrap();

                args.blackboard
                    .insert(fnv!("current_task"), (entry.entity, handle, task));

                args.blackboard.insert(
                    fnv!("MoveParameters"),
                    MoveParameters::new_tile(task_location),
                );

                return BehaviorStatus::success();
            }
            Err(e) => {
                if let FindBestTaskError::MissingReagent(kind) = e {
                    args.blackboard.insert(fnv!("missing_reagent"), kind);
                    return BehaviorStatus::failure();
                }
            }
        }

        BehaviorStatus::failure()
    }

    pub fn prepare_task_reaction_parameterss(
        state: GameStateRef,
        args: &mut BehaviorArgs<'_>,
    ) -> BehaviorStatus {
        if !args.blackboard.contains(fnv!("current_task")) {
            return BehaviorStatus::failure();
        }

        let parameters = {
            let current_task = args
                .blackboard
                .get::<(Entity, TaskHandle, Task)>(fnv!("current_task"))
                .unwrap();

            if !state.world.is_alive(current_task.0) {
                return BehaviorStatus::failure();
            }

            ExecuteReactionParameters {
                reaction: current_task.2.reaction,
                target: ReactionEntity::Task(current_task.0),
            }
        };

        args.blackboard
            .insert(fnv!("ExecuteReactionParameters"), parameters);

        BehaviorStatus::success()
    }

    pub fn cancel_task(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        let mut result = BehaviorStatus::failure();
        if let Some(current_task) = args
            .blackboard
            .get::<(Entity, TaskHandle, Task)>(fnv!("current_task"))
        {
            if let Some(component) = state
                .world
                .get_component::<HasTasksComponent>(current_task.0)
            {
                if component.storage.get_mut().cancel(current_task.1) {
                    result = BehaviorStatus::success();
                }
            }
        }

        if result == BehaviorStatus::success() {
            let last_task = args
                .blackboard
                .remove_get::<(Entity, TaskHandle, Task)>(fnv!("current_task"))
                .unwrap();

            let _ = args
                .blackboard
                .insert(fnv!("last_task"), (last_task, TaskResult::Cancelled));
        }

        result
    }

    pub fn complete_task(state: GameStateRef, args: &mut BehaviorArgs<'_>) -> BehaviorStatus {
        let mut result = BehaviorStatus::failure();
        if let Some(current_task) = args
            .blackboard
            .get::<(Entity, TaskHandle, Task)>(fnv!("current_task"))
        {
            if let Some(component) = state
                .world
                .get_component::<HasTasksComponent>(current_task.0)
            {
                if component.storage.get_mut().complete(current_task.1) {
                    result = BehaviorStatus::success();
                }
            }
        }

        if result == BehaviorStatus::success() {
            let last_task = args
                .blackboard
                .remove_get::<(Entity, TaskHandle, Task)>(fnv!("current_task"));

            let _ = args
                .blackboard
                .insert(fnv!("last_task"), (last_task, TaskResult::Complete));
        }

        result
    }
}
