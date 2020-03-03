use crate::{
    bt::{
        BehaviorArgs, BehaviorHandle, BehaviorRoot, BehaviorStatus, BehaviorStorage,
        BehaviorTreeComponent,
    },
    iaus::{
        decisions::{DecisionHandle, DecisionStorage},
        Decision,
    },
    SensesComponent,
};
use rl_core::{
    components::{BlackboardComponent, NeedsComponent},
    failure,
    legion::prelude::*,
    map::Map,
    time::Time,
    GameStateRef, Logging,
};

pub fn prepare(world: &mut World, resources: &mut Resources) -> Result<(), failure::Error> {
    crate::iaus::decisions::prepare(world, resources)?;

    Ok(())
}

#[derive(Clone)]
pub struct UtilityStateComponent {
    pub available: Vec<DecisionEntry>,
    pub idle: usize,
    pub current: usize,
}
impl UtilityStateComponent {
    pub fn new(idle: usize, avialable: Vec<DecisionEntry>) -> Self {
        Self {
            current: idle,
            idle,
            available: avialable,
        }
    }

    #[inline]

    pub fn current(&self) -> &DecisionEntry {
        &self.available[self.current]
    }

    #[inline]
    pub fn current_mut(&mut self) -> &mut DecisionEntry {
        &mut self.available[self.current]
    }
}

pub struct UtilityState<'a> {
    pub map: &'a Map,
    pub senses: &'a SensesComponent,
    pub needs: &'a NeedsComponent,
}
impl<'a> UtilityState<'a> {
    pub fn new(map: &'a Map, senses: &'a SensesComponent, needs: &'a NeedsComponent) -> Self {
        Self { map, senses, needs }
    }
}

#[derive(Clone)]
pub struct DecisionEntry {
    pub decision: DecisionHandle,
    pub last_score: f64,

    pub last_tick: f64,
    pub frequency: f64,

    pub behavior: Option<BehaviorHandle>,
}
impl DecisionEntry {
    pub fn with_behavior(
        behavior: BehaviorHandle,
        decision: DecisionHandle,
        frequency: f64,
    ) -> Self {
        Self {
            behavior: Some(behavior),
            decision,
            frequency,
            last_score: 0.0,
            last_tick: 0.0,
        }
    }

    pub fn new(decision: DecisionHandle, frequency: f64) -> Self {
        Self {
            decision,
            frequency,
            last_score: 0.0,
            last_tick: 0.0,
            behavior: None,
        }
    }
}

#[allow(unused_variables)]
pub fn build_scoring_system(
    world: &mut World,
    resources: &mut Resources,
) -> Box<dyn FnMut(&mut World, &mut Resources)> {
    let mut sorted_cache = Vec::with_capacity(1024);

    let query = <(
        Write<UtilityStateComponent>,
        Read<SensesComponent>,
        Read<NeedsComponent>,
        Write<BehaviorTreeComponent>,
        Write<BlackboardComponent>,
    )>::query();

    let mut command_buffer = CommandBuffer::new(world);

    Box::new(move |world, resources| {
        game_metrics::scope!("iaus_scoring_system");

        let (log, time, map, behavior_storage, decision_storage) = <(
            Read<Logging>,
            Read<Time>,
            Read<Map>,
            Read<BehaviorStorage>,
            Read<DecisionStorage>,
        )>::fetch(&resources);

        let entities = unsafe {
            query
                .iter_entities_unchecked(world)
                .map(|(entity, (_, _, _, _, _))| entity)
                .collect::<Vec<_>>()
        };

        for entity in entities {
            {
                let senses = world.get_component::<SensesComponent>(entity).unwrap();
                let needs = world.get_component::<NeedsComponent>(entity).unwrap();
                let mut utility =
                    unsafe { world.get_component_mut_unchecked::<UtilityStateComponent>(entity) }
                        .unwrap();
                let mut behavior_tree =
                    unsafe { world.get_component_mut_unchecked::<BehaviorTreeComponent>(entity) }
                        .unwrap();
                let mut blackboard =
                    unsafe { world.get_component_mut_unchecked::<BlackboardComponent>(entity) }
                        .unwrap();

                utility.available.iter_mut().for_each(|entry| {
                    if time.world_time - entry.last_tick > entry.frequency {
                        entry.last_tick = time.world_time;
                        entry.last_score = decision_storage
                            .get(entry.decision)
                            .unwrap()
                            .score(&UtilityState::new(&map, &senses, &needs));
                    }
                });

                // If a behavior is currently running and not cancellable, we should continue
                if (!behavior_tree.root.is_none()
                    && behavior_tree.last_status.is_cancellable()
                    && !behavior_tree.root.is_forced())
                    || behavior_tree.root.is_none()
                {
                    sorted_cache.extend(
                        utility
                            .available
                            .iter()
                            .enumerate()
                            .map(|(n, entry)| (n, entry.last_score)),
                    );

                    sorted_cache.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                    let mut current = utility.current;

                    for n in &sorted_cache {
                        let choice = &utility.available[n.0];

                        if choice.behavior == behavior_tree.root.handle() {
                            break;
                        } else {
                            if let Some(handle) = choice.behavior {
                                let behavior =
                                    behavior_storage.get(choice.behavior.unwrap()).unwrap();

                                let result = behavior.eval(
                                    GameStateRef { world, resources },
                                    &mut BehaviorArgs {
                                        entity,
                                        senses: &senses,
                                        tree: &behavior_tree,
                                        blackboard: &mut blackboard,
                                        command_buffer: &mut command_buffer,
                                    },
                                );

                                if result == BehaviorStatus::running(false) {
                                    current = n.0;
                                    behavior_tree.root = choice
                                        .behavior
                                        .map_or(BehaviorRoot::None, BehaviorRoot::Decision);
                                } else {
                                    continue;
                                }
                            }

                            break;
                        }
                    }
                    utility.current = current;

                    sorted_cache.clear();
                }
            }
            command_buffer.write(world)
        }
    })
}
