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
    clippy::new_ret_no_self,
    clippy::cast_precision_loss,
    clippy::missing_safety_doc,
    incomplete_features
)]
#![feature(const_generics, const_type_id, const_fn)]

pub mod buffer;
pub mod manager;
pub mod pass;
pub mod shader;
pub mod state;
pub mod texture;
pub mod vertex;

pub use self::{shader::ShaderProgram, state::RenderState};

pub use glow;

use rl_core::{
    winit::{event_loop::EventLoop, window::Window},
    ScreenDimensions,
};
use std::sync::Arc;

pub struct RenderContext {
    pub gl: glow::Context,
    pub context: glutin::RawContext<glutin::PossiblyCurrent>,
    pub current_state: RenderState,
    pub window: Arc<Window>,
}
impl RenderContext {
    pub fn new(
        event_loop: &EventLoop<()>,
        _app: &mut rl_core::app::ApplicationContext,
        state: &mut rl_core::GameState,
    ) -> Result<Self, anyhow::Error> {
        // Build depending on our platform?

        #[cfg(not(all(target_arch = "wasm32", feature = "web-sys")))]
        let (gl, context, window) = unsafe {
            let wb = glutin::window::WindowBuilder::new()
                .with_title("Hello triangle!")
                .with_inner_size(glutin::dpi::LogicalSize::new(1024.0, 768.0));
            let windowed_context = glutin::ContextBuilder::new()
                .with_vsync(true)
                .build_windowed(wb, event_loop)
                .unwrap();
            let windowed_context = windowed_context.make_current().unwrap();
            let gl = glow::Context::from_loader_function(|s| {
                windowed_context.get_proc_address(s) as *const _
            });

            let (context, window) = windowed_context.split();

            (gl, context, window)
        };

        state.resources.insert(ScreenDimensions {
            size: window.inner_size().to_logical(window.scale_factor()),
            dpi: window.scale_factor(),
        });
        let window = Arc::new(window);

        state.resources.insert(window.clone());

        let current_state = RenderState::save(&gl);

        Ok(Self {
            gl,
            context,
            window,
            current_state,
        })
    }
}

pub trait GlPass {
    unsafe fn draw(&mut self, args: RenderArgs<'_>) -> Result<(), anyhow::Error>;
}

pub struct RenderArgs<'a> {
    pub app_context: &'a mut rl_core::app::ApplicationContext,
    pub render_context: &'a mut RenderContext,
    pub state: &'a mut rl_core::GameState,
}
