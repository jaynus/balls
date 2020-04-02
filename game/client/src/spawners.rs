use rl_ai::HasTasksComponent;
use rl_core::defs::{
    body::*, building::*, creature::*, foliage::*, item::*, material::*, race::*, workshop::*, *,
};
use rl_core::{
    components::*, data::SpawnEvent, event::Channel, legion::prelude::*, map::Map, time::Time,
    transform::Translation,
};
use rl_render_pod::sprite::{SpriteLayer, StaticSpriteTag};

pub use rl_core::data::{SpawnArguments, SpawnKind, Target, TargetPosition};

pub trait Spawnable {
    fn spawn(
        &self,
        resources: &Resources,
        command_buffer: &mut CommandBuffer,
        target: Target,
        kind: &SpawnArguments,
    ) -> Result<Vec<Entity>, anyhow::Error>;
}

impl Spawnable for WorkshopDefinition {
    fn spawn(
        &self,
        resources: &Resources,
        command_buffer: &mut CommandBuffer,
        target: Target,
        kind: &SpawnArguments,
    ) -> Result<Vec<Entity>, anyhow::Error> {
        let map = resources.get::<Map>().unwrap();

        let material = if let SpawnArguments::Workshop { material } = kind {
            *material
        } else {
            panic!("Wrong kind to spawner")
        };

        let (world, tile) = target.from_map(&map);

        Ok(command_buffer
            .insert(
                (WorkshopTag, BuildingTag, SpriteLayer::Building),
                vec![(
                    EntityMeta::new(resources.get::<Time>().unwrap().stamp()),
                    Translation(world),
                    WorkshopComponent::new(self.id),
                    material,
                    PositionComponent::new(tile),
                    DimensionsComponent::with_tiles(self.building.dimensions),
                    ItemContainerComponent::default(),
                    HasTasksComponent::default(),
                    BuildingComponent::new(self.building.id()),
                    self.building.sprite.make(),
                )],
            )
            .to_vec())
    }
}

impl Spawnable for ItemDefinition {
    fn spawn(
        &self,
        resources: &Resources,
        command_buffer: &mut CommandBuffer,
        target: Target,
        kind: &SpawnArguments,
    ) -> Result<Vec<Entity>, anyhow::Error> {
        let map = resources.get::<Map>().unwrap();

        let material = if let SpawnArguments::Item { material } = kind {
            *material
        } else {
            panic!("Wrong kind to spawner")
        };

        let result = match target {
            Target::None => panic!(),
            Target::Entity(target_entity) => {
                let entities = command_buffer
                    .insert(
                        (ItemTag, SpriteLayer::Item),
                        vec![(
                            ItemContainerChildComponent {
                                parent: target_entity,
                            },
                            ItemComponent::new(self.id),
                            HasTasksComponent::default(),
                        )],
                    )
                    .to_vec();
                let item_entity = entities[0];

                command_buffer.exec_mut(move |world| {
                    world
                        .get_component_mut::<ItemContainerComponent>(target_entity)
                        .unwrap()
                        .push(item_entity);
                });

                entities
            }
            Target::Position(pos) => {
                let (world, tile) = pos.from_map(&map);

                command_buffer
                    .insert(
                        (ItemTag, SpriteLayer::Item),
                        vec![(
                            EntityMeta::new(resources.get::<Time>().unwrap().stamp()),
                            Translation(world),
                            ItemComponent::new(self.id),
                            material,
                            PositionComponent::new(tile),
                            DimensionsComponent::default(),
                            self.sprite.make(),
                        )],
                    )
                    .to_vec()
            }
        };

        if let Some(ItemExtension::Container { capacity }) =
            self.get_extension(ItemExtensionKind::Container)
        {
            let mut container = ItemContainerComponent::default();
            container.capacity = *capacity;
            command_buffer.add_component::<ItemContainerComponent>(result[0], container);
        }

        Ok(result)
    }
}

impl Spawnable for FoliageDefinition {
    fn spawn(
        &self,
        resources: &Resources,
        command_buffer: &mut CommandBuffer,
        target: Target,
        kind: &SpawnArguments,
    ) -> Result<Vec<Entity>, anyhow::Error> {
        let map = resources.get::<Map>().unwrap();

        let mut dimensions = if let SpawnArguments::Foliage { dimensions } = kind {
            if let Some(dimensions) = dimensions {
                *dimensions
            } else {
                DimensionsComponent::new(self.dimensions)
            }
        } else {
            panic!("Wrong kind to spawner")
        };

        dimensions.collision = self.collision;

        Ok(match target {
            Target::None => panic!(),
            Target::Entity(_) => panic!("We cant spawn foliage on an entity!"),
            Target::Position(pos) => {
                let (_, tile) = pos.from_map(&map);

                command_buffer
                    .insert(
                        (
                            StaticTag,
                            StaticSpriteTag(self.sprite.make()),
                            SpriteLayer::Foliage,
                            FoliageTag(self.kind),
                        ),
                        vec![(
                            EntityMeta::new(resources.get::<Time>().unwrap().stamp()),
                            Translation(map.tile_to_world(tile)),
                            dimensions,
                            MaterialComponent::new(self.material.id(), MaterialState::Solid),
                            PositionComponent::new(tile),
                            FoliageComponent::new(self.id),
                        )],
                    )
                    .to_vec()
            }
        })
    }
}

impl Spawnable for CreatureDefinition {
    fn spawn(
        &self,
        resources: &Resources,
        command_buffer: &mut CommandBuffer,
        target: Target,
        kind: &SpawnArguments,
    ) -> Result<Vec<Entity>, anyhow::Error> {
        use rl_ai::{
            bt::{BehaviorStorage, BehaviorTreeComponent},
            iaus::decisions::DecisionStorage,
            task::TaskPrioritiesComponent,
            SensesComponent,
        };
        use rl_core::petgraph::visit::IntoNodeReferences;
        use rl_render_pod::sprite::SparseSpriteArray;

        let (races, bodies, behaviors, decisions, map) = <(
            Read<DefinitionStorage<RaceDefinition>>,
            Read<DefinitionStorage<BodyDefinition>>,
            Read<BehaviorStorage>,
            Read<DecisionStorage>,
            Read<Map>,
        )>::fetch(&resources);

        let name = if let SpawnArguments::Creature { name } = kind {
            name
        } else {
            panic!("Wrong kind to spawner")
        };

        let (world, tile) = target.from_map(&map);

        let race = self.race.fetch(&races).unwrap();
        let body = race.body.fetch(&bodies).unwrap();

        let utilitycomp = rl_ai::utility::UtilityStateComponent::new(
            0,
            vec![
                rl_ai::utility::DecisionEntry::with_behavior(
                    behaviors.get_handle("idle").unwrap(),
                    decisions.get_handle("idle").unwrap(),
                    0.5,
                ),
                rl_ai::utility::DecisionEntry::with_behavior(
                    behaviors.get_handle("try_graze").unwrap(),
                    decisions.get_handle("hunger").unwrap(),
                    0.5,
                ),
            ],
        );

        // TODO: make this a body spawner
        let carry = CarryComponent {
            limbs: body
                .graph
                .node_references()
                .filter_map(|(idx, part)| {
                    if part.flags.contains(PartFlag::MANIPULATE) {
                        Some((idx, None))
                    } else {
                        None
                    }
                })
                .collect(),
        };

        let entities = command_buffer
            .insert(
                (CreatureTag, SpriteLayer::Creature),
                vec![(
                    CreatureComponent::new(self.id),
                    RaceComponent::new(self.race.id()),
                    EntityMeta::new(resources.get::<Time>().unwrap().stamp()),
                    BodyComponent::new(body.id(), &bodies),
                    carry,
                    Translation(world),
                    PositionComponent::new(tile),
                    BlackboardComponent::default(),
                    MovementComponent::default(),
                    DimensionsComponent::default(),
                    race.sprite.make(),
                    SparseSpriteArray::default(),
                    BehaviorTreeComponent::default(),
                    utilitycomp,
                    SensesComponent::default(),
                    NeedsComponent::default(),
                    TaskPrioritiesComponent::default(),
                )],
            )
            .to_vec();

        if let Some(name) = name {
            command_buffer.add_component(entities[0], NameComponent::new(name));
        }

        Ok(entities)
    }
}

#[derive(Default)]
pub struct Pawn;
impl Spawnable for Pawn {
    fn spawn(
        &self,
        resources: &Resources,
        command_buffer: &mut CommandBuffer,
        target: Target,
        kind: &SpawnArguments,
    ) -> Result<Vec<Entity>, anyhow::Error> {
        use rl_ai::{
            bt::{BehaviorStorage, BehaviorTreeComponent},
            iaus::decisions::DecisionStorage,
            task::TaskPrioritiesComponent,
            SensesComponent,
        };
        use rl_core::petgraph::visit::IntoNodeReferences;
        use rl_render_pod::sprite::SparseSpriteArray;

        let (races, bodies, behaviors, decisions, map) = <(
            Read<DefinitionStorage<RaceDefinition>>,
            Read<DefinitionStorage<BodyDefinition>>,
            Read<BehaviorStorage>,
            Read<DecisionStorage>,
            Read<Map>,
        )>::fetch(&resources);

        let args = if let SpawnArguments::Pawn { arguments } = kind {
            arguments
        } else {
            panic!("Wrong kind to spawner")
        };

        let (world, tile) = target.from_map(&map);

        let utilitycomp = rl_ai::utility::UtilityStateComponent::new(
            0,
            vec![
                rl_ai::utility::DecisionEntry::with_behavior(
                    behaviors.get_handle("idle").unwrap(),
                    decisions.get_handle("idle").unwrap(),
                    0.5,
                ),
                rl_ai::utility::DecisionEntry::with_behavior(
                    behaviors.get_handle("try_eat").unwrap(),
                    decisions.get_handle("hunger").unwrap(),
                    0.5,
                ),
                rl_ai::utility::DecisionEntry::with_behavior(
                    behaviors.get_handle("try_drink").unwrap(),
                    decisions.get_handle("thirst").unwrap(),
                    0.5,
                ),
                rl_ai::utility::DecisionEntry::with_behavior(
                    behaviors.get_handle("do_work").unwrap(),
                    decisions.get_handle("work").unwrap(),
                    0.5,
                ),
            ],
        );

        let race = races.get(args.race).unwrap();
        let body = race.body.fetch(&bodies).unwrap();

        // TODO: make this a body spawner
        let carry = CarryComponent {
            limbs: body
                .graph
                .node_references()
                .filter_map(|(idx, part)| {
                    if part.flags.contains(PartFlag::MANIPULATE) {
                        Some((idx, None))
                    } else {
                        None
                    }
                })
                .collect(),
        };

        Ok(command_buffer
            .insert(
                (PawnTag, SpriteLayer::Pawn),
                vec![(
                    RaceComponent::new(args.race),
                    EntityMeta::new(resources.get::<Time>().unwrap().stamp()),
                    NameComponent::new(&args.name),
                    BodyComponent::new(body.id(), &bodies),
                    carry,
                    Translation(world),
                    PositionComponent::new(tile),
                    BlackboardComponent::default(),
                    MovementComponent::default(),
                    DimensionsComponent::default(),
                    race.sprite.make(),
                    SparseSpriteArray::default(),
                    BehaviorTreeComponent::default(),
                    utilitycomp,
                    SensesComponent::default(),
                    NeedsComponent::default(),
                    TaskPrioritiesComponent::default(),
                )],
            )
            .to_vec())
    }
}

pub fn build_system(
    world: &mut World,
    resources: &mut Resources,
) -> Box<dyn FnMut(&mut World, &mut Resources)> {
    let listener = {
        if resources.contains::<Channel<SpawnEvent>>() {
            let mut channel = resources.get_mut::<Channel<SpawnEvent>>().unwrap();
            channel.bind_listener(128)
        } else {
            let mut channel = Channel::<SpawnEvent>::default();
            let listener = channel.bind_listener(128);
            resources.insert(channel);
            listener
        }
    };

    let mut command_buffer = CommandBuffer::new(world);

    let mut spawned_entities = Vec::default();

    Box::new(move |world, resources| {
        let (channel, items, _, creatures, foliages) = <(
            Read<Channel<SpawnEvent>>,
            Read<DefinitionStorage<ItemDefinition>>,
            Read<DefinitionStorage<WorkshopDefinition>>,
            Read<DefinitionStorage<CreatureDefinition>>,
            Read<DefinitionStorage<FoliageDefinition>>,
        )>::fetch(&resources);

        while let Some(event) = channel.read(listener) {
            let kind = SpawnKind::from(&event.kind);
            spawned_entities.extend(match kind {
                SpawnKind::Creature => creatures
                    .get(event.id.into())
                    .unwrap()
                    .spawn(resources, &mut command_buffer, event.target, &event.kind)
                    .unwrap(),
                SpawnKind::Pawn => Pawn::default()
                    .spawn(resources, &mut command_buffer, event.target, &event.kind)
                    .unwrap(),
                SpawnKind::Item => items
                    .get(event.id.into())
                    .unwrap()
                    .spawn(resources, &mut command_buffer, event.target, &event.kind)
                    .unwrap(),
                SpawnKind::Foliage => foliages
                    .get(event.id.into())
                    .unwrap()
                    .spawn(resources, &mut command_buffer, event.target, &event.kind)
                    .unwrap(),
                _ => unimplemented!(),
            });
        }
        spawned_entities.clear();
        command_buffer.write(world);
    })
}
