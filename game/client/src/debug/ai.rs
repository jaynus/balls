use rl_ai::{
    bt::{BehaviorStorage, BehaviorTreeComponent},
    iaus::{decisions::DecisionStorage, Decision},
    utility::UtilityStateComponent,
};
use rl_core::{
    components::{NameComponent, NeedsComponent, PawnTag},
    defs::{
        item::{ItemDefinition, ItemProperty},
        needs::NeedKind,
        reaction::ReactionDefinition,
        Definition, DefinitionComponent, DefinitionStorage,
    },
    event::Channel,
    inventory,
    legion::prelude::*,
    strum::{AsStaticRef, IntoEnumIterator},
    Logging,
};
use rl_reaction::{BeginReactionEvent, ReactionEntity};
use rl_ui::{
    imgui::{self, im_str, Condition, ImString},
    selection::SelectionState,
    UiWindowSet,
};

pub fn build_decisions_window(world: &mut World, resources: &mut Resources) {
    #[derive(Default)]
    struct NeedsWindowState {
        selected_edible_item: usize,
    }

    let pawn_query = <(
        Read<NameComponent>,
        Read<NeedsComponent>,
        Read<UtilityStateComponent>,
        Read<BehaviorTreeComponent>,
    )>::query()
    .filter(tag::<PawnTag>());

    let non_pawn_query = <(
        TryRead<NameComponent>,
        Read<NeedsComponent>,
        Read<UtilityStateComponent>,
        Read<BehaviorTreeComponent>,
    )>::query()
    .filter(!tag::<PawnTag>());

    resources.insert(NeedsWindowState::default());

    UiWindowSet::create_with(
        world,
        resources,
        "decisionsWindow",
        true,
        move |ui, _window_manager, world, resources, _command_buffer| {
            let (
                _log,
                _selection_state,
                behavior_storage,
                decision_storage,
                item_defs,
                reaction_defs,
                mut window_state,
                reaction_channel,
            ) = unsafe {
                <(
                    Read<Logging>,
                    Read<SelectionState>,
                    Read<BehaviorStorage>,
                    Read<DecisionStorage>,
                    Read<DefinitionStorage<ItemDefinition>>,
                    Read<DefinitionStorage<ReactionDefinition>>,
                    Write<NeedsWindowState>,
                    Read<Channel<BeginReactionEvent>>,
                )>::fetch_unchecked(&resources)
            };

            imgui::Window::new(im_str!("needsWindow"))
                .size([300.0, 300.0], Condition::Always)
                .build(ui, || {
                    for (entity, (name, needs, _, _)) in pawn_query.iter_entities(&world) {
                        ui.text(&format!("Pawn: {}", name.name));

                        let mut edible_items = Vec::default();

                        inventory::for_all_items_recursive(entity, world, |_, (item, comp)| {
                            let def = comp.fetch(&item_defs);
                            if def.properties.contains(ItemProperty::IsEdible) {
                                edible_items
                                    .push((imgui::ImString::from(def.name().to_owned()), item));
                            }
                        });

                        if !edible_items.is_empty()
                            && imgui::CollapsingHeader::new(im_str!("Consume"))
                                .default_open(true)
                                .build(ui)
                        {
                            imgui::ComboBox::new(im_str!("Item##Selection")).build_simple_string(
                                ui,
                                &mut window_state.selected_edible_item,
                                edible_items
                                    .iter()
                                    .map(|(s, _)| s)
                                    .collect::<Vec<_>>()
                                    .as_slice(),
                            );

                            if ui.button(im_str!("send"), [0.0, 0.0]) {
                                // Fire off the consume reaction
                                reaction_channel
                                    .write(BeginReactionEvent::new(
                                        reaction_defs.get_id("Consume (Any)").unwrap(),
                                        Some(ReactionEntity::Pawn(entity)),
                                        ReactionEntity::Any(
                                            edible_items[window_state.selected_edible_item].1,
                                        ),
                                        None,
                                    ))
                                    .unwrap();
                            }
                        }

                        let columns = ImString::new(format!("PawnNeeds_{}", name.name).as_str());
                        ui.columns(3, &columns, true);
                        ui.text("Need");
                        ui.next_column();
                        ui.text("Score");
                        ui.next_column();
                        ui.text("Decays");
                        ui.next_column();
                        for need in NeedKind::iter() {
                            let need_state = needs.get(need);
                            ui.text(need.as_static());
                            ui.next_column();
                            ui.text(&format!("{:.2}", need_state.value));
                            ui.next_column();
                            ui.text(&format!("{:?}", need_state.decays));
                            ui.next_column();
                        }

                        ui.columns(1, im_str!(""), false);
                    }
                });

            imgui::Window::new(im_str!("decisionsWindow"))
                .size([300.0, 300.0], Condition::Always)
                .build(ui, || {
                    ui.columns(3, im_str!("balls2"), true);
                    ui.set_current_column_width(50.0);
                    ui.text("Pawn");
                    ui.next_column();
                    ui.set_current_column_width(50.0);
                    ui.text("Active Decision");
                    ui.next_column();
                    ui.set_current_column_width(200.0);
                    ui.text("Active Behavior");
                    ui.next_column();

                    for (name, _, utility, bt) in pawn_query.iter(&world) {
                        ui.set_current_column_width(50.0);
                        ui.text(&name.name);
                        ui.next_column();

                        ui.set_current_column_width(50.0);
                        ui.text(
                            decision_storage
                                .get(utility.current().decision)
                                .unwrap()
                                .name(),
                        );
                        ui.next_column();

                        ui.set_current_column_width(200.0);
                        {
                            if let Some(behavior_handle) = bt.root.handle() {
                                let behavior_name =
                                    behavior_storage.get_name(behavior_handle).unwrap();
                                ui.text(&format!("{}({})", bt.root.as_ref(), behavior_name));
                            } else {
                                ui.text(&"None".to_string());
                            }
                        }
                        ui.next_column();
                    }
                    for (name, _, utility, bt) in non_pawn_query.iter(&world) {
                        ui.set_current_column_width(50.0);
                        if let Some(name) = name {
                            ui.text(&name.name);
                        } else {
                            ui.text("NONAME");
                        }
                        ui.next_column();

                        ui.set_current_column_width(50.0);
                        ui.text(
                            decision_storage
                                .get(utility.current().decision)
                                .unwrap()
                                .name(),
                        );
                        ui.next_column();

                        ui.set_current_column_width(200.0);
                        {
                            if let Some(behavior_handle) = bt.root.handle() {
                                let behavior_name =
                                    behavior_storage.get_name(behavior_handle).unwrap();
                                ui.text(&format!("{}({})", bt.root.as_ref(), behavior_name));
                            } else {
                                ui.text(&"None".to_string());
                            }
                        }
                        ui.next_column();
                    }
                    ui.columns(1, im_str!(""), false);

                    if imgui::CollapsingHeader::new(im_str!("Decision State"))
                        .default_open(true)
                        .build(ui)
                    {
                        for (name, _, utility, _) in pawn_query.iter(&world) {
                            ui.text(&format!("Pawn: {}", name.name));

                            let columns =
                                ImString::new(format!("PawnDecisions_{}", name.name).as_str());
                            ui.columns(3, &columns, true);
                            ui.text("Decision");
                            ui.next_column();
                            ui.text("Score");
                            ui.next_column();
                            ui.text("Tick");
                            ui.next_column();
                            for entry in &utility.available {
                                ui.text(decision_storage.get(entry.decision).unwrap().name());
                                ui.next_column();
                                ui.text(&format!("{:.3}", entry.last_score));
                                ui.next_column();
                                ui.text(&format!("{:.3}", entry.last_tick));
                                ui.next_column();
                            }
                            ui.columns(1, im_str!(""), false);
                        }
                        for (name, _, utility, _) in non_pawn_query.iter(&world) {
                            let name = if let Some(name) = name {
                                name.name.clone()
                            } else {
                                "NONAME".to_string()
                            };

                            ui.text(&format!("Non-Pawn: {}", name));
                            let columns = ImString::new(format!("PawnDecisions_{}", name).as_str());
                            ui.columns(3, &columns, true);
                            ui.text("Decision");
                            ui.next_column();
                            ui.text("Score");
                            ui.next_column();
                            ui.text("Tick");
                            ui.next_column();
                            for entry in &utility.available {
                                ui.text(decision_storage.get(entry.decision).unwrap().name());
                                ui.next_column();
                                ui.text(&format!("{:.3}", entry.last_score));
                                ui.next_column();
                                ui.text(&format!("{:.3}", entry.last_tick));
                                ui.next_column();
                            }
                            ui.columns(1, im_str!(""), false);
                        }
                    }
                });

            true
        },
    );
}
