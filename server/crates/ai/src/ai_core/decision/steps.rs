//! Steps of `decide_profile_inner` kept out of mod.rs: the decision's last orders and its trace.

use super::defense::DefensivePanic;
use super::*;

/// What the tech and Barracks step takes from the decision so far.
pub(super) struct TechInput<'a> {
    pub(super) facts: &'a AiFacts,
    pub(super) profile: &'a AiProfile,
    pub(super) required_tech_path: &'a [EntityKind],
    pub(super) production_builders: &'a [&'a [u32]],
    pub(super) production_policy: ProductionPolicy,
    pub(super) spend_plan: Option<&'a spend_plan::SpendPlan>,
    pub(super) defensive_panic: DefensivePanic,
    pub(super) target_steel_workers: usize,
    pub(super) delay_opening_barracks: bool,
    pub(super) expansion_blocks_tech_path: bool,
    pub(super) hold_tech_for_expansion: bool,
    pub(super) save_for_unplanned_expansion: bool,
    pub(super) natural_ordered_or_up: bool,
    pub(super) rifle_factory_free: bool,
    pub(super) banked: bool,
    pub(super) build_search: ai_shared::BuildSearch,
}

/// Places the next building of the required tech path, then a Barracks when the curve calls for
/// one (see the gates below).
pub(super) fn build_tech<F>(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    input: TechInput<'_>,
    placeable: &mut F,
) where
    F: FnMut(EntityKind, u32, u32) -> bool,
{
    let TechInput {
        facts,
        profile,
        required_tech_path,
        production_builders,
        production_policy,
        spend_plan,
        defensive_panic,
        target_steel_workers,
        delay_opening_barracks,
        expansion_blocks_tech_path,
        hold_tech_for_expansion,
        save_for_unplanned_expansion,
        natural_ordered_or_up,
        rifle_factory_free,
        banked,
        build_search,
    } = input;
    for kind in required_tech_path {
        if *kind == EntityKind::Barracks && delay_opening_barracks {
            continue;
        }
        if turtle_should_delay_tech_for_entrenchment(profile, memory, facts, *kind) {
            continue;
        }
        if (expansion_blocks_tech_path || hold_tech_for_expansion)
            && !(*kind == EntityKind::Factory
                && (jeff_v1_1::factory_alongside_natural(profile.id, facts) || rifle_factory_free))
            && !(*kind == EntityKind::EngineeringComplex
                && jeff_v1_1::complex_alongside_natural(facts))
        {
            continue;
        }
        if facts.building_count(*kind) + planned_in_intents(intents, *kind) > 0 {
            continue;
        }
        // Jeff's AI V1.1 (`ecafter`): no Factory before its opening order is decided, no
        // Engineering Complex before a Factory, and with the Factory first none before the
        // Factory's first Scout Car is ordered.
        if let Some(opening) = facts.v1_1_opening {
            if (*kind == EntityKind::Factory && opening == OpeningOrder::Undecided)
                || (*kind == EntityKind::EngineeringComplex
                    && (facts.building_count(EntityKind::Factory) == 0
                        || (opening == OpeningOrder::FactoryFirst && !facts.v1_1_car_ordered)))
            {
                continue;
            }
        }
        // The Rifleman opening (`riflenat`): the Factory after the natural's order, the
        // Engineering Complex after the Factory's first Scout Car.
        if jeff_v1_1::rifle_opening(profile.id)
            && ((*kind == EntityKind::Factory && !natural_ordered_or_up && !rifle_factory_free)
                || (*kind == EntityKind::EngineeringComplex && !facts.v1_1_car_ordered))
        {
            continue;
        }
        if let Some(build_action) = try_build_production(
            observation,
            facts,
            actions,
            production_builders,
            profile,
            *kind,
            build_search,
            (map_analysis, &memory.tank_micro),
            &mut *placeable,
        ) {
            if *kind == EntityKind::Factory {
                if let Some(enemy_base) = facts.nearest_public_enemy_base {
                    let defensive_machine_gunners =
                        defensive_machine_gunner_units_for_build_clearance(observation, profile);
                    if let Some(units) = relocate_machine_gunners_from_factory_site(
                        observation,
                        actions,
                        (build_action.tile_x, build_action.tile_y),
                        &defensive_machine_gunners,
                        enemy_base,
                    ) {
                        // Clearing a construction footprint is a tactical move, not a new
                        // staging assignment. The live adapter suppresses repeated staging
                        // commands for units that are already in position.
                        intents.push(AiIntent::Move { units });
                    }
                }
            }
            intents.push(AiIntent::Build { kind: *kind });
        }
    }

    let target_barracks = if defensive_panic.active {
        defensive_panic_barracks_target(defensive_panic)
    } else {
        active_barracks_curve(profile).target(
            observation.economy.steel,
            facts.worker_count,
            target_steel_workers,
        )
    };
    let target_barracks = turtle_barracks_target(profile, facts, target_barracks);
    let target_barracks =
        jeff_v1_1::third_base::barracks_target(profile.id, banked, target_barracks);
    let target_barracks =
        jeff_v1_1::rifle_barracks_target(profile.id, observation, facts, memory, target_barracks);
    // The Rifleman opening's second Barracks is a step of its spend plan, paid for there.
    let planned_barracks =
        jeff_v1_1::rifle_opening(profile.id) && facts.building_count(EntityKind::Barracks) < 2;
    let surplus_barracks_enabled = profile.surplus_steel_production.is_some_and(|policy| {
        let (barracks_steel, _) = rts_rules::economy::cost(EntityKind::Barracks);
        let reserve = jeff_v1_1::surplus_reserve(profile.id, policy.reserve);
        actions.budget().steel() >= reserve.saturating_add(barracks_steel)
    });
    // The Rifleman opening's second Barracks goes up whatever the armored production policy
    // trains: from the Training Centre it lists only Tanks and Scout Cars, and the Barracks then
    // waited for 600 Steel of surplus (1:45-1:48).
    if (production_uses_building(production_policy, EntityKind::Barracks)
        || surplus_barracks_enabled
        || planned_barracks)
        && !delay_opening_barracks
        && facts.building_count(EntityKind::Barracks)
            + planned_in_intents(intents, EntityKind::Barracks)
            < target_barracks
        && !expansion_blocks_tech_path
        && !save_for_unplanned_expansion
        && planned_in_intents(intents, EntityKind::Barracks) == 0
        && (facts.building_count(EntityKind::Barracks) == 0
            || planned_barracks
            || spend_plan.is_none_or(|plan| {
                plan.affords_off_plan(intents, rts_rules::economy::cost(EntityKind::Barracks).0)
            }))
        && try_build_production(
            observation,
            facts,
            actions,
            production_builders,
            profile,
            EntityKind::Barracks,
            build_search,
            (map_analysis, &memory.tank_micro),
            &mut *placeable,
        )
        .is_some()
    {
        intents.push(AiIntent::Build {
            kind: EntityKind::Barracks,
        });
    }
}

/// What the production loop takes from the decision so far.
pub(super) struct TrainInput<'a> {
    pub(super) facts: &'a AiFacts,
    pub(super) profile: &'a AiProfile,
    pub(super) production_policy: &'a ProductionPolicy,
    pub(super) effective_unit_priorities: &'a [EntityKind],
    pub(super) production_unit_counts: &'a [(EntityKind, usize)],
    pub(super) production_max_counts: &'a [(EntityKind, usize)],
    pub(super) spend_plan: Option<&'a spend_plan::SpendPlan>,
    pub(super) security_rifle_target: Option<usize>,
    pub(super) emergency_rifle_target: Option<usize>,
    pub(super) early_rifles: Option<usize>,
    pub(super) rifle_line: Option<usize>,
    pub(super) rifle_guns: Option<usize>,
    pub(super) rifle_opening: bool,
    pub(super) panic: bool,
    pub(super) save_for_unplanned_expansion: bool,
    pub(super) save_for_first_tech_unit: bool,
    pub(super) save_for_required_tech_building: bool,
    pub(super) later_base_rally: Option<(f32, f32)>,
}

/// Trains units at every production building, in production order, within the caps and savings
/// the decision has set.
pub(super) fn train_units(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    intents: &mut Vec<AiIntent>,
    input: TrainInput<'_>,
) {
    let TrainInput {
        facts,
        profile,
        production_policy,
        effective_unit_priorities,
        production_unit_counts,
        production_max_counts,
        spend_plan,
        security_rifle_target,
        emergency_rifle_target,
        early_rifles,
        rifle_line,
        rifle_guns,
        rifle_opening,
        panic,
        save_for_unplanned_expansion,
        save_for_first_tech_unit,
        save_for_required_tech_building,
        later_base_rally,
    } = input;
    for building_kind in production_building_order(effective_unit_priorities) {
        let buildings = facts.production_buildings(building_kind);
        if buildings.is_empty() {
            continue;
        }
        let key_tech_unit = production_policy
            .save_for_first_tech_unit
            .unwrap_or(EntityKind::Worker);
        let security_recruits =
            security_rifle_target.filter(|_| building_kind == EntityKind::Barracks);
        let emergency_recruits =
            emergency_rifle_target.filter(|_| building_kind == EntityKind::Barracks);
        let early_recruits = early_rifles.filter(|_| building_kind == EntityKind::Barracks);
        let save_for_tech = security_recruits.is_none()
            && emergency_recruits.is_none()
            && early_recruits.is_none()
            && (save_for_unplanned_expansion
                || (save_for_first_tech_unit
                    && !planned_train_in_intents(intents, key_tech_unit))
                || save_for_required_tech_building)
            && !rts_rules::economy::trainable_units(building_kind).contains(&key_tech_unit)
            && !can_train_pre_tank_defensive_machine_gunner(profile, facts, building_kind)
            // The Rifleman opening's Barracks do not save: their cap below leaves the Steel the
            // spend plan's steps, the next base and the next Tank need.
            && !(rifle_opening && building_kind == EntityKind::Barracks);
        // Jeff's AI V1.1 trains Riflemen from the Steel past the first Tank's while that Tank waits.
        let rifles_only = save_for_tech
            && save_for_first_tech_unit
            && !save_for_unplanned_expansion
            && !save_for_required_tech_building
            && jeff_v1_1::rifles_past_first_tank(
                profile.id,
                building_kind,
                actions.budget().steel(),
            );
        let save_for_tech = save_for_tech && !rifles_only;
        let mut building_max_counts = production_max_counts.to_vec();
        if let Some(policy) = profile
            .surplus_steel_production
            .filter(|policy| producer_for_unit(policy.unit) == Some(building_kind))
        {
            let current = production_unit_counts
                .iter()
                .find_map(|(kind, count)| (*kind == policy.unit).then_some(*count))
                .unwrap_or(0);
            let (unit_steel, _) = rts_rules::economy::cost(policy.unit);
            let affordable_above_reserve = if unit_steel == 0 {
                0
            } else {
                let reserve = jeff_v1_1::surplus_reserve(profile.id, policy.reserve);
                actions.budget().steel().saturating_sub(reserve) as usize / unit_steel as usize
            };
            let surplus_cap = jeff::surplus_cap(profile.id, policy.unit, observation);
            building_max_counts.retain(|(kind, _)| *kind != policy.unit);
            building_max_counts.push((
                policy.unit,
                current
                    .saturating_add(affordable_above_reserve)
                    .min(surplus_cap)
                    .max(security_recruits.unwrap_or(0)),
            ));
        }
        for target in [
            emergency_recruits,
            rifle_line.filter(|_| building_kind == EntityKind::Barracks),
            early_recruits,
        ]
        .into_iter()
        .flatten()
        {
            opening_rush::raise_rifle_cap(&mut building_max_counts, target);
        }
        jeff::cap_routine_riflemen(
            spend_plan,
            building_kind == EntityKind::Barracks
                && !panic
                && emergency_recruits.is_none()
                && early_recruits.is_none(),
            intents,
            production_unit_counts,
            &mut building_max_counts,
        );
        // The guns' Riflemen are no routine Riflemen: the spend plan never held the guns back.
        if let Some(guns) = rifle_guns.filter(|_| building_kind == EntityKind::Barracks) {
            let base = rifle_line
                .unwrap_or(0)
                .max(security_rifle_target.unwrap_or(0));
            jeff_v1_1::rifle_cap_for_guns(guns, base, &mut building_max_counts);
        }
        // The Rifleman opening: no Rifleman ceiling, only the Steel left after the spend plan's
        // steps (before the first Tank) or a Tank's Steel (after it); the recruit targets stand.
        if rifle_opening && building_kind == EntityKind::Barracks {
            let cap = jeff_v1_1::rifle_opening_cap(
                spend_plan,
                intents,
                production_unit_counts,
                actions.budget().steel(),
                facts.unit_count(EntityKind::Tank) > 0,
            )
            .max(security_recruits.unwrap_or(0))
            .max(emergency_recruits.unwrap_or(0))
            .max(early_recruits.unwrap_or(0));
            building_max_counts.retain(|(kind, _)| *kind != EntityKind::Rifleman);
            building_max_counts.push((EntityKind::Rifleman, cap));
        }
        let home_holds_tank_reserve = !uses_current_jeffs_ai_policy(profile.id)
            || later_bases::main_tank_ids(observation, memory).len() >= memory.home_tank_reserve();
        let production_rally = is_jeffs_ai_profile(profile.id)
            .then(|| jeffs_production_rally(observation, facts))
            .flatten();
        let rifleman_rally = uses_home_rifle_coverage(profile.id)
            .then(|| jeffs_rifleman_home_rally(observation, facts))
            .flatten();
        let push_rally = jeff_v1_1::push::reinforcement_rally(memory);
        let limits = (production_unit_counts, building_max_counts.as_slice());
        let rifle_priority = [EntityKind::Rifleman];
        let priorities: &[EntityKind] = if rifles_only {
            &rifle_priority
        } else {
            effective_unit_priorities
        };
        jeff_v1_1::reserve_tank_steel(profile.id, building_kind, facts, observation, actions);
        for (buildings, unit_priorities) in
            jeff_v1_1::barracks_batches(profile.id, building_kind, buildings, priorities, limits)
        {
            let trained_units = actions::train_units_with_rally_for_unit(
                actions,
                TrainUnitsRequest {
                    buildings: &buildings,
                    unit_priorities: &unit_priorities,
                    completed_building_kinds: facts.complete_building_kinds(),
                    completed_upgrades: facts.completed_upgrades(),
                    max_queue_depth: production_policy.queue_depth,
                    save_for_tech,
                    current_counts: production_unit_counts,
                    max_counts: &building_max_counts,
                    balance_unit_priorities: production_policy.balance_unit_priorities,
                },
                |unit| {
                    // While a new base is being taken, fresh Tanks and Riflemen join its guards, but
                    // Tanks only once the main holds its reserve.
                    if let Some((x, y)) = later_base_rally.filter(|_| {
                        unit == EntityKind::Rifleman
                            || (unit == EntityKind::Tank && home_holds_tank_reserve)
                    }) {
                        return Some((x, y, RallyKind::AttackMove));
                    }
                    // Jeff's AI V1.1's new Tanks join its push where it holds.
                    if let Some((x, y)) = push_rally.filter(|_| unit == EntityKind::Tank) {
                        return Some((x, y, RallyKind::AttackMove));
                    }
                    if unit == EntityKind::Rifleman {
                        rifleman_rally
                            .map(|(x, y)| (x, y, RallyKind::Move))
                            .or_else(|| {
                                production_rally.map(|(x, y)| (x, y, RallyKind::AttackMove))
                            })
                    } else {
                        production_rally.map(|(x, y)| (x, y, RallyKind::AttackMove))
                    }
                },
            );
            for trained in trained_units {
                memory.note_turtle_train(profile, trained.unit);
                intents.push(AiIntent::Train { kind: trained.unit });
            }
        }
    }
}

/// The last orders each decision, over every other: Jeff's AI V1.1's Riflemen hunt Panzerfausts,
/// the current Jeff's resting Tanks gather (V1.1: on its stations), and V1.1's Tank raid, smoke
/// cover, denial watch, charges and reach discipline. `held`: the route line's reserved units and
/// those local defense took; `blockers`: the forward defensive Tank and the units on the next
/// base's footprint.
pub(super) fn late_orders(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile: &AiProfile,
    (route_line_reserved, local_defense_assigned): (&BTreeSet<u32>, &BTreeSet<u32>),
    (forward_defensive_tank, expansion_footprint_blockers): (Option<u32>, &[u32]),
) {
    // Jeff's AI V1.1's Riflemen hunt the enemy's Panzerfausts, over every other order.
    let reserved = route_line_reserved;
    jeff_v1_1::hunt::hunt_panzerfausts(actions, observation, memory, intents, profile.id, reserved);
    actions.mark("hunt");

    // Last: the current Jeff's resting Tanks with no other orders gather on the home post, so
    // none sits idle in a corner of the main or out on the map.
    if uses_current_jeffs_ai_policy(profile.id) {
        let mut excluded: BTreeSet<u32> = local_defense_assigned.clone();
        excluded.extend(forward_defensive_tank);
        excluded.extend(route_line_reserved.iter().copied());
        excluded.extend(expansion_footprint_blockers.iter().copied());
        excluded.extend(memory.jeff_v1_1.tank_raid.units());
        excluded.extend(memory.jeff_v1_1.front_line.units());
        excluded.extend(memory.jeff_v1_1.home_guard.units());
        // Jeff's AI V1.1 stations them at its bases instead (`stations`).
        if jeff_v1_1::stations::active(profile.id) {
            let live = (&mut *actions, observation, map_analysis, &mut *intents);
            jeff_v1_1::stations::station(live, memory, &excluded);
        } else {
            let gathered = jeff::gather_resting_tanks(actions, observation, memory, excluded);
            if !gathered.is_empty() {
                intents.push(AiIntent::Move { units: gathered });
            }
        }
    }
    actions.mark("gather");
    // Jeff's AI V1.1's Riflemen charge Tanks shelling its bases, and its seen Tanks keep out of
    // reach of more enemy Tanks, over every other order.
    // Jeff's AI V1.1's Tank party raids the enemy's outlying bases.
    let live = (&mut *actions, observation, map_analysis, &mut *intents);
    jeff_v1_1::tank_raid::raid(live, memory, profile.id);
    actions.mark("raid");
    jeff_v1_1::smoke_cover::cover_push_retreat(actions, observation, memory);
    actions.mark("smokecover");
    let raiders: BTreeSet<u32> = memory.jeff_v1_1.tank_raid.units().collect();
    let held = &(route_line_reserved | &raiders);
    jeff_v1_1::deny::watch(actions, observation, memory, intents, held);
    actions.mark("deny");
    let raiders: BTreeSet<u32> = memory.jeff_v1_1.tank_raid.units().collect();
    let reserved = &(route_line_reserved | &raiders);
    jeff_v1_1::charge::charge_tanks(actions, observation, memory, intents, profile.id, reserved);
    actions.mark("charge");
    jeff_v1_1::reveal::keep_out_of_reach(
        actions,
        observation,
        memory,
        intents,
        profile.id,
        reserved,
    );
    actions.mark("reveal");
}

/// Home defense's staging when local defense did not take over: the current Jeff's home Riflemen,
/// the Turtle's choke line, and the home guns (Jeff's AI V1.1 `mainfront`: on the main line's
/// wings). `assigned`: the units local defense took and the route line's reserved ones.
pub(super) fn stage_home_defense(
    (actions, observation, map_analysis, intents): opening_rush::LiveStep<'_, '_, '_, '_>,
    memory: &mut AiDecisionMemory,
    profile: &AiProfile,
    facts: &AiFacts,
    handled_local_defense: bool,
    assigned: (&BTreeSet<u32>, &BTreeSet<u32>),
    defensive_machine_gunners_available: &[u32],
) {
    let local_defense_assigned = assigned.0;
    let turtle_defense_active = profile.turtle_defense.is_some();
    if !handled_local_defense && is_jeffs_ai_profile(profile.id) && profile.home_anti_tank.is_some()
    {
        let live = (&mut *actions, observation, map_analysis, &mut *intents);
        jeff::stage_home_riflemen(live, memory, profile, facts, assigned);
    }

    if !handled_local_defense && turtle_defense_active {
        if let Some(policy) = profile.turtle_defense {
            if let Some(units) = stage_turtle_choke_defense(
                actions,
                observation,
                map_analysis,
                policy,
                local_defense_assigned,
            ) {
                intents.push(AiIntent::Stage { units });
            }
        }
    }

    if !handled_local_defense
        && !turtle_defense_active
        && !defensive_machine_gunners_available.is_empty()
    {
        // Jeff's AI V1.1 (`mainfront`): the home guns hold the main line's wings.
        let main_line_guns = (is_jeff_v1_1(profile.id) && !jeff_v1_1::off("mainfront"))
            .then(|| {
                jeff_v1_1::base_line::stage_main(
                    actions,
                    observation,
                    map_analysis,
                    memory,
                    defensive_machine_gunners_available,
                    true,
                )
            })
            .flatten();
        if let Some(units) = main_line_guns {
            if !units.is_empty() {
                intents.push(AiIntent::Stage { units });
            }
        } else if let Some(enemy_base) = facts.nearest_public_enemy_base {
            let staged = if uses_current_jeff_defense(profile.id) {
                stage_defensive_pocket_machine_gunners(
                    actions,
                    observation,
                    map_analysis,
                    defensive_machine_gunners_available,
                    enemy_base,
                    profile
                        .defensive_machine_gunners
                        .map_or(0, |policy| policy.target_count),
                )
            } else if memory.home_defensive_tank.is_some() {
                let distance = profile
                    .defensive_machine_gunners
                    .map(|policy| policy.perimeter_distance_tiles)
                    .unwrap_or(6.0)
                    + profile
                        .home_anti_tank
                        .map(|policy| policy.machine_gunner_screen_tiles)
                        .unwrap_or(0.0);
                stage_home_machine_gunner_screen(
                    actions,
                    observation,
                    map_analysis,
                    defensive_machine_gunners_available,
                    enemy_base,
                    distance,
                    profile
                        .home_anti_tank
                        .map(|policy| policy.lateral_spacing_tiles)
                        .unwrap_or(4.5),
                )
            } else {
                stage_defensive_machine_gunner_perimeter(
                    actions,
                    observation,
                    map_analysis,
                    profile,
                    defensive_machine_gunners_available,
                    enemy_base,
                )
            };
            if let Some(units) = staged {
                intents.push(AiIntent::Stage { units });
            }
        }
    }
}

/// The decision trace's spend plan, push and Tank lines, and Jeff's AI V1.1's notes.
pub(super) fn finish_trace(
    decision: &mut AiDecision,
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    profile: &AiProfile,
    spend_plan: Option<&spend_plan::SpendPlan>,
) {
    decision.trace.plan = spend_plan.map(|plan| plan.trace_line(observation));
    decision.trace.push = match (
        jeff_v1_1::push::trace_line(observation, map_analysis, memory),
        jeff_v1_1::tank_raid::trace_line(memory),
    ) {
        (Some(push), Some(raid)) => Some(format!("{push} | {raid}")),
        (push, raid) => push.or(raid),
    };
    tank_micro::finish(decision, observation, map_analysis, memory);
    if memory.profile_id().is_some_and(is_jeff_v1_1) {
        // Which system holds each Tank, for the decision trace: two short lines, under the
        // trace's line cap.
        let tank_ids: BTreeSet<u32> = observation
            .owned
            .iter()
            .filter(|unit| unit.kind == EntityKind::Tank)
            .map(|unit| unit.id)
            .collect();
        let tanks_of = |ids: &mut dyn Iterator<Item = u32>| {
            ids.filter(|id| tank_ids.contains(id))
                .collect::<BTreeSet<u32>>()
        };
        let intent_units = |attack: bool| {
            tanks_of(
                &mut decision
                    .intents
                    .iter()
                    .filter_map(|intent| match intent {
                        AiIntent::Attack { units } if attack => Some(units.iter().copied()),
                        AiIntent::Stage { units } if !attack => Some(units.iter().copied()),
                        _ => None,
                    })
                    .flatten(),
            )
        };
        let first = format!(
            "v1_1_roles line={:?} guard={:?} push={:?} home={:?}",
            tanks_of(&mut memory.jeff_v1_1.front_line.units()),
            tanks_of(&mut memory.jeff_v1_1.home_guard.units()),
            memory.containment.active_tanks,
            memory.home_defensive_tank,
        );
        let second = format!(
            "v1_1_roles2 attack={:?} stage={:?} {}",
            intent_units(true),
            intent_units(false),
            memory.jeff_v1_1.front_line.trace(),
        );
        decision.trace.notes.push(first);
        decision.trace.notes.push(second);
        decision.trace.notes.push(memory.expansion_security.trace());
        decision
            .trace
            .notes
            .extend(jeff_v1_1::resume::trace_line(memory));
        decision.trace.notes.push(memory.jeff_v1_1.natclear.trace());
        decision.trace.notes.push(memory.jeff_v1_1.breakout.trace());
        if jeff_v1_1::stations::active(profile.id) {
            decision.trace.stations = Some(format!(
                "{} call={} launched={}",
                memory.jeff_v1_1.stations.trace(),
                memory.jeff_v1_1.push.call_trace(),
                memory.containment.wave_launched
            ));
        }
        decision
            .trace
            .notes
            .push(jeff_v1_1::south_route::trace(memory));
        decision.trace.strike = Some(format!(
            "v1_1_meth {} opening={:?} trade={}/{} value={}/{} ahead={}",
            memory.jeff_v1_1.meth_strike.trace(),
            memory.jeff_v1_1.opening,
            memory.jeff_v1_1.kills,
            memory.jeff_v1_1.deaths,
            memory.jeff_v1_1.value_killed,
            memory.jeff_v1_1.value_lost,
            jeff_v1_1::trading_ahead(memory)
        ));
    }
}
