use crate::{
    components::PositionComponent,
    data::{Target, TargetPosition},
    defs::{
        common::Property,
        condition::{
            Condition, ConditionSet, ConditionSetRef, Item, Nutrition, Operator, Subject, Tile,
            Value,
        },
        foliage::FoliageKind,
        Definition, DefinitionComponent, DefinitionStorage,
    },
    math::Vec3i,
};
use legion::prelude::*;
use strum::IntoEnumIterator;

pub fn check_tile(
    condition: &Condition,
    tile: &Tile,
    world: &World,
    resources: &Resources,
    source: Target,
    target: Target,
) -> bool {
    use crate::map::Map;

    if let Some(coord) = subject_position(condition.subject, world, source, target) {
        let map = resources.get::<Map>().unwrap();

        let res = match tile {
            Tile::Kind(kind) => map.get(coord).kind == *kind,
        };

        res == condition.op.into()
    } else {
        false
    }
}

pub fn check_foliage(
    condition: &Condition,
    kind: FoliageKind,
    world: &World,
    _resources: &Resources,
    source: Target,
    target: Target,
) -> bool {
    use crate::components::FoliageTag;

    let target = subject_entity(condition.subject, source, target).unwrap();

    world
        .get_tag::<FoliageTag>(target)
        .map_or(false, |tag| tag.0 == kind)
        == condition.op.into()
}

pub fn check_item(
    condition: &Condition,
    property: &Item,
    world: &World,
    resources: &Resources,
    source: Target,
    target: Target,
) -> bool {
    let items = resources
        .get::<DefinitionStorage<ItemDefinition>>()
        .unwrap();
    let materials = resources
        .get::<DefinitionStorage<MaterialDefinition>>()
        .unwrap();

    let target = subject_entity(condition.subject, source, target).unwrap();

    let check_item = |target| {
        if let Item::Material(_) = property {
            let _ = world.get_component::<MaterialComponent>(target);
            false
        } else {
            world
                .get_component::<ItemComponent>(target)
                .map_or(false, |item_comp| {
                    let item = item_comp.fetch(&items);
                    match property {
                        Item::Ability(ability) => {
                            item.abilities.iter().any(|ab| *ab == (ability).into())
                        }
                        Item::Name(name) => item.name().eq(name),
                        Item::Property(property) => item.properties.contains(*property),
                        Item::Nutrition(nut) => match nut {
                            Nutrition::Kind(kind) => {
                                let material = world.get_component::<MaterialComponent>(target);

                                if let Some(kind) = kind {
                                    item_nutrition(
                                        *kind,
                                        *item_comp,
                                        material.as_deref(),
                                        &items,
                                        &materials,
                                    )
                                    .is_some()
                                } else {
                                    NeedKind::iter().any(|kind| {
                                        item_nutrition(
                                            kind,
                                            *item_comp,
                                            material.as_deref(),
                                            &items,
                                            &materials,
                                        )
                                        .is_some()
                                    })
                                }
                            }
                        },
                        Item::Material(_) => unreachable!(),
                    }
                })
                == condition.op.into()
        }
    };

    if check_item(target) {
        true
    } else {
        crate::inventory::find_item_recursive(source.entity().unwrap(), world, |_, (item, _)| {
            check_item(item)
        })
        .is_some()
    }
}

#[allow(unused_variables)]
pub fn check_property(
    condition: &Condition,
    property: Property,
    world: &World,
    resources: &Resources,
    source: Target,
    _target: Target,
) -> bool {
    unimplemented!()
}

pub fn check_condition<'a, 's>(
    condition: &'s Condition,
    world: &'a World,
    resources: &'a Resources,
    source: Target,
    target: Target,
    skip_self: bool,
) -> Result<(), &'s Condition> {
    if skip_self && condition.subject == Subject::Me {
        return Ok(());
    }

    if match &condition.value {
        Value::Tile(value) => check_tile(condition, value, world, resources, source, target),
        Value::Item(value) => check_item(condition, value, world, resources, source, target),
        Value::Foliage(value) => check_foliage(condition, *value, world, resources, source, target),
        Value::Property(value) => {
            check_property(condition, *value, world, resources, source, target)
        }
    } {
        Ok(())
    } else {
        Err(condition)
    }
}

pub fn check_set<'a, 's>(
    set: &'s ConditionSet,
    world: &'a World,
    resources: &'a Resources,
    source: Target,
    target: Target,
    skip_self: bool,
) -> Result<(), &'s Condition> {
    let left_res = check_condition(&set.left, world, resources, source, target, skip_self);

    if let Some(right) = set.right.as_ref() {
        match right.op {
            Operator::And => {
                if left_res.is_ok() {
                    check_set(&right.set, world, resources, source, target, skip_self)?;
                } else {
                    return left_res;
                }
            }
            Operator::Or => {
                if left_res.is_err() {
                    check_set(&right.set, world, resources, source, target, skip_self)?;
                } else {
                    return left_res;
                }
            }
            _ => panic!("invalid op for set"),
        }
    }

    left_res
}

pub fn check<'a, 's>(
    world: &'a World,
    resources: &'a Resources,
    source: Target,
    target: Target,
    sets: &'s [ConditionSetRef],
    skip_self: bool,
) -> Result<(), &'s Condition> {
    for set in sets {
        check_set(&set, world, resources, source, target, skip_self)?;
    }

    Ok(())
}

impl Into<bool> for Operator {
    fn into(self) -> bool {
        match self {
            Operator::True => true,
            Operator::False => false,
            _ => unimplemented!(),
        }
    }
}

fn subject_entity(subject: Subject, source: Target, target: Target) -> Option<Entity> {
    fn entity(target: Target) -> Option<Entity> {
        if let Target::Entity(entity) = target {
            Some(entity)
        } else {
            None
        }
    }

    match subject {
        Subject::Target => entity(target),
        Subject::Me => entity(source),
        Subject::Any => unimplemented!(),
    }
}
fn subject_position(
    subject: Subject,
    world: &World,
    source: Target,
    target: Target,
) -> Option<Vec3i> {
    fn position(world: &World, target: Target) -> Option<Vec3i> {
        match target {
            Target::None => unimplemented!(),
            Target::Position(position) => match position {
                TargetPosition::Tile(pos) => Some(pos),
                TargetPosition::World(_) => unimplemented!(),
            },
            Target::Entity(entity) => world
                .get_component::<PositionComponent>(entity)
                .map(|position| **position),
        }
    }

    match subject {
        Subject::Target => position(world, target),
        Subject::Me => position(world, source),
        Subject::Any => unimplemented!(),
    }
}

use crate::defs::{
    foliage::{FoliageComponent, FoliageDefinition},
    item::{ItemComponent, ItemDefinition},
    material::{MaterialComponent, MaterialDefinition},
    needs::{NeedKind, ProvidesNutrition},
};

pub fn item_nutrition(
    kind: NeedKind,
    item: ItemComponent,
    material: Option<&MaterialComponent>,
    items: &DefinitionStorage<ItemDefinition>,
    materials: &DefinitionStorage<MaterialDefinition>,
) -> Option<std::ops::Range<i32>> {
    let def = item.fetch(&items);

    let nutrition = match &def.nutrition {
        ProvidesNutrition::FromMaterial => {
            if let Some(material_comp) = material {
                let material_state = material_comp.fetch_state(&materials);

                &material_state.nutrition
            } else {
                return None;
            }
        }
        ProvidesNutrition::Value(value) => value,
    };

    match kind {
        NeedKind::Calories => {
            if nutrition.calories.start > 0 {
                Some(nutrition.calories.clone())
            } else {
                None
            }
        }
        NeedKind::Hydration => {
            if nutrition.hydration.start > 0 {
                Some(nutrition.hydration.clone())
            } else {
                None
            }
        }
        _ => unimplemented!(),
    }
}

pub fn foliage_nutrition(
    kind: NeedKind,
    foliage: FoliageComponent,
    material: Option<&MaterialComponent>,
    materials: &DefinitionStorage<MaterialDefinition>,
    foliages: &DefinitionStorage<FoliageDefinition>,
) -> Option<std::ops::Range<i32>> {
    let def = foliage.fetch(&foliages);

    let nutrition = match &def.nutrition {
        ProvidesNutrition::FromMaterial => {
            if let Some(material_comp) = material {
                let material_state = material_comp.fetch_state(&materials);

                &material_state.nutrition
            } else {
                return None;
            }
        }
        ProvidesNutrition::Value(value) => value,
    };

    match kind {
        NeedKind::Calories => {
            if nutrition.calories.start > 0 {
                Some(nutrition.calories.clone())
            } else {
                None
            }
        }
        NeedKind::Hydration => {
            if nutrition.hydration.start > 0 {
                Some(nutrition.hydration.clone())
            } else {
                None
            }
        }
        _ => unimplemented!(),
    }
}
