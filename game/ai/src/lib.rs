#![deny(clippy::pedantic, clippy::all)]
#![allow(
    clippy::must_use_candidate,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    dead_code,
    clippy::use_self,
    clippy::default_trait_access,
    clippy::module_name_repetitions,
    non_camel_case_types,
    incomplete_features
)]
#![feature(try_trait, core_intrinsics, impl_trait_in_bindings)]
use rl_core::{
    dispatcher::{DispatcherBuilder, RelativeStage, Stage},
    failure,
    legion::prelude::*,
};

pub mod action;
pub mod body;
pub mod bt;
pub mod iaus;
pub mod movement;
pub mod needs;
pub mod pathfinding;
pub mod task;
pub mod utility;

pub use task::*;

#[derive(Debug, Clone, Copy, Hash)]
pub struct SensesComponent {}
impl Default for SensesComponent {
    fn default() -> Self {
        Self {}
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Hash, serde::Serialize, serde::Deserialize,
)]
#[repr(isize)]
pub enum AIStage {
    Setup,
    Planning,
    ActionPlanning,
    Execution,
}
impl Into<RelativeStage> for AIStage {
    fn into(self) -> RelativeStage {
        RelativeStage(Stage::AI, self as isize)
    }
}

pub fn bundle(
    world: &mut World,
    resources: &mut Resources,
    builder: &mut DispatcherBuilder,
) -> Result<(), failure::Error> {
    builder.add_system(AIStage::Setup, task::build_update_task_cache_system);
    builder.add_thread_local_fn(AIStage::Planning, utility::build_scoring_system);
    builder.add_thread_local_fn(AIStage::Execution, bt::system);
    builder.add_system(AIStage::Execution, movement::build_process_movement_system);

    builder.add_system(Stage::End, task::build_cleanup_virtual_tasks);

    needs::bundle(world, resources, builder)
}
