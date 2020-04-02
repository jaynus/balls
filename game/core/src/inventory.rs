use crate::{
    components::{CarryComponent, ItemContainerComponent},
    data::DimensionsVec,
    defs::{
        body::{BodyComponent, BodyDefinition},
        item::{ItemComponent, ItemDefinition},
    },
    legion::prelude::*,
};
use std::cmp::Ordering;

pub fn can_contain_item(container: &ItemContainerComponent, item: &ItemDefinition) -> bool {
    if let Ordering::Greater =
        compare_volumes(container.consumed + item.dimensions, container.capacity)
    {
        true
    } else {
        false
    }
}

#[allow(clippy::many_single_char_names)]
pub fn compare_volumes(a: DimensionsVec, b: DimensionsVec) -> Ordering {
    let x = a.x.cmp(&b.x);
    let y = a.y.cmp(&b.y);
    let z = a.z.cmp(&b.z);

    if Ordering::Greater == x || Ordering::Greater == y || Ordering::Greater == z {
        return Ordering::Greater;
    }
    if Ordering::Equal == x && Ordering::Equal == y && Ordering::Equal == z {
        return Ordering::Greater;
    }

    Ordering::Less
}

pub fn can_carry(_body: &BodyComponent, _body_def: &BodyDefinition) {}

pub fn carry_has_item(carry: &CarryComponent, item: Entity) -> bool {
    for limb in &carry.limbs {
        if let Some(carrying) = limb.1 {
            if carrying == item {
                return true;
            }
        }
    }
    false
}

pub fn container_has_item(container: &ItemContainerComponent, item: Entity) -> bool {
    for inside in &container.inside {
        if *inside == item {
            return true;
        }
    }
    false
}

pub fn container_has_item_recursive(
    world: &World,
    container: &ItemContainerComponent,
    item: Entity,
) -> bool {
    for inside in &container.inside {
        if *inside == item {
            return true;
        }

        if let Some(subcontainer) = world.get_component::<ItemContainerComponent>(*inside) {
            if container_has_item(&subcontainer, item) {
                return true;
            }
        }
    }

    false
}

pub fn remove_item(world: &World, holder: Entity, item: Entity) -> bool {
    unsafe {
        if let Some(mut container) =
            world.get_component_mut_unchecked::<ItemContainerComponent>(holder)
        {
            if container_has_item_recursive(world, &container, item) {
                return container.remove(item);
            }
        }
        if let Some(mut carry) = world.get_component_mut_unchecked::<CarryComponent>(holder) {
            if carry_has_item(&carry, item) {
                return carry.remove(item);
            }
        }
    }

    false
}

pub fn find_item_recursive<F: FnMut(Entity, (Entity, &ItemComponent)) -> bool>(
    source: Entity,
    world: &World,
    mut f: F,
) -> Option<(Entity, Entity)> {
    for_all_items_recursive_impl(source, world, &mut f)
}

pub fn for_all_items_recursive<F: FnMut(Entity, (Entity, &ItemComponent))>(
    source: Entity,
    world: &World,
    mut f: F,
) {
    for_all_items_recursive_impl(source, world, &mut |source, (container, comp)| {
        (f)(source, (container, comp));
        false
    });
}

pub fn for_all_items_recursive_impl<F: FnMut(Entity, (Entity, &ItemComponent)) -> bool>(
    source: Entity,
    world: &World,
    f: &mut F,
) -> Option<(Entity, Entity)> {
    if let Some(carrying) = world.get_component::<CarryComponent>(source) {
        for item_entity in carrying.iter() {
            if let Some(item) = world.get_component::<ItemComponent>(item_entity) {
                if f(source, (item_entity, &item)) {
                    return Some((source, item_entity));
                }
            }
            if world.has_component::<ItemContainerComponent>(item_entity) {
                if let Some(ret) = for_all_items_recursive_impl(source, world, f) {
                    return Some(ret);
                }
            }
        }
    }
    if let Some(container) = world.get_component::<ItemContainerComponent>(source) {
        for item_entity in &container.inside {
            if let Some(item) = world.get_component::<ItemComponent>(*item_entity) {
                if f(source, (*item_entity, &item)) {
                    return Some((source, *item_entity));
                }
            }
            if world.has_component::<ItemContainerComponent>(*item_entity) {
                if let Some(ret) = for_all_items_recursive_impl(source, world, f) {
                    return Some(ret);
                }
            }
        }
    }

    None
}
