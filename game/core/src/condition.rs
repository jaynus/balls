use crate::defs::{
    common::Property,
    condition::{Condition, ConditionSet, ConditionSetRef, Item, Operator, Tile, Value},
    foliage::FoliageKind,
    DefinitionStorage,
};
use legion::prelude::*;

pub struct CheckConditionParameters<'a> {
    world: &'a World,
    resources: &'a Resources,
    entity: Entity,
    target: Option<Entity>,
    sets: &'a [ConditionSetRef],
}

pub fn check_tile(property: &Tile, state: &CheckConditionParameters) -> bool {
    unimplemented!()
}

pub fn check_foliage(property: &FoliageKind, state: &CheckConditionParameters) -> bool {
    unimplemented!()
}

pub fn check_item(property: &Item, state: &CheckConditionParameters) -> bool {
    unimplemented!()
}

pub fn check_property(property: &Property, state: &CheckConditionParameters) -> bool {
    unimplemented!()
}

pub fn check_condition<'a, 'p>(
    condition: &'a Condition,
    state: &'p CheckConditionParameters,
) -> Result<(), &'a Condition> {
    if match &condition.value {
        Value::Tile(value) => check_tile(value, state),
        Value::Item(value) => check_item(value, state),
        Value::Foliage(value) => check_foliage(value, state),
        Value::Property(value) => check_property(value, state),
    } {
        Ok(())
    } else {
        Err(condition)
    }
}

pub fn check_set<'a, 'p>(
    set: &'a ConditionSet,
    state: &'p CheckConditionParameters,
) -> Result<(), &'a Condition> {
    let left_res = check_condition(&set.left, state);
    if let Some(right) = set.right.as_ref() {
        match right.op {
            Operator::And => {
                if left_res.is_ok() {
                    check_set(&right.set, state)?;
                } else {
                    return left_res;
                }
            }
            Operator::Or => {
                if left_res.is_err() {
                    check_set(&right.set, state)?;
                } else {
                    return left_res;
                }
            }
            _ => panic!("invalid op for set"),
        }
    }

    Ok(())
}

pub fn check<'a, 's>(
    world: &'a World,
    resources: &'a Resources,
    entity: Entity,
    target: Option<Entity>,
    sets: &'s [ConditionSetRef],
) -> Result<(), &'s Condition> {
    let state = CheckConditionParameters {
        world,
        resources,
        entity,
        target,
        sets,
    };
    for set in sets {
        check_set(&set, &state)?;
    }

    unimplemented!()
}

/*
#[allow(clippy::trivially_copy_pass_by_ref)]
fn check_item_materials(
    _state: GameStateRef,
    _source: Entity,
    _item: &ItemComponent,
    _limits: &[MaterialLimit],
) -> bool {
    true
}

pub fn check_item_available(
    state: GameStateRef,
    source: Entity,
    item: &rl_core::defs::item::ItemRef,
    limits: &[MaterialLimit],
) -> Result<Entity, ()> {
    rl_core::inventory::find_item_recursive(source, state.world, |_, (_, comp)| {
        comp.id() == item.id() && check_item_materials(state, source, &comp, limits)
    })
    .map(|(_, item)| item)
    .ok_or(())
}

pub fn check_item_property(
    state: GameStateRef,
    source: Entity,
    property: rl_core::defs::item::ItemProperty,
    limits: &[MaterialLimit],
) -> Result<Entity, ()> {
    let item_defs = state
        .resources
        .get::<DefinitionStorage<ItemDefinition>>()
        .unwrap();

    rl_core::inventory::find_item_recursive(source, state.world, |_, (_, comp)| {
        comp.fetch(&item_defs).properties.contains(property)
            && check_item_materials(state, source, &comp, limits)
    })
    .map(|(_, item)| item)
    .ok_or(())
}

pub fn check_item_ability(
    state: GameStateRef,
    source: Entity,
    ability: rl_core::defs::item::ItemAbilityKind,
    quality: u32,
    limits: &[MaterialLimit],
) -> Result<Entity, ()> {
    let item_defs = state
        .resources
        .get::<DefinitionStorage<ItemDefinition>>()
        .unwrap();

    rl_core::inventory::find_item_recursive(source, state.world, |_, (_, comp)| {
        comp.fetch(&item_defs)
            .abilities
            .contains(&ItemAbility::new(ability, quality))
            && check_item_materials(state, source, &comp, limits)
    })
    .map(|(_, item)| item)
    .ok_or(())
}*/
