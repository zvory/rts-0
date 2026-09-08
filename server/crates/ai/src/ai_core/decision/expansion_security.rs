use super::geometry::{building_center, dist2, normalized_direction, squared, tile_center};
use super::*;

const HOME_RIFLES: usize = 4;
pub(super) const ESCORT_PARTY_SIZE: usize = 4;
pub(super) const SECURITY_RIFLE_TARGET: usize = HOME_RIFLES + ESCORT_PARTY_SIZE;
const REQUIRED_GUARDS: usize = 2;
const SECURE_TICKS: u32 = config::TICK_HZ * 3;
const GUARD_COVERAGE_TILES: f32 = 8.0;
const MIN_PARTY_SEPARATION_TILES: f32 = 2.75;
const TANK_FRONT_OFFSET_TILES: f32 = 2.75;
const BUILD_START_TIMEOUT_TICKS: u32 = config::TICK_HZ * 3;
const ESCORT_STALL_TICKS: u32 = config::TICK_HZ * 10;
const ESCORT_RETRY_TICKS: u32 = config::TICK_HZ * 10;
const ESCORT_PROGRESS_EPS_TILES: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct EscortProgress {
    best_distance_milli_tiles: u32,
    last_progress_tick: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ExpansionSecurity {
    pub(super) site: Option<(u32, u32)>,
    pub(super) riflemen: Vec<u32>,
    slots: BTreeMap<u32, usize>,
    progress: BTreeMap<u32, EscortProgress>,
    retry_after: BTreeMap<u32, u32>,
    secure_since: Option<u32>,
    build_attempt_tick: Option<u32>,
    build_attempt_worker: Option<u32>,
    retry_builder: Option<u32>,
    rejected_sites: BTreeSet<(u32, u32)>,
}

impl ExpansionSecurity {
    pub(super) fn note_build_attempt(&mut self, tick: u32, worker: u32) {
        self.build_attempt_tick = Some(tick);
        self.build_attempt_worker = Some(worker);
        self.retry_builder = None;
    }

    pub(super) fn retry_builder(&self) -> Option<u32> {
        self.retry_builder
    }
}

/// The first Tank is paid before the natural; subsequent gas is reserved for the Depot.
/// This deliberately does not require Depot affordability, a build intent, or a finished Tank.
pub(super) fn expansion_is_next(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
) -> bool {
    profile.id == JEFFS_AI_ID
        && observation
            .owned
            .iter()
            .filter(|entity| entity.kind == EntityKind::ResourceDepot && entity.hp > 0)
            .count()
            < 2
        && facts.complete_building_count(EntityKind::Factory) > 0
        && (facts.unit_count(EntityKind::Tank) > 0
            || observation.owned.iter().any(|unit| {
                unit.kind == EntityKind::Factory
                    && unit.production_kind == Some(EntityKind::Tank)
                    && unit.production_queue_len.unwrap_or(0) > 0
            }))
}

pub(super) fn predicts_natural_from_opening(observation: &AiObservation) -> bool {
    observation.map.width == 166
        && observation.map.height == 166
        && matches!(observation.own_start_tile, (157, 47) | (8, 47))
}

pub(super) fn prepare<F: FnMut(EntityKind, u32, u32) -> bool>(
    observation: &AiObservation,
    facts: &AiFacts,
    profile: &AiProfile,
    memory: &mut AiDecisionMemory,
    placeable: &mut F,
) {
    if profile.id != JEFFS_AI_ID {
        return;
    }
    let active_depot_count = observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::ResourceDepot && entity.hp > 0)
        .count();
    // Once the attempted Depot appears in the authoritative observation, the build order
    // succeeded. Do not retain its timeout: if that Depot is destroyed later, stale attempt
    // state must not reject the proven site or retry the old builder immediately.
    if active_depot_count >= 2 {
        memory.expansion_security.build_attempt_tick = None;
        memory.expansion_security.build_attempt_worker = None;
        memory.expansion_security.retry_builder = None;
        if memory.expansion_security.riflemen.len() > REQUIRED_GUARDS {
            let Some(center) = memory.expansion_security.site.and_then(|site| {
                building_center(site, EntityKind::ResourceDepot, observation.map.tile_size)
            }) else {
                return;
            };
            memory.expansion_security.riflemen.sort_by(|left, right| {
                let distance = |id: &u32| {
                    observation
                        .owned
                        .iter()
                        .find(|unit| unit.id == *id)
                        .map_or(f32::MAX, |unit| dist2(unit.x, unit.y, center.0, center.1))
                };
                distance(left)
                    .total_cmp(&distance(right))
                    .then_with(|| left.cmp(right))
            });
            memory.expansion_security.riflemen.truncate(REQUIRED_GUARDS);
        }
    }
    let timed_out_site = memory
        .expansion_security
        .build_attempt_tick
        .is_some_and(|tick| {
            active_depot_count < 2
                && observation.tick.saturating_sub(tick) >= BUILD_START_TIMEOUT_TICKS
        });
    if timed_out_site {
        if let Some(site) = memory.expansion_security.site {
            memory.expansion_security.rejected_sites.insert(site);
        }
        memory.expansion_security.site = None;
        memory.expansion_security.riflemen.clear();
        memory.expansion_security.slots.clear();
        memory.expansion_security.secure_since = None;
        memory.expansion_security.build_attempt_tick = None;
        memory.expansion_security.retry_builder =
            memory.expansion_security.build_attempt_worker.take();
    }
    let security_wait_complete = memory
        .expansion_security
        .secure_since
        .is_some_and(|since| observation.tick.saturating_sub(since) >= SECURE_TICKS);
    let site_became_blocked = memory.expansion_security.site.is_some_and(|site| {
        facts.building_count(EntityKind::ResourceDepot) < 2
            && (site_blocked_by_owned_building(observation, site)
                || (security_wait_complete
                    && !placeable(EntityKind::ResourceDepot, site.0, site.1)))
    });
    if site_became_blocked {
        let rejected_sites = std::mem::take(&mut memory.expansion_security.rejected_sites);
        memory.expansion_security = ExpansionSecurity {
            rejected_sites,
            ..ExpansionSecurity::default()
        };
    }
    if memory.expansion_security.site.is_none()
        && ((profile.id == JEFFS_AI_ID && predicts_natural_from_opening(observation))
            || expansion_is_next(observation, facts, profile))
        && facts.building_count(EntityKind::ResourceDepot) < 2
    {
        if let Some(policy) = profile.expansion {
            let rejected_sites = &memory.expansion_security.rejected_sites;
            memory.expansion_security.site = expansion::expansion_resource_depot_site(
                observation,
                policy,
                EntityKind::ResourceDepot,
                profile.id,
                &mut |kind, x, y| !rejected_sites.contains(&(x, y)) && placeable(kind, x, y),
            );
        }
    }
    if memory.expansion_security.site.is_none() {
        return;
    }
    // Predict and reserve the footprint from the opening, but do not pull the security party
    // off the opening army until the expansion is actually the next strategic milestone.
    if !expansion_is_next(observation, facts, profile) {
        return;
    }
    memory
        .expansion_security
        .retry_after
        .retain(|_, retry_tick| observation.tick < *retry_tick);
    let mut rifles: Vec<_> = observation
        .owned
        .iter()
        .filter(|unit| unit.kind == EntityKind::Rifleman && unit.is_complete && unit.hp > 0)
        .map(|unit| unit.id)
        .collect();
    rifles.sort_unstable();
    memory
        .expansion_security
        .riflemen
        .retain(|id| rifles.contains(id));
    let home_reserve = rifles
        .iter()
        .copied()
        .filter(|id| !memory.expansion_security.riflemen.contains(id))
        .take(HOME_RIFLES)
        .collect::<BTreeSet<_>>();
    let mut candidates = rifles
        .iter()
        .copied()
        .filter(|id| !home_reserve.contains(id))
        .filter(|id| !memory.expansion_security.riflemen.contains(id))
        .filter(|id| !memory.expansion_security.retry_after.contains_key(id))
        .collect::<Vec<_>>();
    if let Some(center) = building_center(
        memory.expansion_security.site.unwrap(),
        EntityKind::ResourceDepot,
        observation.map.tile_size,
    ) {
        candidates.sort_by(|left, right| {
            let distance = |id| {
                observation
                    .owned
                    .iter()
                    .find(|unit| unit.id == id)
                    .map_or(f32::MAX, |unit| dist2(unit.x, unit.y, center.0, center.1))
            };
            distance(*left)
                .total_cmp(&distance(*right))
                .then_with(|| left.cmp(right))
        });
    }
    candidates.truncate(ESCORT_PARTY_SIZE.saturating_sub(memory.expansion_security.riflemen.len()));
    for id in &candidates {
        if memory.expansion_security.riflemen.len() >= ESCORT_PARTY_SIZE {
            break;
        }
        if !memory.expansion_security.riflemen.contains(id) {
            memory.expansion_security.riflemen.push(*id);
        }
    }
    let security = &mut memory.expansion_security;
    security
        .slots
        .retain(|id, _| security.riflemen.contains(id));
    security
        .progress
        .retain(|id, _| security.riflemen.contains(id));
    for id in &security.riflemen {
        if !security.slots.contains_key(id) {
            if let Some(slot) = (0..ESCORT_PARTY_SIZE)
                .find(|slot| !security.slots.values().any(|used| used == slot))
            {
                security.slots.insert(*id, slot);
            }
        }
    }
    memory
        .containment_active_riflemen
        .retain(|id| !memory.expansion_security.riflemen.contains(id));
}

/// Move friendly combat units out of the predicted Depot footprint before they can settle there.
/// Workers are excluded because the eventual builder must be allowed to enter the reserved area.
pub(super) fn clear_reserved_footprint(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    actions: &mut AiActionContext<'_>,
) -> Vec<u32> {
    let Some(site) = memory.expansion_security.site else {
        return Vec::new();
    };
    if observation
        .owned
        .iter()
        .filter(|entity| entity.kind == EntityKind::ResourceDepot && entity.hp > 0)
        .count()
        >= 2
    {
        return Vec::new();
    }
    let Some(stats) = config::building_stats(EntityKind::ResourceDepot) else {
        return Vec::new();
    };
    let ts = observation.map.tile_size as f32;
    let rect = (
        site.0 as f32 * ts,
        site.1 as f32 * ts,
        site.0.saturating_add(stats.foot_w) as f32 * ts,
        site.1.saturating_add(stats.foot_h) as f32 * ts,
    );
    let Some(center) = building_center(site, EntityKind::ResourceDepot, observation.map.tile_size)
    else {
        return Vec::new();
    };
    let blockers = observation
        .owned
        .iter()
        .filter(|unit| unit.kind.is_unit() && unit.kind != EntityKind::Worker && unit.hp > 0)
        .filter(|unit| {
            crate::sdk::unit_circle_touches_rect(
                (unit.x, unit.y),
                rts_rules::balance::unit_placement_radius(unit.kind) + 0.25 * ts,
                rect,
            )
        })
        .map(|unit| unit.id)
        .collect::<Vec<_>>();
    for id in &blockers {
        let Some(unit) = observation.owned.iter().find(|unit| unit.id == *id) else {
            continue;
        };
        // Use a short radial exit. A destination toward home can lie across Schone Tage's cliff
        // and leave the blocker parked inside the footprint indefinitely.
        let direction = normalized_direction(center, (unit.x, unit.y)).unwrap_or((0.0, 1.0));
        let half_diagonal =
            ((stats.foot_w * stats.foot_w + stats.foot_h * stats.foot_h) as f32).sqrt() * 0.5;
        let clearance =
            half_diagonal * ts + rts_rules::balance::unit_placement_radius(unit.kind) + 0.5 * ts;
        actions::move_units(
            actions,
            [*id],
            center.0 + direction.0 * clearance,
            center.1 + direction.1 * clearance,
        );
    }
    blockers
}

fn site_blocked_by_owned_building(observation: &AiObservation, site: (u32, u32)) -> bool {
    observation
        .owned
        .iter()
        .filter(|entity| entity.hp > 0 && entity.kind.is_building())
        .any(|entity| {
            let Some(stats) = config::building_stats(entity.kind) else {
                return false;
            };
            let center_tile = (
                (entity.x / observation.map.tile_size as f32)
                    .floor()
                    .max(0.0) as u32,
                (entity.y / observation.map.tile_size as f32)
                    .floor()
                    .max(0.0) as u32,
            );
            let existing = (
                center_tile.0.saturating_sub(stats.foot_w / 2),
                center_tile.1.saturating_sub(stats.foot_h / 2),
            );
            !ai_shared::footprints_respect_clearance(
                EntityKind::ResourceDepot,
                site.0,
                site.1,
                entity.kind,
                existing.0,
                existing.1,
            )
        })
}

pub(super) fn positions(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    security: &ExpansionSecurity,
) -> Vec<(f32, f32)> {
    let Some(site) = security.site else {
        return Vec::new();
    };
    let Some(center) = building_center(site, EntityKind::ResourceDepot, observation.map.tile_size)
    else {
        return Vec::new();
    };
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let target = observation
        .players
        .iter()
        .filter(|p| p.is_alive && observation.is_enemy_player(p.id))
        .min_by_key(|p| p.id)
        .map(|p| tile_center(p.start_tile, observation.map.tile_size))
        .unwrap_or(center);
    let crossroads_direction = defense::crossroads_wall_aware_approach_direction(observation);
    let direction = crossroads_direction
        .or_else(|| normalized_direction(center, target))
        .or_else(|| normalized_direction(home, center))
        .unwrap_or((1.0, 0.0));
    let forward_tiles = if crossroads_direction.is_some() {
        4.5
    } else {
        3.5
    };
    let ts = observation.map.tile_size as f32;
    let Some(depot) = config::building_stats(EntityKind::ResourceDepot) else {
        return Vec::new();
    };
    let sight =
        config::unit_stats(EntityKind::Rifleman).map_or(5.0, |stats| stats.sight_tiles as f32);
    let coverage_tiles = (sight - 1.0).min(10.0);
    // A guard may stop within the arrival tolerance. Keep its entire placement circle clear
    // even at the inward edge of that tolerance, otherwise our own screen blocks construction.
    let guard_clearance = rts_rules::balance::unit_placement_radius(EntityKind::Rifleman)
        + (defense::EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES + 0.25) * ts;
    let depot_rect = (
        site.0 as f32 * ts,
        site.1 as f32 * ts,
        site.0.saturating_add(depot.foot_w) as f32 * ts,
        site.1.saturating_add(depot.foot_h) as f32 * ts,
    );
    let mut points = Vec::new();
    for lateral in [-4.5, -1.5, 1.5, 4.5] {
        let desired = (
            center.0 + (direction.0 * forward_tiles - direction.1 * lateral) * ts,
            center.1 + (direction.1 * forward_tiles + direction.0 * lateral) * ts,
        );
        if let Some(point) = defense::separated_rifle_position_where(
            observation,
            analysis,
            desired,
            direction,
            &points,
            |point| {
                !crate::sdk::unit_circle_touches_rect(point, guard_clearance, depot_rect)
                    && dist2(point.0, point.1, center.0, center.1) <= squared(coverage_tiles * ts)
            },
        ) {
            points.push(point);
        }
    }
    points
}

/// Once the second Depot has an actual foundation, stage armor between it and the Rifle screen.
/// Deriving this from the observed building keeps the anchor stable while build intents retry.
pub(super) fn tank_staging_center(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
) -> Option<(f32, f32)> {
    let home = tile_center(observation.own_start_tile, observation.map.tile_size);
    let mut depots = observation
        .owned
        .iter()
        .filter(|entity| {
            entity.kind == EntityKind::ResourceDepot && entity.is_complete && entity.hp > 0
        })
        .collect::<Vec<_>>();
    depots.sort_by(|left, right| {
        dist2(left.x, left.y, home.0, home.1)
            .total_cmp(&dist2(right.x, right.y, home.0, home.1))
            .then_with(|| left.id.cmp(&right.id))
    });
    if depots.len() < 2 {
        return None;
    }
    depots.remove(0);
    let target = observation
        .players
        .iter()
        .filter(|player| player.is_alive && observation.is_enemy_player(player.id))
        .min_by_key(|player| player.id)
        .map(|player| tile_center(player.start_tile, observation.map.tile_size))?;
    let forward = normalized_direction(home, target)?;
    let expansion = *depots.iter().max_by(|left, right| {
        let progress = |depot: &&AiEntitySummary| {
            (depot.x - home.0) * forward.0 + (depot.y - home.1) * forward.1
        };
        progress(left)
            .total_cmp(&progress(right))
            .then_with(|| right.id.cmp(&left.id))
    })?;
    let direction = defense::crossroads_wall_aware_approach_direction(observation)
        .or_else(|| normalized_direction((expansion.x, expansion.y), target))?;
    let perpendicular = (-direction.1, direction.0);
    let ts = observation.map.tile_size as f32;
    let desired = (
        expansion.x + direction.0 * TANK_FRONT_OFFSET_TILES * ts,
        expansion.y + direction.1 * TANK_FRONT_OFFSET_TILES * ts,
    );
    let stats = config::building_stats(EntityKind::ResourceDepot)?;
    let depot_rect = (
        expansion.x - stats.foot_w as f32 * ts * 0.5,
        expansion.y - stats.foot_h as f32 * ts * 0.5,
        expansion.x + stats.foot_w as f32 * ts * 0.5,
        expansion.y + stats.foot_h as f32 * ts * 0.5,
    );
    let tank_clearance = rts_rules::balance::unit_placement_radius(EntityKind::Tank) + 0.25 * ts;
    defense::separated_rifle_position_where(
        observation,
        analysis,
        desired,
        direction,
        &[],
        |point| {
            let offset = (point.0 - expansion.x, point.1 - expansion.y);
            let forward = offset.0 * direction.0 + offset.1 * direction.1;
            let lateral = (offset.0 * perpendicular.0 + offset.1 * perpendicular.1).abs();
            forward >= 2.5 * ts
                && forward <= 4.0 * ts
                && lateral <= 2.0 * ts
                && !crate::sdk::unit_circle_touches_rect(point, tank_clearance, depot_rect)
        },
    )
}

pub(super) fn surplus_tank_for_forward_base(
    observation: &AiObservation,
    memory: &AiDecisionMemory,
) -> Option<u32> {
    if memory.containment_recovery_active {
        return None;
    }
    observation
        .owned
        .iter()
        .filter(|unit| {
            unit.kind == EntityKind::Tank
                && unit.is_complete
                && unit.hp > 0
                && unit.free_for_combat
                && Some(unit.id) != memory.home_defensive_tank
                && !memory.containment_active_tanks.contains(&unit.id)
                && !memory.containment_opening_tanks.contains(&unit.id)
        })
        .min_by_key(|unit| unit.id)
        .map(|unit| unit.id)
}

pub(super) fn update_and_stage(
    observation: &AiObservation,
    analysis: Option<&AiMapAnalysis>,
    memory: &mut AiDecisionMemory,
    actions: &mut AiActionContext<'_>,
) -> bool {
    let security = &mut memory.expansion_security;
    let Some(site) = security.site else {
        return false;
    };
    let Some(center) = building_center(site, EntityKind::ResourceDepot, observation.map.tile_size)
    else {
        return false;
    };
    let points = positions(observation, analysis, security);
    let ts = observation.map.tile_size as f32;
    let contested = observation.visible_enemies.iter().any(|enemy| {
        enemy.hp > 0
            && (enemy.kind.is_unit() || enemy.kind.is_building())
            && dist2(enemy.x, enemy.y, center.0, center.1) < squared(11.0 * ts)
    });
    let Some(depot) = config::building_stats(EntityKind::ResourceDepot) else {
        return false;
    };
    let depot_rect = (
        site.0 as f32 * ts,
        site.1 as f32 * ts,
        site.0.saturating_add(depot.foot_w) as f32 * ts,
        site.1.saturating_add(depot.foot_h) as f32 * ts,
    );
    let mut assignments = Vec::new();
    for id in &security.riflemen {
        let Some(point) = security.slots.get(id).and_then(|slot| points.get(*slot)) else {
            continue;
        };
        let Some(unit) = observation.owned.iter().find(|unit| unit.id == *id) else {
            continue;
        };
        let close = dist2(unit.x, unit.y, point.0, point.1)
            <= squared(defense::EXPANSION_DEFENSIVE_LINE_REISSUE_EPS_TILES * ts);
        let footprint_clear = !crate::sdk::unit_circle_touches_rect(
            (unit.x, unit.y),
            rts_rules::balance::unit_placement_radius(unit.kind),
            depot_rect,
        );
        assignments.push((*id, *point, close, footprint_clear, unit.state));
    }
    let qualified = assignments
        .iter()
        .filter_map(|(id, _, _, footprint_clear, _)| {
            let unit = observation.owned.iter().find(|unit| unit.id == *id)?;
            (*footprint_clear
                && dist2(unit.x, unit.y, center.0, center.1) <= squared(GUARD_COVERAGE_TILES * ts))
            .then_some((*id, (unit.x, unit.y)))
        })
        .collect::<Vec<_>>();
    let arrived = qualified.iter().enumerate().any(|(index, (_, left))| {
        qualified.iter().skip(index + 1).any(|(_, right)| {
            dist2(left.0, left.1, right.0, right.1) >= squared(MIN_PARTY_SEPARATION_TILES * ts)
        })
    });

    let progress_epsilon = (ESCORT_PROGRESS_EPS_TILES * 1000.0) as u32;
    let mut stalled = Vec::new();
    for (id, point, _, footprint_clear, _) in &assignments {
        let Some(unit) = observation.owned.iter().find(|unit| unit.id == *id) else {
            continue;
        };
        let covering_site = *footprint_clear
            && dist2(unit.x, unit.y, center.0, center.1) <= squared(GUARD_COVERAGE_TILES * ts);
        if covering_site {
            security.progress.remove(id);
            continue;
        }
        let distance_milli_tiles =
            (dist2(unit.x, unit.y, point.0, point.1).sqrt() / ts * 1000.0) as u32;
        let progress = security.progress.entry(*id).or_insert(EscortProgress {
            best_distance_milli_tiles: distance_milli_tiles,
            last_progress_tick: observation.tick,
        });
        if distance_milli_tiles.saturating_add(progress_epsilon)
            < progress.best_distance_milli_tiles
        {
            progress.best_distance_milli_tiles = distance_milli_tiles;
            progress.last_progress_tick = observation.tick;
        } else if observation.tick.saturating_sub(progress.last_progress_tick) >= ESCORT_STALL_TICKS
        {
            stalled.push(*id);
        }
    }
    // On contact, leave the guards unclaimed so local incident handling can use them. Once the
    // area clears, their normal staging orders bring them back to their assigned posts.
    if !contested {
        for (id, point, close, footprint_clear, _) in assignments {
            if close && footprint_clear {
                actions::hold_position_units(actions, [id]);
            } else {
                actions::attack_move_units(actions, [id], point.0, point.1);
            }
        }
    }
    if arrived && !contested {
        let since = *security.secure_since.get_or_insert(observation.tick);
        let secured = observation.tick.saturating_sub(since) >= SECURE_TICKS;
        if secured {
            return true;
        }
    } else {
        security.secure_since = None;
    }
    for id in stalled {
        security.riflemen.retain(|candidate| *candidate != id);
        security.slots.remove(&id);
        security.progress.remove(&id);
        security
            .retry_after
            .insert(id, observation.tick.saturating_add(ESCORT_RETRY_TICKS));
    }
    false
}
