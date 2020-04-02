use crate::{GlPass, RenderArgs, RenderContext};
use glow::HasContext;

use rl_core::{app, winit::event_loop::EventLoop, GameState, Manager};

type PassConstructor =
    Box<dyn FnOnce(&mut RenderArgs<'_>) -> Result<Box<dyn GlPass>, anyhow::Error>>;

pub struct RenderManagerBuilder {
    pass_constructors: Vec<PassConstructor>,
}
impl RenderManagerBuilder {
    pub fn new() -> Result<Self, anyhow::Error> {
        Ok(Self {
            pass_constructors: Vec::default(),
        })
    }

    pub fn with_pass<F>(mut self, f: F) -> Self
    where
        F: 'static + FnOnce(&mut RenderArgs<'_>) -> Result<Box<dyn GlPass>, anyhow::Error>,
    {
        self.pass_constructors.push(Box::new(f));
        self
    }

    pub fn build(
        self,
        event_loop: &EventLoop<()>,
        app: &mut app::ApplicationContext,
        state: &mut rl_core::GameState,
    ) -> Result<RenderManager, anyhow::Error> {
        let Self { pass_constructors } = self;

        let mut context = RenderContext::new(event_loop, app, state)?;

        let mut args = RenderArgs {
            render_context: &mut context,
            app_context: app,
            state,
        };

        let mut passes = Vec::new();
        for constructor in pass_constructors {
            passes.push((constructor)(&mut args)?);
        }

        unsafe {
            context.gl.clear_color(0.1, 0.2, 0.3, 1.0);
        }

        Ok(RenderManager { context, passes })
    }
}

pub struct RenderManager {
    context: RenderContext,
    passes: Vec<Box<dyn GlPass>>,
}

impl RenderManager {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        _context: RenderContext,
        _passes: Vec<Box<dyn GlPass>>,
    ) -> Result<Self, anyhow::Error> {
        unimplemented!()
    }

    unsafe fn draw(
        &mut self,
        app_context: &mut app::ApplicationContext,
        state: &mut GameState,
    ) -> Result<(), anyhow::Error> {
        let context = &mut self.context;

        context.gl.clear(glow::COLOR_BUFFER_BIT);

        {
            for pass in &mut self.passes {
                pass.draw(RenderArgs {
                    render_context: context,
                    app_context,
                    state,
                })?;
            }
        }

        // Swap buffer
        context.context.swap_buffers()?;

        Ok(())
    }
}
impl Manager for RenderManager {
    fn tick(
        &mut self,
        app_context: &mut app::ApplicationContext,
        state: &mut GameState,
    ) -> Result<(), anyhow::Error> {
        unsafe { self.draw(app_context, state) }
    }
}
