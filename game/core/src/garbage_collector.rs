use crate::{components::Destroy, event::Channel, legion::prelude::*, time::Time};

struct DestroyMeta {
    pub frame: u64,
}

#[derive(Copy, Clone)]
pub struct DestroyEvent {
    entity: Entity,
    parameters: Destroy,
}
impl DestroyEvent {
    pub fn new(entity: Entity, parameters: Destroy) -> Self {
        Self { entity, parameters }
    }
}

pub fn build(_: &mut World, resources: &mut Resources) -> Box<dyn Schedulable> {
    let mut channel = Channel::<DestroyEvent>::default();
    let listener_id = channel.bind_listener(256);

    resources.insert(channel);

    SystemBuilder::<()>::new("garbage_collection_system")
        .read_resource::<Time>()
        .read_resource::<Channel<DestroyEvent>>()
        .with_query(<Read<Destroy>>::query().filter(!component::<DestroyMeta>()))
        .with_query(<Read<DestroyMeta>>::query())
        .build(
            move |command_buffer, world, (time, channel), (queue_query, delete_query)| {
                // Collect and apply everything from the destroy channel
                for event in channel.read(listener_id) {
                    println!("Destroy queued, waiting {} frames", event.parameters.delay);
                    command_buffer.add_component(event.entity, event.parameters);
                    command_buffer.add_component(
                        event.entity,
                        DestroyMeta {
                            frame: event.parameters.delay + time.frame,
                        },
                    )
                }

                for (entity, destroy) in queue_query.iter_entities_mut(world) {
                    println!("Destroy queued, waiting {} frames", destroy.delay);
                    command_buffer.add_component(
                        entity,
                        DestroyMeta {
                            frame: destroy.delay + time.frame,
                        },
                    )
                }
                for (entity, destroy) in delete_query.iter_entities_mut(world) {
                    if destroy.frame >= time.frame {
                        println!("Destroying: {:?}", entity);
                        command_buffer.delete(entity);
                    }
                }
            },
        )
}
