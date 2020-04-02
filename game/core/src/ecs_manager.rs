use crate::app;
use crate::{
    dispatcher::{Dispatcher, DispatcherBuilder},
    GameState, Manager,
};

type DispatcherBuilderSet<'a> = Vec<(DispatcherBuilder<'a>, Box<dyn Fn(&GameState) -> bool>)>;
type DispatcherSet = Vec<(Dispatcher, Box<dyn Fn(&GameState) -> bool>)>;

#[derive(Default)]
pub struct EcsManagerBuilder<'a> {
    dispatchers: DispatcherBuilderSet<'a>,
}
impl<'a> EcsManagerBuilder<'a> {
    pub fn add_dispatcher<F>(&mut self, builder: DispatcherBuilder<'a>, can_tick: F)
    where
        F: 'static + Fn(&GameState) -> bool,
    {
        self.dispatchers.push((builder, Box::new(can_tick)));
    }

    pub fn with_dispatcher<F>(mut self, builder: DispatcherBuilder<'a>, can_tick: F) -> Self
    where
        F: 'static + Fn(&GameState) -> bool,
    {
        self.add_dispatcher(builder, can_tick);

        self
    }

    pub fn build(
        mut self,
        _: &mut app::ApplicationContext,
        state: &mut GameState,
    ) -> Result<EcsManager, anyhow::Error> {
        let world = &mut state.world;
        let resources = &mut state.resources;

        let mut dispatchers = Vec::with_capacity(self.dispatchers.len());
        for (builder, f) in self.dispatchers.drain(..) {
            dispatchers.push((builder.build(world, resources)?, f));
        }

        Ok(EcsManager { dispatchers })
    }
}

pub struct EcsManager {
    dispatchers: DispatcherSet,
}

impl Manager for EcsManager {
    fn tick(
        &mut self,
        _context: &mut app::ApplicationContext,
        state: &mut GameState,
    ) -> Result<(), anyhow::Error> {
        for dispatcher in &mut self.dispatchers {
            if (dispatcher.1)(state) {
                let world = &mut state.world;
                let resources = &mut state.resources;

                dispatcher.0.run(world, resources);
            }
        }

        Ok(())
    }
}
