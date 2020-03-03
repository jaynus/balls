#![deny(clippy::pedantic, clippy::all)]
#![allow(
    clippy::must_use_candidate,
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    dead_code,
    clippy::default_trait_access,
    clippy::module_name_repetitions,
    incomplete_features
)]
#![feature(const_generics, const_fn)]

pub mod manager;
pub mod pass;
pub mod shader;

pub mod data;
pub mod utils;

pub use ash;

pub mod alloc {
    pub type AllocatorPtr = std::sync::Arc<vk_mem::Allocator>;
    pub use vk_mem::*;
}

pub struct RenderContext {
    pub vk: data::VulkanContext,
    pub allocator: alloc::AllocatorPtr,
    pub renderpass: ash::vk::RenderPass,
}

use rl_core::{
    failure,
    settings::{DisplayMode, Settings},
    winit::{self, event_loop::EventLoop, window::WindowBuilder},
    GameState, ScreenDimensions,
};
use std::sync::Arc;

pub fn build_context(
    settings: &Settings,
    event_loop: &EventLoop<()>,
    game_state: &mut GameState,
) -> Result<(), failure::Error> {
    let mut builder = WindowBuilder::new().with_title(settings.window_title.clone());

    match settings.display_mode {
        DisplayMode::Fullscreen => {
            unimplemented!(
                // TODO: https://github.com/rust-windowing/winit/blob/master/examples/fullscreen.rs
            )
        }
        DisplayMode::Windowed(w, h) => {
            builder = builder.with_inner_size(winit::dpi::Size::Logical(
                winit::dpi::LogicalSize::new(f64::from(w), f64::from(h)),
            ))
        }
    }

    let window = Arc::new(builder.build(event_loop)?);

    game_state.resources.insert(ScreenDimensions {
        size: window.inner_size().to_logical(window.scale_factor()),
        dpi: window.scale_factor(),
    });

    game_state.resources.insert(window);

    Ok(())
}
