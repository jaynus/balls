#![feature(const_fn)]
#![deny(clippy::pedantic, clippy::all)]
#![allow(
    clippy::must_use_candidate,
    clippy::missing_errors_doc,
    clippy::wildcard_imports,
    clippy::missing_safety_doc,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    dead_code,
    clippy::default_trait_access,
    clippy::module_name_repetitions,
    non_camel_case_types
)]

pub mod context;
pub mod factory;
pub mod resources;

const APP_NAME: &str = "RL";

pub mod alloc {
    pub type AllocatorPtr = std::sync::Arc<vk_mem::Allocator>;
    pub use vk_mem::*;
}

pub trait Destroyable {
    unsafe fn destroy(&mut self, device: &context::Context) -> Result<(), Error>;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Unknown")]
    Unknown,
}
