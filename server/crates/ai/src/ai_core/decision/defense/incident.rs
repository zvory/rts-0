use super::envelope::LocalDefenseContact;
use super::*;

pub(super) const SEARCH_TICKS: u32 = config::TICK_HZ * 2;
const REACQUIRE_TILES: f32 = 1.5;
/// A Tank that has not moved for three seconds reaches this range (the simulation ramps it from
/// the 5-tile base). Any movement or hull turn resets it to the base range.
const STATIONARY_TANK_RANGE_TILES: f32 = 14.0;
/// A Tank beyond stationary range closes to here and parks, so it settles inside full range
/// before the attackers reach it.
const STATIONARY_TANK_CLOSE_TO_TILES: f32 = 12.0;
/// Spotters stop this far short of the threat: inside the 10-tile sight radius, outside the
/// Rifleman and Machine Gunner reach.
const SPOTTER_STANDOFF_TILES: f32 = 8.0;
const MAX_SPOTTERS: usize = 2;
/// With nobody spotting, a Tank parks inside its own 10-tile sight so it can see what it shoots.
const UNSPOTTED_TANK_PARK_TILES: f32 = 9.0;
/// A completed building that lost HP this recently means the base is being raided.
const BUILDINGS_UNDER_FIRE_TICKS: u32 = config::TICK_HZ * 4;

/// `stationary_tanks` keeps defending Tanks parked for their stationary range bonus and sends
/// infantry forward to provide the vision for those long shots, instead of ordering the Tanks to
/// attack (which drives them to the 5-tile base range). `raid_defense` (the live Jeff) answers
/// raids without Tanks away from the dug-in line with `super::raid` instead.
pub(in crate::ai_core::decision) fn respond_to_local_incident(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    local_defenders: &[u32],
    map_analysis: Option<&AiMapAnalysis>,
    stationary_tanks: bool,
    profile_id: &str,
) -> Option<Vec<u32>> {
    let damaged_building = stationary_tanks
        .then(|| note_building_damage(observation, memory))
        .flatten();
    if damaged_building.is_some() {
        memory.local_defense_building_hit_tick = Some(observation.tick);
    }
    if runs_live_jeff(profile_id) {
        let resend = profile_id == JEFFS_AI_ID;
        if let super::raid::RaidAnswer::Handled(units) = super::raid::respond(
            actions,
            observation,
            memory,
            local_defenders,
            map_analysis,
            resend,
        ) {
            memory.clear_defensive_incident();
            return (!units.is_empty()).then_some(units);
        }
    }
    let buildings_under_fire = stationary_tanks
        && memory
            .local_defense_building_hit_tick
            .is_some_and(|hit| observation.tick.saturating_sub(hit) <= BUILDINGS_UNDER_FIRE_TICKS);
    if let Some(contact) = local_defense_contact(observation) {
        memory.note_defensive_contact(
            observation.tick,
            contact.intercept,
            contact.threat_value,
            contact.armored_threat,
        );
        let eligible = eligible_local_defenders(observation, local_defenders);
        if buildings_under_fire && !contact.armored_threat {
            return respond_to_raid(
                actions,
                observation,
                memory,
                eligible,
                local_defenders,
                map_analysis,
                &contact,
            );
        }
        let mut interceptors = select_defensive_interceptors(
            observation,
            memory,
            eligible,
            contact.intercept,
            contact.threat_value,
            contact.armored_threat,
        );
        let smoke = crate::ai_core::decision::frontal::smoke::maybe_issue_local_defense_smoke(
            actions,
            observation,
            &interceptors,
            local_defenders,
            &contact.target_ids,
            memory,
        );
        let mut attack_targets = contact.target_ids.clone();
        if let Some(smoke) = smoke {
            let scout = match smoke {
                crate::ai_core::decision::frontal::smoke::LocalDefenseSmokeDirective::Obscure {
                    target,
                    scout,
                } => {
                    attack_targets.retain(|candidate| *candidate != target);
                    scout
                }
                crate::ai_core::decision::frontal::smoke::LocalDefenseSmokeDirective::Reposition {
                    scout,
                } => scout,
            };
            interceptors.retain(|unit| *unit != scout);
        }
        if stationary_tanks {
            if let Some(target) = primary_defense_target(observation, &attack_targets) {
                return stationary_tank_defense(
                    actions,
                    observation,
                    memory,
                    interceptors,
                    local_defenders,
                    map_analysis,
                    &attack_targets,
                    target,
                    true,
                );
            }
        }
        return if let Some(target) = primary_defense_target(observation, &attack_targets) {
            actions::attack_units(actions, interceptors, target)
        } else {
            actions::hold_position_units(actions, interceptors)
        };
    }

    // Enemy Tanks shelling a base from beyond the zone: the home Tanks answer them together from
    // firing range, or, too few to match them, stay put rather than go in one at a time. Either
    // way nobody searches the damaged building: the shooter is in sight, and the search sent
    // Riflemen out against Tanks while the Tanks stood out of range.
    if stationary_tanks {
        if let Some(siege) = tank_siege(observation, local_defenders) {
            if !siege.matched() {
                return None;
            }
            return stationary_tank_defense(
                actions,
                observation,
                memory,
                siege.responders,
                local_defenders,
                map_analysis,
                &siege.contact.target_ids,
                siege.target,
                true,
            );
        }
    }

    // A Resource Depot sees one tile, so attackers shooting a building from range are often in
    // fog. Treat the building losing HP as the contact so defenders go and find them.
    if let Some(position) = damaged_building {
        memory.note_defensive_contact(observation.tick, position, 1, false);
    }
    let incident = memory.defensive_incident(observation.tick, SEARCH_TICKS)?;
    let candidates = local_defense_units_with_plans(observation, local_defenders);
    let reacquire2 = squared(REACQUIRE_TILES * observation.map.tile_size as f32);
    let reached_last_contact = candidates.iter().any(|id| {
        observation.owned.iter().any(|unit| {
            unit.id == *id
                && dist2(unit.x, unit.y, incident.position.0, incident.position.1) <= reacquire2
        })
    });
    if reached_last_contact {
        memory.clear_defensive_incident();
        memory.local_defense_held_tanks.clear();
        return None;
    }
    let interceptors = if buildings_under_fire && !incident.armored_threat {
        // Raiders shooting from fog: whoever is not dug in goes to the damaged building, with no
        // two-to-one minimum. Trenches stay, since nothing can tell whether they reach the raid.
        candidates
            .into_iter()
            .filter(|id| {
                memory.estimated_entrenchment_ticks(observation, *id)
                    < rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS
            })
            .collect()
    } else {
        select_defensive_interceptors(
            observation,
            memory,
            candidates,
            incident.position,
            incident.threat_value,
            incident.armored_threat,
        )
    };
    // Contact is lost: a parked Tank can no longer see anything to shoot, so it joins the search
    // and is parked again once the threat is back in sight.
    memory.local_defense_held_tanks.clear();
    actions::attack_move_units(
        actions,
        interceptors,
        incident.position.0,
        incident.position.1,
    )
}

/// Home Tanks this close to the Tank being answered answer it.
const SIEGE_RESPONSE_TILES: f32 = 32.0;
/// Enemy Tanks this close to the Tank being answered count against the home Tanks.
const SIEGE_ODDS_TILES: f32 = 16.0;

/// Enemy Tanks within reach of one of the base's buildings, the one to answer first, and the home
/// Tanks near enough to answer.
pub(in crate::ai_core::decision) struct TankSiege {
    pub(in crate::ai_core::decision) contact: LocalDefenseContact,
    pub(in crate::ai_core::decision) target: u32,
    pub(in crate::ai_core::decision) responders: Vec<u32>,
    pub(in crate::ai_core::decision) enemy_tanks: usize,
}

impl TankSiege {
    /// Whether the home Tanks at least match the enemy Tanks around the target.
    pub(in crate::ai_core::decision) fn matched(&self) -> bool {
        self.responders.len() >= self.enemy_tanks
    }
}

/// Enemy Tanks in reach of the base and the home Tanks (from `local_defenders`) that could answer
/// them. Tanks inside the local defense zone count too; the zone's own response runs first.
pub(in crate::ai_core::decision) fn tank_siege(
    observation: &AiObservation,
    local_defenders: &[u32],
) -> Option<TankSiege> {
    let contact = super::envelope::tank_siege_contact(observation)?;
    let target = primary_defense_target(observation, &contact.target_ids)?;
    let target_position = observation
        .visible_enemies
        .iter()
        .find(|enemy| enemy.id == target)
        .map(|enemy| (enemy.x, enemy.y))?;
    let ts = observation.map.tile_size as f32;
    let responders = observation
        .owned
        .iter()
        .filter(|unit| {
            unit.kind == EntityKind::Tank
                && unit.is_complete
                && unit.hp > 0
                && local_defenders.contains(&unit.id)
                && dist2(unit.x, unit.y, target_position.0, target_position.1)
                    <= squared(SIEGE_RESPONSE_TILES * ts)
        })
        .map(|unit| unit.id)
        .collect();
    let enemy_tanks = observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            enemy.kind == EntityKind::Tank
                && enemy.hp > 0
                && dist2(enemy.x, enemy.y, target_position.0, target_position.1)
                    <= squared(SIEGE_ODDS_TILES * ts)
        })
        .count();
    Some(TankSiege {
        contact,
        target,
        responders,
        enemy_tanks,
    })
}

/// Infantry already within this much of its reach of a raider counts as covering the raid.
const RAID_REACH_MARGIN_TILES: f32 = 0.5;
/// Dug-in infantry only leaves its trenches for a raid at least this big that nobody reaches.
const RAID_COLLAPSE_MIN_RAIDERS: usize = 3;
/// Out-of-reach infantry sent to a raid: this many per raider, and never fewer than the minimum,
/// since a raid is often seen one unit at a time as it arrives.
const RAID_SHIFT_PER_RAIDER: usize = 2;
const RAID_MIN_SHIFT: usize = 4;

/// Infantry is killing buildings. The usual two-to-one response sends nobody when it cannot reach
/// that value, and ordering units to attack a raider makes them chase it out of the base. Instead:
/// - infantry that already reaches a raider stays where it is and fights;
/// - dug-in infantry keeps its trench as long as anyone else reaches the raid, because a trench
///   is worth more than one more rifle in the open;
/// - other infantry attack-moves to the defensive point beside the building under attack, in
///   proportion to the raid (two per raider, at least four, nearest first). It fights on the way and stops
///   there, so it never follows a retreating raider out of the base;
/// - the trenches join only when a raid of three or more reaches nobody at all, so a single
///   raider shooting from range cannot empty them.
///
/// Tanks keep the parked stationary-range defense, without sending spotters forward.
fn respond_to_raid(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    eligible: Vec<u32>,
    local_defenders: &[u32],
    map_analysis: Option<&AiMapAnalysis>,
    contact: &LocalDefenseContact,
) -> Option<Vec<u32>> {
    let ts = observation.map.tile_size as f32;
    let owned = |id: u32| observation.owned.iter().find(|unit| unit.id == id);
    let raiders: Vec<&AiEntitySummary> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| contact.target_ids.contains(&enemy.id) && enemy.hp > 0)
        .collect();
    let (tanks, infantry): (Vec<u32>, Vec<u32>) = eligible
        .into_iter()
        .partition(|id| owned(*id).is_some_and(|unit| unit.kind == EntityKind::Tank));

    let mut assigned = Vec::new();
    if let Some(target) = primary_defense_target(observation, &contact.target_ids) {
        if !tanks.is_empty() {
            if let Some(units) = stationary_tank_defense(
                actions,
                observation,
                memory,
                tanks,
                local_defenders,
                map_analysis,
                &contact.target_ids,
                target,
                false,
            ) {
                assigned.extend(units);
            }
        }
    }

    let reaches = |unit: &AiEntitySummary, entrenched: bool| {
        let range = config::unit_stats(unit.kind).map_or(0.0, |stats| stats.range_tiles);
        let bonus = if entrenched {
            rts_rules::balance::ENTRENCHMENT_RANGE_BONUS_TILES as f32
        } else {
            0.0
        };
        let reach = (range + bonus + RAID_REACH_MARGIN_TILES) * ts;
        raiders
            .iter()
            .any(|raider| dist2(unit.x, unit.y, raider.x, raider.y) <= reach * reach)
    };
    let mut staying = Vec::new();
    let mut hold = Vec::new();
    let mut entrenched_out_of_reach = Vec::new();
    let mut shifting = Vec::new();
    for id in infantry {
        let Some(unit) = owned(id) else {
            continue;
        };
        let entrenched = memory.estimated_entrenchment_ticks(observation, id)
            >= rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS;
        if reaches(unit, entrenched) {
            // In reach: fight from here. Only a unit still walking somewhere is stopped.
            if unit.state == AiEntityState::Move {
                hold.push(id);
            }
            staying.push(id);
        } else if entrenched {
            entrenched_out_of_reach.push(id);
        } else {
            shifting.push(id);
        }
    }
    if staying.is_empty() && raiders.len() >= RAID_COLLAPSE_MIN_RAIDERS {
        // Nobody reaches a real raid: the trenches cannot help where they are.
        shifting.append(&mut entrenched_out_of_reach);
    } else {
        staying.append(&mut entrenched_out_of_reach);
    }
    // Answer in proportion to the raid, nearest the building first: a lone raider does not pull
    // the whole base out of position.
    let intercept = contact.intercept;
    shifting.sort_by(|left, right| {
        let distance = |id: &u32| {
            owned(*id).map_or(f32::INFINITY, |unit| {
                dist2(unit.x, unit.y, intercept.0, intercept.1)
            })
        };
        distance(left)
            .total_cmp(&distance(right))
            .then_with(|| left.cmp(right))
    });
    shifting.truncate(RAID_MIN_SHIFT.max(RAID_SHIFT_PER_RAIDER * raiders.len()));
    if let Some(units) = actions::hold_position_units(actions, hold) {
        assigned.extend(units);
    }
    if let Some(units) =
        actions::attack_move_units(actions, shifting, contact.intercept.0, contact.intercept.1)
    {
        assigned.extend(units);
    }
    assigned.extend(staying);
    assigned.sort_unstable();
    assigned.dedup();
    (!assigned.is_empty()).then_some(assigned)
}

/// Records completed building HP and returns the position of the building that lost the most
/// since the previous decision. Incomplete buildings are skipped because they gain HP while built.
fn note_building_damage(
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
) -> Option<(f32, f32)> {
    let previous = std::mem::take(&mut memory.local_defense_building_hp);
    let mut damaged = None;
    let mut largest_loss = 0;
    for building in observation
        .owned
        .iter()
        .filter(|entity| entity.kind.is_building() && entity.is_complete && entity.hp > 0)
    {
        memory
            .local_defense_building_hp
            .insert(building.id, building.hp);
        let loss = previous
            .get(&building.id)
            .map_or(0, |hp| hp.saturating_sub(building.hp));
        if loss > largest_loss {
            largest_loss = loss;
            damaged = Some((building.x, building.y));
        }
    }
    damaged
}

fn clear_line_of_fire(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    from: (f32, f32),
    to: (f32, f32),
) -> bool {
    let Some(direction) = normalized_direction(from, to) else {
        return true;
    };
    let tiles = dist2(from.0, from.1, to.0, to.1).sqrt() / observation.map.tile_size as f32;
    defensive_firing_lane_is_clear(observation, map_analysis, from, direction, tiles)
}

/// Where a Tank should park `park_tiles` from its target: the direct approach when that line of
/// fire is clear, otherwise the nearest open point around the target that has one.
fn tank_park_point(
    observation: &AiObservation,
    map_analysis: Option<&AiMapAnalysis>,
    tank: (f32, f32),
    target: (f32, f32),
    park_tiles: f32,
) -> (f32, f32) {
    let ts = observation.map.tile_size as f32;
    let point_at = |angle: f32| {
        clamp_to_map(
            (
                target.0 + angle.cos() * park_tiles * ts,
                target.1 + angle.sin() * park_tiles * ts,
            ),
            observation.map,
        )
    };
    let direct = normalized_direction(target, tank).unwrap_or((0.0, 1.0));
    let base = direct.1.atan2(direct.0);
    // Fan out from the direct approach, alternating sides, in 22.5 degree steps.
    let mut best = None;
    for step in 0..=8 {
        for side in [1.0, -1.0] {
            if step == 0 && side < 0.0 {
                continue;
            }
            let point = point_at(base + side * step as f32 * std::f32::consts::PI / 8.0);
            if defensive_position_is_open(observation, map_analysis, point.0, point.1)
                && clear_line_of_fire(observation, map_analysis, point, target)
            {
                let distance = dist2(point.0, point.1, tank.0, tank.1);
                if best.is_none_or(|(_, best_distance)| distance < best_distance) {
                    best = Some((point, distance));
                }
            }
        }
        if best.is_some() {
            break;
        }
    }
    best.map_or_else(|| point_at(base), |(point, _)| point)
}

#[allow(clippy::too_many_arguments)]
fn stationary_tank_defense(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
    interceptors: Vec<u32>,
    local_defenders: &[u32],
    map_analysis: Option<&AiMapAnalysis>,
    attack_targets: &[u32],
    target: u32,
    allow_spotters: bool,
) -> Option<Vec<u32>> {
    let ts = observation.map.tile_size as f32;
    let target_position = observation
        .visible_enemies
        .iter()
        .find(|enemy| enemy.id == target)
        .map(|enemy| (enemy.x, enemy.y))?;
    let threats: Vec<(f32, f32)> = observation
        .visible_enemies
        .iter()
        .filter(|enemy| attack_targets.contains(&enemy.id))
        .map(|enemy| (enemy.x, enemy.y))
        .collect();
    let owned = |id: u32| observation.owned.iter().find(|unit| unit.id == id);
    let (tanks, mut infantry): (Vec<u32>, Vec<u32>) = interceptors
        .into_iter()
        .partition(|id| owned(*id).is_some_and(|unit| unit.kind == EntityKind::Tank));
    memory
        .local_defense_held_tanks
        .retain(|id| tanks.contains(id));

    // Parked Tanks outrange their own 10-tile sight: without infantry ahead of them, the long
    // shots have no vision. Add spotters when the response selected only armor. Machine Gunners
    // never spot: they must stop and set up to fire, and one moved out as a spotter on one
    // decision and ordered to attack on the next never gets to shoot.
    let mut spotters = Vec::new();
    if allow_spotters && infantry.is_empty() && !tanks.is_empty() {
        let mut candidates: Vec<&AiEntitySummary> =
            eligible_local_defenders(observation, local_defenders)
                .into_iter()
                .filter_map(owned)
                .filter(|unit| unit.kind == EntityKind::Rifleman)
                .filter(|unit| {
                    memory.estimated_entrenchment_ticks(observation, unit.id)
                        < rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS
                })
                .collect();
        candidates.sort_by(|left, right| {
            dist2(left.x, left.y, target_position.0, target_position.1)
                .total_cmp(&dist2(
                    right.x,
                    right.y,
                    target_position.0,
                    target_position.1,
                ))
                .then_with(|| left.id.cmp(&right.id))
        });
        spotters.extend(candidates.iter().take(MAX_SPOTTERS).map(|unit| unit.id));
    }
    // With infantry lighting targets a Tank can sit at full stationary range. Alone it only sees
    // 10 tiles (a Resource Depot sees 1), so it must park where it can see its own targets.
    let vision_support = !infantry.is_empty() || !spotters.is_empty();
    let (hold_tiles, park_tiles) = if vision_support {
        (STATIONARY_TANK_RANGE_TILES, STATIONARY_TANK_CLOSE_TO_TILES)
    } else {
        (UNSPOTTED_TANK_PARK_TILES, UNSPOTTED_TANK_PARK_TILES)
    };

    let mut assigned = Vec::new();
    let mut hold = Vec::new();
    for tank_id in &tanks {
        let Some(tank) = owned(*tank_id) else {
            continue;
        };
        // Tank shells stop at buildings and sight-blocking terrain: parking only helps with a
        // threat both in range and in a clear line of fire.
        let can_fire = threats.iter().any(|threat| {
            dist2(tank.x, tank.y, threat.0, threat.1) <= squared(hold_tiles * ts)
                && clear_line_of_fire(observation, map_analysis, (tank.x, tank.y), *threat)
        });
        if can_fire {
            // Holding clears the Tank's target, so re-issue it only when something moved it.
            if !memory.local_defense_held_tanks.contains(tank_id)
                || tank.state == AiEntityState::Move
            {
                hold.push(*tank_id);
                memory.local_defense_held_tanks.insert(*tank_id);
            }
        } else {
            memory.local_defense_held_tanks.remove(tank_id);
            let park = tank_park_point(
                observation,
                map_analysis,
                (tank.x, tank.y),
                target_position,
                park_tiles,
            );
            if let Some(units) = actions::move_units(actions, [*tank_id], park.0, park.1) {
                assigned.extend(units);
            }
        }
    }
    if let Some(units) = actions::hold_position_units(actions, hold) {
        assigned.extend(units);
    }
    // Held Tanks keep their slot in the response even on ticks where no new order is needed.
    assigned.extend(memory.local_defense_held_tanks.iter().copied());

    for spotter_id in &spotters {
        let Some(spotter) = owned(*spotter_id) else {
            continue;
        };
        let direction =
            normalized_direction(target_position, (spotter.x, spotter.y)).unwrap_or((0.0, 1.0));
        let point = clamp_to_map(
            (
                target_position.0 + direction.0 * SPOTTER_STANDOFF_TILES * ts,
                target_position.1 + direction.1 * SPOTTER_STANDOFF_TILES * ts,
            ),
            observation.map,
        );
        if let Some(units) = actions::move_units(actions, [*spotter_id], point.0, point.1) {
            assigned.extend(units);
        }
    }

    // Selected infantry advance into contact as before, which also lights the targets.
    infantry.retain(|id| !spotters.contains(id));
    if let Some(units) = actions::attack_units(actions, infantry, target) {
        assigned.extend(units);
    }
    assigned.sort_unstable();
    assigned.dedup();
    (!assigned.is_empty()).then_some(assigned)
}

fn eligible_local_defenders(observation: &AiObservation, local_defenders: &[u32]) -> Vec<u32> {
    local_defense_units_with_plans(observation, local_defenders)
        .into_iter()
        .filter(|id| {
            observation.owned.iter().any(|unit| {
                unit.id == *id
                    && matches!(
                        unit.kind,
                        EntityKind::Rifleman
                            | EntityKind::MachineGunner
                            | EntityKind::ScoutCar
                            | EntityKind::Panzerfaust
                            | EntityKind::Tank
                    )
            })
        })
        .collect()
}

pub(in crate::ai_core::decision) fn select_defensive_interceptors(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    mut candidates: Vec<u32>,
    contact: (f32, f32),
    threat_value: u32,
    armored_threat: bool,
) -> Vec<u32> {
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|entity| (entity.id, entity))
        .collect();
    candidates.sort_by(|left, right| {
        let left_counter_rank = by_id.get(left).map_or(u8::MAX, |unit| {
            defensive_counter_rank(unit.kind, armored_threat)
        });
        let right_counter_rank = by_id.get(right).map_or(u8::MAX, |unit| {
            defensive_counter_rank(unit.kind, armored_threat)
        });
        let left_entrenchment = memory.estimated_entrenchment_ticks(observation, *left);
        let right_entrenchment = memory.estimated_entrenchment_ticks(observation, *right);
        let left_dist = by_id
            .get(left)
            .map(|unit| dist2(unit.x, unit.y, contact.0, contact.1))
            .unwrap_or(f32::INFINITY);
        let right_dist = by_id
            .get(right)
            .map(|unit| dist2(unit.x, unit.y, contact.0, contact.1))
            .unwrap_or(f32::INFINITY);
        left_counter_rank
            .cmp(&right_counter_rank)
            .then_with(|| left_entrenchment.cmp(&right_entrenchment))
            .then_with(|| left_dist.total_cmp(&right_dist))
            .then_with(|| left.cmp(right))
    });
    candidates.dedup();
    candidates.retain(|unit_id| {
        by_id.get(unit_id).is_some_and(|unit| {
            unit.kind != EntityKind::Rifleman
                || memory.estimated_entrenchment_ticks(observation, *unit_id)
                    < rts_rules::balance::ENTRENCHMENT_DIG_IN_TICKS
        })
    });

    // A two-to-one observed-value response clears a small penetration without uprooting every
    // entrenched edge guard. Larger forces naturally exhaust the available home reserve.
    let required_value = threat_value.saturating_mul(2).max(1);
    let mut selected = Vec::new();
    let mut selected_value: u32 = 0;
    let mut has_anti_armor = false;
    let eligible_candidates = candidates.clone();
    for unit_id in candidates {
        let Some(unit) = by_id.get(&unit_id) else {
            continue;
        };
        selected.push(unit_id);
        has_anti_armor |= matches!(unit.kind, EntityKind::Tank | EntityKind::Panzerfaust);
        let (steel, oil) = rts_rules::economy::cost(unit.kind);
        selected_value = selected_value.saturating_add(steel.saturating_add(oil).max(1));
        if selected_value >= required_value {
            break;
        }
    }
    let selected_has_tank = selected.iter().any(|unit_id| {
        by_id
            .get(unit_id)
            .is_some_and(|unit| unit.kind == EntityKind::Tank)
    });
    let river_natural =
        crate::ai_core::decision::expansion::has_jeff_river_expansion_site(observation);
    if selected_has_tank && river_natural {
        for kind in [EntityKind::Tank, EntityKind::Rifleman] {
            for unit_id in eligible_candidates
                .iter()
                .copied()
                .filter(|unit_id| by_id.get(unit_id).is_some_and(|unit| unit.kind == kind))
            {
                if !selected.contains(&unit_id) {
                    selected.push(unit_id);
                }
                let selected_of_kind = selected
                    .iter()
                    .filter(|selected_id| {
                        by_id.get(selected_id).is_some_and(|unit| unit.kind == kind)
                    })
                    .count();
                if selected_of_kind >= 2 {
                    break;
                }
            }
        }
    }
    if selected_value < required_value && !has_anti_armor {
        Vec::new()
    } else {
        selected
    }
}

fn defensive_counter_rank(kind: EntityKind, armored_threat: bool) -> u8 {
    if armored_threat {
        match kind {
            EntityKind::Tank | EntityKind::Panzerfaust => 0,
            EntityKind::Rifleman | EntityKind::MachineGunner | EntityKind::ScoutCar => 1,
            _ => 2,
        }
    } else {
        0
    }
}

fn primary_defense_target(observation: &AiObservation, target_ids: &[u32]) -> Option<u32> {
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| target_ids.contains(&enemy.id))
        .max_by(|left, right| {
            let left_armored = matches!(left.kind, EntityKind::Tank | EntityKind::ScoutCar);
            let right_armored = matches!(right.kind, EntityKind::Tank | EntityKind::ScoutCar);
            left_armored
                .cmp(&right_armored)
                .then_with(|| unit_value(left.kind).cmp(&unit_value(right.kind)))
                .then_with(|| right.id.cmp(&left.id))
        })
        .map(|enemy| enemy.id)
}
