use crate::{ActiveReactionComponent, BeginReactionEvent, ReactionEffect, ReactionResult};
use rl_core::{
    components::NeedsComponent,
    defs::{
        foliage::{FoliageComponent, FoliageDefinition},
        item::{ItemComponent, ItemDefinition},
        material::{MaterialComponent, MaterialDefinition},
        needs::{NeedKind, Nutrition, ProvidesNutrition},
        reaction::{ReactionDefinition, Reagent},
        DefinitionComponent, DefinitionStorage,
    },
    fxhash::FxHashMap,
    legion::prelude::*,
    GameStateRef,
};

#[derive(Default)]
pub struct ConsumeEdibleEffect;
impl ReactionEffect for ConsumeEdibleEffect {
    fn name() -> &'static str {
        "ConsumeEdibleEffect"
    }

    fn tick(
        &mut self,
        state: GameStateRef,
        _reaction: &ReactionDefinition,
        _component: &ActiveReactionComponent,
        event: &BeginReactionEvent,
        entities: &FxHashMap<Reagent, Entity>,
    ) -> ReactionResult {
        if entities.is_empty() {
            panic!("wut");
        }

        for (_reagent_entry, entity) in entities.iter() {
            let nutrition = get_nutrition(state, *entity).unwrap();

            let mut needs_comp = unsafe {
                state
                    .world
                    .get_component_mut_unchecked::<NeedsComponent>(
                        event.initiator.unwrap().entity(),
                    )
                    .unwrap()
            };

            needs_comp.add(NeedKind::Calories, nutrition.calories.start);
            needs_comp.add(NeedKind::Hydration, nutrition.hydration.start);
        }

        ReactionResult::Success
    }
}

pub fn get_nutrition(state: GameStateRef, entity: Entity) -> Option<Nutrition> {
    let (items, materials, foliages) = <(
        Read<DefinitionStorage<ItemDefinition>>,
        Read<DefinitionStorage<MaterialDefinition>>,
        Read<DefinitionStorage<FoliageDefinition>>,
    )>::fetch(state.resources);

    let item_comp = state.world.get_component::<ItemComponent>(entity);
    let foliage_comp = state.world.get_component::<FoliageComponent>(entity);

    let optional_nutrition = if let Some(comp) = &item_comp {
        &comp.fetch(&items).nutrition
    } else if let Some(comp) = &foliage_comp {
        &comp.fetch(&foliages).nutrition
    } else {
        return None;
    };

    let nutrition = match &optional_nutrition {
        ProvidesNutrition::FromMaterial => {
            if item_comp.is_some() {
                if let Some(material_comp) = state.world.get_component::<MaterialComponent>(entity)
                {
                    let _item = item_comp.unwrap().fetch(&items);
                    let material_state = material_comp.fetch_state(&materials);

                    &material_state.nutrition
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        ProvidesNutrition::Value(value) => value,
    };

    Some(nutrition.clone())
}
