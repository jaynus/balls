#![feature(const_fn)]
#![deny(clippy::pedantic, clippy::all)]
#![allow(
    clippy::must_use_candidate,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    dead_code,
    clippy::default_trait_access,
    clippy::module_name_repetitions,
    non_camel_case_types
)]

pub mod color;
pub mod pod;
pub mod shader;
pub mod sprite;
pub use std140;

pub use shaderc;
pub use spirv_cross;
pub use spirv_reflect;
