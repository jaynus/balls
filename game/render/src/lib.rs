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

pub mod manager;

pub use rendy;
pub use rendy::hal;

use rl_core::{
    legion::prelude::*,
    settings::{DisplayMode, Settings},
    winit::{
        self,
        event_loop::EventLoop,
        window::{Window, WindowBuilder},
    },
    ScreenDimensions,
};
use std::sync::Arc;

pub struct RenderContext<B: hal::Backend> {
    _marker: std::marker::PhantomData<B>,
}
impl<B: hal::Backend> RenderContext<B> {
    pub fn new(
        event_loop: &EventLoop<()>,
        _app: &mut rl_core::app::ApplicationContext,
        state: &mut rl_core::GameState,
    ) -> Result<Self, anyhow::Error> {
        let window_builder = {
            let settings = state.resources.get::<Settings>().unwrap();

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

            builder
        };

        unimplemented!()
    }
}
