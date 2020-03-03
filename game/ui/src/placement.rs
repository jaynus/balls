use rl_core::{
    components::EntityMeta,
    event::Channel,
    input::{ActionBinding, InputActionEvent, InputState, InputStateKind},
    legion::prelude::*,
    map::Map,
    math::Vec3,
    time::Time,
    transform::Translation,
};
use rl_render_pod::sprite::Sprite;

pub fn build_placement_system(
    world: &mut World,
    resources: &mut Resources,
) -> Box<dyn FnMut(&mut World, &mut Resources)> {
    let mut default_sprite = Sprite::default();
    default_sprite.color.a = 0.0;
    let placement_entity = world.insert(
        (),
        vec![(
            EntityMeta::new(resources.get::<Time>().unwrap().stamp()),
            Translation::zero(),
            default_sprite,
        )],
    )[0];

    let listener_id = resources
        .get_mut::<Channel<InputActionEvent>>()
        .unwrap()
        .bind_listener(64);

    let clear_placement_alpha = move |world: &mut World| {
        world
            .get_component_mut::<Sprite>(placement_entity)
            .unwrap()
            .color
            .a = 0.0;
    };

    Box::new(move |world: &mut World, resources: &mut Resources| {
        let mut cancel = false;
        let mut complete = None;

        let (mut input_state, map) = <(Write<InputState>, Read<Map>)>::fetch_mut(resources);
        // Active placement request

        if input_state.state == InputStateKind::Placement {
            if let Some(placement) = input_state.placement_request() {
                *world.get_component_mut::<Sprite>(placement_entity).unwrap() =
                    placement.on_draw(world, resources);

                *world
                    .get_component_mut::<Translation>(placement_entity)
                    .unwrap() = Translation(
                    input_state.mouse_world_position
                        - Vec3::new(
                            map.sprite_dimensions.x as f32 / 2.0,
                            map.sprite_dimensions.y as f32 / 2.0,
                            0.0,
                        ),
                );

                while let Some(event) = resources
                    .get_mut::<Channel<InputActionEvent>>()
                    .unwrap()
                    .read(listener_id)
                {
                    match event {
                        InputActionEvent::Released(ActionBinding::Selection) => {
                            complete = Some((
                                input_state.mouse_world_position,
                                input_state.mouse_tile_position,
                            ));
                        }

                        InputActionEvent::Released(ActionBinding::DoAction) => {
                            cancel = true;
                        }
                        _ => {}
                    }
                }
            } else {
                clear_placement_alpha(world);
            }

            if cancel {
                clear_placement_alpha(world);
                input_state.clear_placement_request(world, resources);
            }
            if let Some((world_position, tile_position)) = complete {
                clear_placement_alpha(world);
                input_state.complete_placement_request(
                    world,
                    resources,
                    world_position,
                    tile_position,
                );
            }
        } else {
            if input_state.placement_request().is_some() {
                clear_placement_alpha(world);
                input_state.clear_placement_request(world, resources);
            }
            while let Some(_) = resources
                .get_mut::<Channel<InputActionEvent>>()
                .unwrap()
                .read(listener_id)
            {}
        }
    })
}
