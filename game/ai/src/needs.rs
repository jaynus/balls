use rl_core::{
    components::NeedsComponent,
    dispatcher::{DispatcherBuilder, Stage},
    legion::prelude::*,
    smallvec::SmallVec,
    time::Time,
};

pub fn bundle(
    _: &mut World,
    _: &mut Resources,
    builder: &mut DispatcherBuilder,
) -> Result<(), anyhow::Error> {
    builder.add_system(Stage::Logic, build_apply_decay_system);

    Ok(())
}

pub fn build_apply_decay_system(_: &mut World, _: &mut Resources) -> Box<dyn Schedulable> {
    SystemBuilder::<()>::new("apply_decay_system")
        .read_resource::<Time>()
        .with_query(<Write<NeedsComponent>>::query())
        .build(move |_, world, time, needs_query| {
            game_metrics::scope!("apply_decay_system");

            for mut needscomp in needs_query.iter_mut(world) {
                for need in needscomp.iter_mut() {
                    // Check lifetime on decays, remove if ended
                    let mut remove_decay = SmallVec::<[usize; 3]>::default();
                    for (n, decay) in need.decays.iter().enumerate() {
                        if decay.start + decay.lifetime.as_secs_f64() >= time.world_time {
                            remove_decay.push(n);
                        }
                    }

                    // Apply decays
                    for decay in &mut need.decays {
                        decay.acc += time.world_delta.as_secs_f64();

                        while decay.acc > decay.frequency.as_secs_f64() {
                            if let Some(val) = need.value.checked_add(decay.value) {
                                need.value = val.max(decay.minmax.0).min(decay.minmax.1);
                            } else {
                                need.value = decay.minmax.1;
                            }

                            decay.acc -= decay.frequency.as_secs_f64();
                        }
                    }
                }
            }
        })
}
