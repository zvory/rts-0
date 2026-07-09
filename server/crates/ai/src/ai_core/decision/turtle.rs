use super::defense::{main_steel_cluster_center, EXPANSION_DEFENSIVE_LINE_SPACING_TILES};
use super::geometry::{clamp_to_map, dist2, normalized_direction, squared, tile_center};
use super::*;
use crate::ai_core::map_analysis::{AiBaseChoke, AiMapAnalysis};
use crate::ai_core::profiles::TurtleDefensePolicy;

mod debug;
pub(super) use debug::turtle_observer_debug_layers;

const TURTLE_REISSUE_EPS_TILES: f32 = 0.75;
const TURTLE_STAFF_RADIUS_TILES: f32 = 4.0;
const TURTLE_AT_FACE_TARGET_TILES: f32 = 20.0;
const TURTLE_RIFLE_STEEL_LINE_DISTANCE_TILES: f32 = 6.0;
const TURTLE_MACHINE_GUNNER_KIND: [EntityKind; 1] = [EntityKind::MachineGunner];
const TURTLE_RIFLEMAN_KIND: [EntityKind; 1] = [EntityKind::Rifleman];

pub(super) fn turtle_machine_gunner_lines_staffed(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    policy: TurtleDefensePolicy,
) -> bool {
    if policy.machine_gunner_target_chokes == 0 || policy.machine_gunners_per_choke == 0 {
        return false;
    }
    let Some(chokes) = machine_gunner_target_chokes(observation, analysis, policy) else {
        return false;
    };
    if chokes.len() < policy.machine_gunner_target_chokes {
        return false;
    }
    chokes.iter().all(|choke| {
        count_units_near_choke(
            observation,
            *choke,
            &TURTLE_MACHINE_GUNNER_KIND,
            TurtleSlotZone::ChokeLine,
        ) >= policy.machine_gunners_per_choke
    })
}

pub(super) fn stage_turtle_choke_defense(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    policy: TurtleDefensePolicy,
    excluded_units: &BTreeSet<u32>,
) -> Option<Vec<u32>> {
    let mut staged = Vec::new();

    let riflemen = actions::select_ready_combat_units_excluding(
        &observation.owned,
        &TURTLE_RIFLEMAN_KIND,
        excluded_units,
    );
    stage_riflemen_steel_line(actions, observation, &riflemen, &mut staged);

    let Some(chokes) = prioritized_base_chokes(observation, analysis, policy) else {
        return (!staged.is_empty()).then_some(staged);
    };

    let main_ready = observation.upgrades.contains(&UpgradeKind::Entrenchment)
        && count_units_near_choke(
            observation,
            chokes[0],
            &TURTLE_MACHINE_GUNNER_KIND,
            TurtleSlotZone::ChokeLine,
        ) >= policy.main_machine_gunner_target;
    let active_choke_count = if main_ready {
        chokes.len()
    } else {
        early_choke_count(observation, chokes.len())
    };
    let active_chokes = &chokes[..active_choke_count];

    let infantry = actions::select_ready_combat_units_excluding(
        &observation.owned,
        &TURTLE_MACHINE_GUNNER_KIND,
        excluded_units,
    );
    stage_machine_gunners(
        actions,
        observation,
        active_chokes,
        &infantry,
        policy,
        &mut staged,
    );

    let anti_tank_guns = actions::select_ready_combat_units_excluding(
        &observation.owned,
        policy.anti_tank_kinds,
        excluded_units,
    );
    stage_anti_tank_guns(
        actions,
        observation,
        active_chokes,
        &anti_tank_guns,
        policy,
        &mut staged,
    );

    (!staged.is_empty()).then_some(staged)
}

fn stage_riflemen_steel_line(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    units: &[u32],
    staged: &mut Vec<u32>,
) {
    let mut riflemen = unit_ids_with_kinds(observation, units, &TURTLE_RIFLEMAN_KIND);
    riflemen.sort_unstable();
    riflemen.dedup();
    if riflemen.is_empty() {
        return;
    }

    let Some(steel_center) = main_steel_cluster_center(observation) else {
        return;
    };
    let own_start = tile_center(observation.own_start_tile, observation.map.tile_size);
    let Some(enemy_start) = nearest_enemy_start_world(observation, own_start) else {
        return;
    };
    let Some((dir_x, dir_y)) = normalized_direction(steel_center, enemy_start) else {
        return;
    };

    let tile_size = observation.map.tile_size as f32;
    if tile_size <= 0.0 {
        return;
    }
    let line_center = clamp_to_map(
        (
            steel_center.0 + dir_x * TURTLE_RIFLE_STEEL_LINE_DISTANCE_TILES * tile_size,
            steel_center.1 + dir_y * TURTLE_RIFLE_STEEL_LINE_DISTANCE_TILES * tile_size,
        ),
        observation.map,
    );
    let perpendicular = (-dir_y, dir_x);
    let spacing = EXPANSION_DEFENSIVE_LINE_SPACING_TILES * tile_size;
    let center_index = (riflemen.len().saturating_sub(1)) as f32 * 0.5;
    let units_by_id = units_by_id(observation);
    let close_enough2 = close_enough2(observation);

    for (index, unit_id) in riflemen.into_iter().enumerate() {
        let Some(unit) = units_by_id.get(&unit_id).copied() else {
            continue;
        };
        let offset = (index as f32 - center_index) * spacing;
        let target = clamp_to_map(
            (
                line_center.0 + perpendicular.0 * offset,
                line_center.1 + perpendicular.1 * offset,
            ),
            observation.map,
        );
        if dist2(unit.x, unit.y, target.0, target.1) <= close_enough2 {
            if let Some(units) = actions::hold_position_units(actions, [unit_id]) {
                staged.extend(units);
            }
            continue;
        }
        if let Some(units) = actions::attack_move_units(actions, [unit_id], target.0, target.1) {
            staged.extend(units);
        }
    }
}

fn stage_machine_gunners(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    chokes: &[AiBaseChoke],
    units: &[u32],
    policy: TurtleDefensePolicy,
    staged: &mut Vec<u32>,
) {
    let target_choke_count = policy.machine_gunner_target_chokes.min(chokes.len());
    if target_choke_count == 0 {
        return;
    }
    let chokes = &chokes[..target_choke_count];
    let machine_gunners = unit_ids_with_kinds(observation, units, &TURTLE_MACHINE_GUNNER_KIND);
    let assignments = target_count_choke_assignments(
        observation,
        chokes,
        &machine_gunners,
        &TURTLE_MACHINE_GUNNER_KIND,
        TurtleSlotZone::ChokeLine,
        policy.machine_gunners_per_choke,
    );
    let grouped = assignments_by_choke(assignments, chokes.len());
    let units_by_id = units_by_id(observation);
    let close_enough2 = close_enough2(observation);

    for (choke_index, assigned_units) in grouped.into_iter().enumerate() {
        if assigned_units.is_empty() {
            continue;
        }
        let choke = chokes[choke_index];
        let capacity = slot_capacity(
            choke,
            observation.map.tile_size,
            policy.machine_gunner_slot_gap_tiles,
        )
        .max(policy.machine_gunners_per_choke)
        .max(assigned_units.len());
        let preferred_slots = preferred_machine_gunner_slots(
            choke,
            capacity,
            observation.map,
            choke_route_slot_bias(observation, choke),
            policy.machine_gunners_per_choke,
        );
        let assigned_set: BTreeSet<u32> = assigned_units.iter().copied().collect();
        let mut used_slots = used_slots_for_choke(
            observation,
            choke,
            capacity,
            &TURTLE_MACHINE_GUNNER_KIND,
            TurtleSlotZone::ChokeLine,
            &assigned_set,
        );

        for unit_id in assigned_units {
            let Some(unit) = units_by_id.get(&unit_id).copied() else {
                continue;
            };
            let slot_index = existing_or_preferred_slot(
                unit,
                choke,
                capacity,
                TurtleSlotZone::ChokeLine,
                observation.map,
                &used_slots,
                &preferred_slots,
            );
            used_slots.insert(slot_index);
            let Some(target) = slot_world(
                choke,
                slot_index,
                capacity,
                TurtleSlotZone::ChokeLine,
                observation.map,
            ) else {
                continue;
            };
            let target = clamp_to_map(target, observation.map);
            if dist2(unit.x, unit.y, target.0, target.1) <= close_enough2 {
                if let Some(units) = actions::hold_position_units(actions, [unit_id]) {
                    staged.extend(units);
                }
                continue;
            }
            if let Some(units) = actions::attack_move_units(actions, [unit_id], target.0, target.1)
            {
                staged.extend(units);
            }
        }
    }
}

fn stage_anti_tank_guns(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    chokes: &[AiBaseChoke],
    units: &[u32],
    policy: TurtleDefensePolicy,
    staged: &mut Vec<u32>,
) {
    let own_start_world = tile_center(observation.own_start_tile, observation.map.tile_size);
    let zone = TurtleSlotZone::AntiTankEmplacement {
        back_tiles: policy.anti_tank_back_tiles,
        own_start_world,
    };
    let assignments = occupancy_weighted_choke_assignments(
        observation,
        chokes,
        units,
        policy.anti_tank_kinds,
        zone,
    );
    let grouped = assignments_by_choke(assignments, chokes.len());
    let units_by_id = units_by_id(observation);
    let close_enough2 = close_enough2(observation);

    for (choke_index, assigned_units) in grouped.into_iter().enumerate() {
        if assigned_units.is_empty() {
            continue;
        }
        let choke = chokes[choke_index];
        let capacity = slot_capacity(choke, observation.map.tile_size, policy.slot_gap_tiles)
            .max(assigned_units.len());
        let slot_bias = choke_route_slot_bias(observation, choke);
        let preferred_slots = coverage_preferred_slots(
            choke,
            capacity,
            zone,
            observation.map,
            slot_bias,
            assigned_units.len(),
        );
        let assigned_set: BTreeSet<u32> = assigned_units.iter().copied().collect();
        let mut used_slots = used_slots_for_choke(
            observation,
            choke,
            capacity,
            policy.anti_tank_kinds,
            zone,
            &assigned_set,
        );

        for unit_id in assigned_units {
            let Some(unit) = units_by_id.get(&unit_id).copied() else {
                continue;
            };
            let slot_index = existing_or_preferred_slot(
                unit,
                choke,
                capacity,
                zone,
                observation.map,
                &used_slots,
                &preferred_slots,
            );
            used_slots.insert(slot_index);
            let Some(line_point) = slot_world(
                choke,
                slot_index,
                capacity,
                TurtleSlotZone::ChokeLine,
                observation.map,
            ) else {
                continue;
            };
            let Some(emplacement) = slot_world(choke, slot_index, capacity, zone, observation.map)
            else {
                continue;
            };
            let emplacement = clamp_to_map(emplacement, observation.map);
            let face_toward = anti_tank_face_toward(
                choke,
                line_point,
                policy.anti_tank_back_tiles,
                observation.map,
                own_start_world,
            )
            .map(|point| clamp_to_map(point, observation.map))
            .unwrap_or(line_point);
            let needs_move = dist2(unit.x, unit.y, emplacement.0, emplacement.1) > close_enough2;
            if needs_move {
                if let Some(units) =
                    actions::move_units(actions, [unit_id], emplacement.0, emplacement.1)
                {
                    staged.extend(units);
                }
            }
            if let Some(units) = actions::setup_anti_tank_guns(
                actions,
                [unit_id],
                face_toward.0,
                face_toward.1,
                needs_move,
            ) {
                staged.extend(units);
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ChokeAssignment {
    unit_id: u32,
    choke_index: usize,
}

#[derive(Clone, Copy, Debug)]
enum TurtleSlotZone {
    ChokeLine,
    AntiTankEmplacement {
        back_tiles: f32,
        own_start_world: (f32, f32),
    },
}

fn occupancy_weighted_choke_assignments(
    observation: &AiObservation,
    chokes: &[AiBaseChoke],
    units: &[u32],
    kinds: &[EntityKind],
    zone: TurtleSlotZone,
) -> Vec<ChokeAssignment> {
    if units.is_empty() || chokes.is_empty() {
        return Vec::new();
    }

    let units_by_id = units_by_id(observation);
    let mut staffing = staffed_counts(observation, chokes, kinds, zone);
    let mut unit_ids = units.to_vec();
    unit_ids.sort_unstable();
    unit_ids.dedup();

    let mut assignments = Vec::new();
    for unit_id in unit_ids {
        let Some(unit) = units_by_id.get(&unit_id).copied() else {
            continue;
        };
        if let Some(choke_index) = nearest_staffed_choke(unit, chokes, zone, observation.map) {
            assignments.push(ChokeAssignment {
                unit_id,
                choke_index,
            });
            continue;
        }

        let choke_index = least_staffed_choke(chokes, &staffing);
        staffing[choke_index] = staffing[choke_index].saturating_add(1);
        assignments.push(ChokeAssignment {
            unit_id,
            choke_index,
        });
    }
    assignments
}

fn target_count_choke_assignments(
    observation: &AiObservation,
    chokes: &[AiBaseChoke],
    units: &[u32],
    kinds: &[EntityKind],
    zone: TurtleSlotZone,
    target_per_choke: usize,
) -> Vec<ChokeAssignment> {
    if units.is_empty() || chokes.is_empty() {
        return Vec::new();
    }

    let units_by_id = units_by_id(observation);
    let mut staffing = vec![0usize; chokes.len()];
    let mut unit_ids = units.to_vec();
    unit_ids.sort_unstable();
    unit_ids.dedup();

    let mut assignments = Vec::new();
    for unit_id in unit_ids {
        let Some(unit) = units_by_id.get(&unit_id).copied() else {
            continue;
        };
        if !unit_counts_for_turtle(unit, kinds) {
            continue;
        }
        if let Some(choke_index) = nearest_staffed_choke(unit, chokes, zone, observation.map) {
            if target_per_choke == 0 || staffing[choke_index] < target_per_choke {
                staffing[choke_index] = staffing[choke_index].saturating_add(1);
                assignments.push(ChokeAssignment {
                    unit_id,
                    choke_index,
                });
                continue;
            }
        }

        let choke_index = target_count_choke_for_unit(
            unit,
            chokes,
            &staffing,
            target_per_choke,
            zone,
            observation.map,
        );
        staffing[choke_index] = staffing[choke_index].saturating_add(1);
        assignments.push(ChokeAssignment {
            unit_id,
            choke_index,
        });
    }
    assignments
}

fn target_count_choke_for_unit(
    unit: &AiEntitySummary,
    chokes: &[AiBaseChoke],
    staffing: &[usize],
    target_per_choke: usize,
    zone: TurtleSlotZone,
    map: AiMapSummary,
) -> usize {
    (0..chokes.len())
        .min_by(|left, right| {
            let left_count = staffing.get(*left).copied().unwrap_or_default();
            let right_count = staffing.get(*right).copied().unwrap_or_default();
            let left_full = target_per_choke > 0 && left_count >= target_per_choke;
            let right_full = target_per_choke > 0 && right_count >= target_per_choke;
            left_full
                .cmp(&right_full)
                .then_with(|| left_count.cmp(&right_count))
                .then_with(|| {
                    unit_zone_distance2(unit, chokes[*left], zone, map)
                        .total_cmp(&unit_zone_distance2(unit, chokes[*right], zone, map))
                })
                .then_with(|| chokes[*left].id.cmp(&chokes[*right].id))
        })
        .unwrap_or(0)
}

fn assignments_by_choke(assignments: Vec<ChokeAssignment>, choke_count: usize) -> Vec<Vec<u32>> {
    let mut grouped = vec![Vec::new(); choke_count];
    for assignment in assignments {
        if let Some(group) = grouped.get_mut(assignment.choke_index) {
            group.push(assignment.unit_id);
        }
    }
    grouped
}

fn staffed_counts(
    observation: &AiObservation,
    chokes: &[AiBaseChoke],
    kinds: &[EntityKind],
    zone: TurtleSlotZone,
) -> Vec<usize> {
    let mut counts = vec![0usize; chokes.len()];
    for unit in &observation.owned {
        if !unit_counts_for_turtle(unit, kinds) {
            continue;
        }
        if let Some(choke_index) = nearest_staffed_choke(unit, chokes, zone, observation.map) {
            counts[choke_index] = counts[choke_index].saturating_add(1);
        }
    }
    counts
}

fn count_units_near_choke(
    observation: &AiObservation,
    choke: AiBaseChoke,
    kinds: &[EntityKind],
    zone: TurtleSlotZone,
) -> usize {
    observation
        .owned
        .iter()
        .filter(|unit| unit_counts_for_turtle(unit, kinds))
        .filter(|unit| {
            unit_zone_distance2(unit, choke, zone, observation.map)
                <= staff_radius2(observation.map)
        })
        .count()
}

fn unit_counts_for_turtle(unit: &AiEntitySummary, kinds: &[EntityKind]) -> bool {
    unit.is_complete && unit.state != AiEntityState::Dead && kinds.contains(&unit.kind)
}

fn nearest_staffed_choke(
    unit: &AiEntitySummary,
    chokes: &[AiBaseChoke],
    zone: TurtleSlotZone,
    map: AiMapSummary,
) -> Option<usize> {
    let radius2 = staff_radius2(map);
    chokes
        .iter()
        .enumerate()
        .filter_map(|(index, choke)| {
            let distance2 = unit_zone_distance2(unit, *choke, zone, map);
            (distance2 <= radius2).then_some((index, distance2))
        })
        .min_by(
            |(left_index, left_distance), (right_index, right_distance)| {
                left_distance
                    .total_cmp(right_distance)
                    .then_with(|| chokes[*left_index].id.cmp(&chokes[*right_index].id))
            },
        )
        .map(|(index, _)| index)
}

fn least_staffed_choke(chokes: &[AiBaseChoke], staffing: &[usize]) -> usize {
    (0..chokes.len())
        .min_by(|left, right| {
            let left_count = staffing.get(*left).copied().unwrap_or_default();
            let right_count = staffing.get(*right).copied().unwrap_or_default();
            let left_width = usize::from(chokes[*left].width_tiles.max(1));
            let right_width = usize::from(chokes[*right].width_tiles.max(1));
            left_count
                .saturating_mul(right_width)
                .cmp(&right_count.saturating_mul(left_width))
                .then_with(|| chokes[*left].id.cmp(&chokes[*right].id))
        })
        .unwrap_or(0)
}

fn used_slots_for_choke(
    observation: &AiObservation,
    choke: AiBaseChoke,
    capacity: usize,
    kinds: &[EntityKind],
    zone: TurtleSlotZone,
    ignored_unit_ids: &BTreeSet<u32>,
) -> BTreeSet<usize> {
    observation
        .owned
        .iter()
        .filter(|unit| !ignored_unit_ids.contains(&unit.id))
        .filter(|unit| unit_counts_for_turtle(unit, kinds))
        .filter(|unit| {
            unit_zone_distance2(unit, choke, zone, observation.map)
                <= staff_radius2(observation.map)
        })
        .map(|unit| nearest_slot_index(unit, choke, capacity, zone, observation.map))
        .collect()
}

fn existing_or_preferred_slot(
    unit: &AiEntitySummary,
    choke: AiBaseChoke,
    capacity: usize,
    zone: TurtleSlotZone,
    map: AiMapSummary,
    used_slots: &BTreeSet<usize>,
    preferred_slots: &[usize],
) -> usize {
    if unit_zone_distance2(unit, choke, zone, map) <= staff_radius2(map) {
        let current_slot = nearest_slot_index(unit, choke, capacity, zone, map);
        if !used_slots.contains(&current_slot) && preferred_slots.contains(&current_slot) {
            return current_slot;
        }
    }
    choose_preferred_open_slot(capacity, used_slots, preferred_slots)
}

fn choose_preferred_open_slot(
    capacity: usize,
    used_slots: &BTreeSet<usize>,
    preferred_slots: &[usize],
) -> usize {
    if let Some(slot) = preferred_slots
        .iter()
        .copied()
        .find(|slot| *slot < capacity && !used_slots.contains(slot))
    {
        return slot;
    }
    (0..capacity)
        .filter(|slot| !used_slots.contains(slot))
        .max_by(|left, right| {
            nearest_used_slot_gap(*left, used_slots, capacity)
                .cmp(&nearest_used_slot_gap(*right, used_slots, capacity))
                .then_with(|| {
                    center_distance(*right, capacity).cmp(&center_distance(*left, capacity))
                })
                .then_with(|| right.cmp(left))
        })
        .unwrap_or_else(|| {
            if capacity == 0 {
                0
            } else {
                used_slots.len() % capacity
            }
        })
}

fn preferred_machine_gunner_slots(
    choke: AiBaseChoke,
    capacity: usize,
    map: AiMapSummary,
    slot_bias: Option<(f32, f32)>,
    target_per_choke: usize,
) -> Vec<usize> {
    if capacity == 0 {
        return vec![0];
    }
    let target = target_per_choke.clamp(1, capacity);
    coverage_preferred_slots(
        choke,
        capacity,
        TurtleSlotZone::ChokeLine,
        map,
        slot_bias,
        target,
    )
}

fn coverage_preferred_slots(
    choke: AiBaseChoke,
    capacity: usize,
    zone: TurtleSlotZone,
    map: AiMapSummary,
    slot_bias: Option<(f32, f32)>,
    target_count: usize,
) -> Vec<usize> {
    if capacity == 0 {
        return vec![0];
    }

    let target = target_count.clamp(1, capacity);
    let mut selected = Vec::with_capacity(target);
    selected.push(seed_coverage_slot(choke, capacity, zone, map, slot_bias));

    while selected.len() < target {
        let used_slots: BTreeSet<usize> = selected.iter().copied().collect();
        let context = CoverageSlotContext {
            choke,
            capacity,
            zone,
            map,
            slot_bias,
            used_slots: &used_slots,
        };
        let Some(next) = (0..capacity)
            .filter(|slot| !used_slots.contains(slot))
            .max_by(|left, right| {
                compare_coverage_slots(&context, *left, *right)
            })
        else {
            break;
        };
        selected.push(next);
    }

    selected
}

fn seed_coverage_slot(
    choke: AiBaseChoke,
    capacity: usize,
    zone: TurtleSlotZone,
    map: AiMapSummary,
    slot_bias: Option<(f32, f32)>,
) -> usize {
    if capacity == 0 {
        return 0;
    }
    if let Some(bias) = slot_bias {
        return (0..capacity)
            .min_by(|left, right| {
                slot_bias_distance2(choke, *left, capacity, zone, map, bias)
                    .total_cmp(&slot_bias_distance2(
                        choke, *right, capacity, zone, map, bias,
                    ))
                    .then_with(|| left.cmp(right))
            })
            .unwrap_or(0);
    }
    (0..capacity)
        .min_by(|left, right| {
            center_distance(*left, capacity)
                .cmp(&center_distance(*right, capacity))
                .then_with(|| left.cmp(right))
        })
        .unwrap_or(0)
}

struct CoverageSlotContext<'a> {
    choke: AiBaseChoke,
    capacity: usize,
    zone: TurtleSlotZone,
    map: AiMapSummary,
    slot_bias: Option<(f32, f32)>,
    used_slots: &'a BTreeSet<usize>,
}

fn compare_coverage_slots(
    context: &CoverageSlotContext<'_>,
    left: usize,
    right: usize,
) -> std::cmp::Ordering {
    nearest_used_slot_gap(left, context.used_slots, context.capacity)
        .cmp(&nearest_used_slot_gap(
            right,
            context.used_slots,
            context.capacity,
        ))
        .then_with(|| {
            if let Some(bias) = context.slot_bias {
                let left_bias = slot_bias_distance2(
                    context.choke,
                    left,
                    context.capacity,
                    context.zone,
                    context.map,
                    bias,
                );
                let right_bias = slot_bias_distance2(
                    context.choke,
                    right,
                    context.capacity,
                    context.zone,
                    context.map,
                    bias,
                );
                right_bias.total_cmp(&left_bias)
            } else {
                center_distance(right, context.capacity)
                    .cmp(&center_distance(left, context.capacity))
            }
        })
        .then_with(|| right.cmp(&left))
}

fn slot_bias_distance2(
    choke: AiBaseChoke,
    slot: usize,
    capacity: usize,
    zone: TurtleSlotZone,
    map: AiMapSummary,
    bias: (f32, f32),
) -> f32 {
    let target = slot_world(choke, slot, capacity, zone, map)
        .or_else(|| slot_world(choke, slot, capacity, TurtleSlotZone::ChokeLine, map))
        .unwrap_or(choke.center_world);
    dist2(target.0, target.1, bias.0, bias.1)
}

fn nearest_used_slot_gap(slot: usize, used_slots: &BTreeSet<usize>, capacity: usize) -> usize {
    used_slots
        .iter()
        .map(|used| slot.abs_diff(*used))
        .min()
        .unwrap_or(capacity)
}

fn center_distance(slot: usize, capacity: usize) -> usize {
    let center2 = capacity.saturating_sub(1);
    (slot.saturating_mul(2)).abs_diff(center2)
}

fn nearest_slot_index(
    unit: &AiEntitySummary,
    choke: AiBaseChoke,
    capacity: usize,
    zone: TurtleSlotZone,
    map: AiMapSummary,
) -> usize {
    if capacity == 0 {
        return 0;
    }
    (0..capacity)
        .min_by(|left, right| {
            let left_target =
                slot_world(choke, *left, capacity, zone, map).unwrap_or(choke.center_world);
            let right_target =
                slot_world(choke, *right, capacity, zone, map).unwrap_or(choke.center_world);
            dist2(unit.x, unit.y, left_target.0, left_target.1)
                .total_cmp(&dist2(unit.x, unit.y, right_target.0, right_target.1))
                .then_with(|| left.cmp(right))
        })
        .unwrap_or(0)
}

fn slot_capacity(choke: AiBaseChoke, tile_size: u32, slot_gap_tiles: f32) -> usize {
    let gap_tiles = slot_gap_tiles.max(1.0);
    let width_tiles = choke.width_tiles.max(1) as f32;
    let by_width = (width_tiles / gap_tiles).ceil() as usize;
    let by_pixel_width =
        (choke_segment_len(choke) / (gap_tiles * tile_size as f32)).ceil() as usize;
    by_width.max(by_pixel_width).max(1)
}

fn slot_world(
    choke: AiBaseChoke,
    slot_index: usize,
    slot_count: usize,
    zone: TurtleSlotZone,
    map: AiMapSummary,
) -> Option<(f32, f32)> {
    let line_point = choke_line_slot(choke, slot_index, slot_count);
    match zone {
        TurtleSlotZone::ChokeLine => Some(line_point),
        TurtleSlotZone::AntiTankEmplacement {
            back_tiles,
            own_start_world,
        } => backed_choke_point(
            choke,
            line_point,
            back_tiles,
            map.tile_size,
            own_start_world,
        ),
    }
}

fn choke_line_slot(choke: AiBaseChoke, slot_index: usize, slot_count: usize) -> (f32, f32) {
    if slot_count <= 1 {
        return choke.center_world;
    }
    let t = (slot_index as f32 + 0.5) / slot_count as f32;
    (
        choke.endpoint_a_world.0 + (choke.endpoint_b_world.0 - choke.endpoint_a_world.0) * t,
        choke.endpoint_a_world.1 + (choke.endpoint_b_world.1 - choke.endpoint_a_world.1) * t,
    )
}

fn backed_choke_point(
    choke: AiBaseChoke,
    line_point: (f32, f32),
    back_tiles: f32,
    tile_size: u32,
    own_start_world: (f32, f32),
) -> Option<(f32, f32)> {
    let back_px = back_tiles.max(1.0) * tile_size as f32;
    let (dir_x, dir_y) = choke_line_normal_toward_inside(choke, line_point, own_start_world)?;
    Some((
        line_point.0 + dir_x * back_px,
        line_point.1 + dir_y * back_px,
    ))
}

fn anti_tank_face_toward(
    choke: AiBaseChoke,
    line_point: (f32, f32),
    back_tiles: f32,
    map: AiMapSummary,
    own_start_world: (f32, f32),
) -> Option<(f32, f32)> {
    let (back_x, back_y) = choke_line_normal_toward_inside(choke, line_point, own_start_world)?;
    let emplacement = backed_choke_point(
        choke,
        line_point,
        back_tiles,
        map.tile_size,
        own_start_world,
    )?;
    let face_px = TURTLE_AT_FACE_TARGET_TILES.max(back_tiles + 1.0) * map.tile_size as f32;
    Some((
        emplacement.0 - back_x * face_px,
        emplacement.1 - back_y * face_px,
    ))
}

fn choke_line_normal_toward_inside(
    choke: AiBaseChoke,
    line_point: (f32, f32),
    own_start_world: (f32, f32),
) -> Option<(f32, f32)> {
    let dx = choke.endpoint_b_world.0 - choke.endpoint_a_world.0;
    let dy = choke.endpoint_b_world.1 - choke.endpoint_a_world.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f32::EPSILON || !len.is_finite() {
        return normalized_direction(choke.enemy_approach_world, own_start_world)
            .or_else(|| normalized_direction(choke.center_world, own_start_world))
            .or_else(|| {
                normalized_direction(choke.enemy_approach_world, choke.own_approach_world)
            });
    }

    let mut normal = (-dy / len, dx / len);
    let own_vec = (
        own_start_world.0 - line_point.0,
        own_start_world.1 - line_point.1,
    );
    let own_dot = normal.0 * own_vec.0 + normal.1 * own_vec.1;
    if own_dot.abs() <= f32::EPSILON {
        let approach_vec = (
            choke.own_approach_world.0 - line_point.0,
            choke.own_approach_world.1 - line_point.1,
        );
        if normal.0 * approach_vec.0 + normal.1 * approach_vec.1 < 0.0 {
            normal = (-normal.0, -normal.1);
        }
    } else if own_dot < 0.0 {
        normal = (-normal.0, -normal.1);
    }
    Some(normal)
}

fn unit_zone_distance2(
    unit: &AiEntitySummary,
    choke: AiBaseChoke,
    zone: TurtleSlotZone,
    map: AiMapSummary,
) -> f32 {
    let unit_point = (unit.x, unit.y);
    match zone {
        TurtleSlotZone::ChokeLine => {
            point_segment_distance2(unit_point, choke.endpoint_a_world, choke.endpoint_b_world)
        }
        TurtleSlotZone::AntiTankEmplacement {
            back_tiles,
            own_start_world,
        } => {
            let Some(start) = backed_choke_point(
                choke,
                choke.endpoint_a_world,
                back_tiles,
                map.tile_size,
                own_start_world,
            ) else {
                return f32::INFINITY;
            };
            let Some(end) = backed_choke_point(
                choke,
                choke.endpoint_b_world,
                back_tiles,
                map.tile_size,
                own_start_world,
            ) else {
                return f32::INFINITY;
            };
            point_segment_distance2(unit_point, start, end)
        }
    }
}

fn point_segment_distance2(point: (f32, f32), start: (f32, f32), end: (f32, f32)) -> f32 {
    let vx = end.0 - start.0;
    let vy = end.1 - start.1;
    let len2 = vx * vx + vy * vy;
    if len2 <= f32::EPSILON {
        return dist2(point.0, point.1, start.0, start.1);
    }
    let wx = point.0 - start.0;
    let wy = point.1 - start.1;
    let t = ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0);
    let closest = (start.0 + vx * t, start.1 + vy * t);
    dist2(point.0, point.1, closest.0, closest.1)
}

fn closest_point_on_segment_to_segment(
    segment_start: (f32, f32),
    segment_end: (f32, f32),
    route_start: (f32, f32),
    route_end: (f32, f32),
) -> (f32, f32) {
    if let Some(intersection) =
        segment_intersection(segment_start, segment_end, route_start, route_end)
    {
        return intersection;
    }

    let candidates = [
        closest_point_on_segment(route_start, segment_start, segment_end),
        closest_point_on_segment(route_end, segment_start, segment_end),
        segment_start,
        segment_end,
    ];
    candidates
        .into_iter()
        .min_by(|left, right| {
            point_segment_distance2(*left, route_start, route_end)
                .total_cmp(&point_segment_distance2(*right, route_start, route_end))
        })
        .unwrap_or(segment_start)
}

fn segment_intersection(
    a: (f32, f32),
    b: (f32, f32),
    c: (f32, f32),
    d: (f32, f32),
) -> Option<(f32, f32)> {
    let r = (b.0 - a.0, b.1 - a.1);
    let s = (d.0 - c.0, d.1 - c.1);
    let denominator = cross(r, s);
    if denominator.abs() <= f32::EPSILON {
        return None;
    }
    let c_to_a = (c.0 - a.0, c.1 - a.1);
    let t = cross(c_to_a, s) / denominator;
    let u = cross(c_to_a, r) / denominator;
    if (-f32::EPSILON..=1.0 + f32::EPSILON).contains(&t)
        && (-f32::EPSILON..=1.0 + f32::EPSILON).contains(&u)
    {
        Some((a.0 + t * r.0, a.1 + t * r.1))
    } else {
        None
    }
}

fn closest_point_on_segment(point: (f32, f32), start: (f32, f32), end: (f32, f32)) -> (f32, f32) {
    let vx = end.0 - start.0;
    let vy = end.1 - start.1;
    let len2 = vx * vx + vy * vy;
    if len2 <= f32::EPSILON {
        return start;
    }
    let wx = point.0 - start.0;
    let wy = point.1 - start.1;
    let t = ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0);
    (start.0 + vx * t, start.1 + vy * t)
}

fn cross(left: (f32, f32), right: (f32, f32)) -> f32 {
    left.0 * right.1 - left.1 * right.0
}

fn choke_segment_len(choke: AiBaseChoke) -> f32 {
    dist2(
        choke.endpoint_a_world.0,
        choke.endpoint_a_world.1,
        choke.endpoint_b_world.0,
        choke.endpoint_b_world.1,
    )
    .sqrt()
}

fn choke_route_slot_bias(observation: &AiObservation, choke: AiBaseChoke) -> Option<(f32, f32)> {
    let own_start = tile_center(observation.own_start_tile, observation.map.tile_size);
    let enemy_start = nearest_enemy_start_world(observation, own_start)?;
    Some(closest_point_on_segment_to_segment(
        choke.endpoint_a_world,
        choke.endpoint_b_world,
        enemy_start,
        own_start,
    ))
}

fn prioritize_chokes_by_enemy_distance(observation: &AiObservation, chokes: &mut [AiBaseChoke]) {
    if chokes.len() <= 1 {
        return;
    }
    let own_start = tile_center(observation.own_start_tile, observation.map.tile_size);
    let Some(enemy_start) = nearest_enemy_start_world(observation, own_start) else {
        return;
    };
    chokes.sort_by(|left, right| {
        choke_line_distance2_to_point(*left, enemy_start)
            .total_cmp(&choke_line_distance2_to_point(*right, enemy_start))
            .then_with(|| right.width_tiles.cmp(&left.width_tiles))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn choke_line_distance2_to_point(choke: AiBaseChoke, point: (f32, f32)) -> f32 {
    point_segment_distance2(point, choke.endpoint_a_world, choke.endpoint_b_world)
}

fn prioritized_base_chokes(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    policy: TurtleDefensePolicy,
) -> Option<Vec<AiBaseChoke>> {
    let analysis = analysis?;
    let mut chokes = analysis.base_chokes_for_player(observation.player_id, policy.max_chokes);
    if chokes.is_empty() {
        return None;
    }
    // Keep main-choke choice deliberately simple: take the own-base exits nearest
    // our start region, then defend the one whose line is nearest the enemy start.
    prioritize_chokes_by_enemy_distance(observation, &mut chokes);
    Some(chokes)
}

fn machine_gunner_target_chokes(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    policy: TurtleDefensePolicy,
) -> Option<Vec<AiBaseChoke>> {
    let mut chokes = prioritized_base_chokes(observation, analysis, policy)?;
    let target_count = policy.machine_gunner_target_chokes.min(chokes.len());
    chokes.truncate(target_count);
    Some(chokes)
}

fn early_choke_count(observation: &AiObservation, choke_count: usize) -> usize {
    if choke_count <= 1 || cross_spawn_against_nearest_enemy(observation) {
        1
    } else {
        choke_count.min(2)
    }
}

fn cross_spawn_against_nearest_enemy(observation: &AiObservation) -> bool {
    let own_start = tile_center(observation.own_start_tile, observation.map.tile_size);
    let Some(enemy_start) = nearest_enemy_start_tile(observation, own_start) else {
        return true;
    };
    let dx = observation.own_start_tile.0.abs_diff(enemy_start.0);
    let dy = observation.own_start_tile.1.abs_diff(enemy_start.1);
    let diagonal_dx = (observation.map.width / 3).max(1);
    let diagonal_dy = (observation.map.height / 3).max(1);
    dx >= diagonal_dx && dy >= diagonal_dy
}

fn nearest_enemy_start_world(
    observation: &AiObservation,
    own_start: (f32, f32),
) -> Option<(f32, f32)> {
    nearest_enemy_start_tile(observation, own_start)
        .map(|start_tile| tile_center(start_tile, observation.map.tile_size))
}

fn nearest_enemy_start_tile(
    observation: &AiObservation,
    own_start: (f32, f32),
) -> Option<(u32, u32)> {
    observation
        .players
        .iter()
        .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
        .min_by(|left, right| {
            let left_center = tile_center(left.start_tile, observation.map.tile_size);
            let right_center = tile_center(right.start_tile, observation.map.tile_size);
            dist2(left_center.0, left_center.1, own_start.0, own_start.1)
                .total_cmp(&dist2(
                    right_center.0,
                    right_center.1,
                    own_start.0,
                    own_start.1,
                ))
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|player| player.start_tile)
}

fn units_by_id(observation: &AiObservation) -> BTreeMap<u32, &AiEntitySummary> {
    observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect()
}

fn unit_ids_with_kinds(
    observation: &AiObservation,
    units: &[u32],
    kinds: &[EntityKind],
) -> Vec<u32> {
    let units_by_id = units_by_id(observation);
    units
        .iter()
        .copied()
        .filter(|unit_id| {
            units_by_id
                .get(unit_id)
                .is_some_and(|unit| unit_counts_for_turtle(unit, kinds))
        })
        .collect()
}

fn staff_radius2(map: AiMapSummary) -> f32 {
    squared(TURTLE_STAFF_RADIUS_TILES * map.tile_size as f32)
}

fn close_enough2(observation: &AiObservation) -> f32 {
    squared(TURTLE_REISSUE_EPS_TILES * observation.map.tile_size as f32)
}
