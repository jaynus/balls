use crate::{ActiveReactionComponent, BeginReactionEvent, ReactionEffect, ReactionResult};
use rl_core::{
    components::{Destroy, PositionComponent},
    data::Target,
    defs::reaction::{ReactionDefinition, Reagent},
    event::Channel,
    fxhash::FxHashMap,
    garbage_collector::DestroyEvent,
    map::{spatial::StaticSpatialMap, tile::TileKind, Map},
    math::Vec3i,
    GameStateRef,
};

// TODO: we just delete foliage for now. do we want to allow debris products?
fn handle_foliage(position: &PositionComponent, state: &GameStateRef) {
    let channel = state.resources.get::<Channel<DestroyEvent>>().unwrap();

    state
        .resources
        .get::<StaticSpatialMap>()
        .unwrap()
        .locate_all_at_point(position)
        .for_each(|entry| {
            channel
                .write(DestroyEvent::new(entry.entity, Destroy::default()))
                .unwrap();
        });
}

#[derive(Default)]
pub struct TileChannelEffect;
impl ReactionEffect for TileChannelEffect {
    fn name() -> &'static str {
        "TileChannelEffect"
    }

    fn tick(
        &mut self,
        state: GameStateRef,
        _reaction: &ReactionDefinition,
        _component: &ActiveReactionComponent,
        event: &BeginReactionEvent,
        _entities: &FxHashMap<Reagent, Target>,
    ) -> ReactionResult {
        let target_entity = event.target.entity();

        let position = state
            .world
            .get_component::<PositionComponent>(target_entity)
            .unwrap();

        {
            let target_coord = **position;
            let mut map = state.resources.get_mut::<Map>().unwrap();

            map.writer()
                .make_empty(target_coord)
                .make_ramp(
                    Vec3i::new(target_coord.x, target_coord.y, target_coord.z + 1),
                    TileKind::RampUpWest,
                )
                .finish();
        }

        handle_foliage(&position, &state);

        ReactionResult::Success
    }
}

#[derive(Default)]
pub struct TileDigEffect;
impl ReactionEffect for TileDigEffect {
    fn name() -> &'static str {
        "TileDigEffect"
    }

    fn tick(
        &mut self,
        state: GameStateRef,
        _reaction: &ReactionDefinition,
        _component: &ActiveReactionComponent,
        event: &BeginReactionEvent,
        _entities: &FxHashMap<Reagent, Target>,
    ) -> ReactionResult {
        let target_entity = event.target.entity();

        let position = state
            .world
            .get_component::<PositionComponent>(target_entity)
            .unwrap();
        {
            let mut map = state.resources.get_mut::<Map>().unwrap();

            let target_coord = **position;

            map.writer()
                .make_floor(Vec3i::new(target_coord.x, target_coord.y, target_coord.z))
                .finish();
        }

        handle_foliage(&position, &state);

        ReactionResult::Success
    }
}
