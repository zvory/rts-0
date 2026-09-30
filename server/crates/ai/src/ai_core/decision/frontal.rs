use super::geometry::{clamp_to_map, dist2, normalized_direction, tile_center};
use super::*;

pub(super) mod approach;
mod catch_up;
mod containment;
mod formation;
#[cfg(test)]
mod formation_tests;
mod legacy_beta;
pub(super) mod smoke;

use self::catch_up::*;
use self::containment::issue_expansion_containment_wave;
use self::formation::*;
#[cfg(test)]
use self::legacy_beta::{compact_group_near, containment_regroup_radius_tiles};
use self::smoke::*;
use crate::ai_core::profiles::JEFFS_AI_PRE_TANK_CATCHUP_ID;
use rts_rules::faction::AbilityKind;

const ENDGAME_SEARCH_OFFSETS: [(f32, f32); 17] = [
    (0.0, 0.0),
    (8.0, 0.0),
    (8.0, 8.0),
    (0.0, 8.0),
    (-8.0, 8.0),
    (-8.0, 0.0),
    (-8.0, -8.0),
    (0.0, -8.0),
    (8.0, -8.0),
    (16.0, 0.0),
    (16.0, 16.0),
    (0.0, 16.0),
    (-16.0, 16.0),
    (-16.0, 0.0),
    (-16.0, -16.0),
    (0.0, -16.0),
    (16.0, -16.0),
];

pub(super) const OUTBOUND_WAVE_VISIBLE_TARGET_RADIUS_TILES: f32 = 14.0;
const RIFLE_SCREEN_FORWARD_TILES: f32 = 2.0;
const RIFLE_SCREEN_SPACING_TILES: f32 = 2.0;
const RIFLE_SCREEN_SECOND_RANK_BACK_TILES: f32 = 1.5;
const RIFLE_SCREEN_FIRST_RANK: usize = 4;
const MAX_CONTAINMENT_RIFLE_ESCORTS: usize = 6;
const MIN_CONTAINMENT_RIFLE_ESCORTS: usize = 2;
const CONTAINMENT_HOME_RIFLE_RESERVE: usize = 4;
const CONTAINMENT_ESCORT_SELECTION_RADIUS_TILES: f32 = 12.0;
const CONTAINMENT_TANK_SPACING_TILES: f32 = 1.5;
const CONTAINMENT_ASSEMBLY_TOLERANCE_TILES: f32 = 1.75;
const CONTAINMENT_LONGITUDINAL_SPREAD_TILES: f32 = 2.0;
const CONTAINMENT_LATERAL_SLOP_TILES: f32 = 1.0;
const CONTAINMENT_RIFLE_COHESION_TILES: f32 = 7.0;
const CONTAINMENT_MARCH_STEP_TILES: f32 = 8.0;
/// A marching push halts for ordinary combat units this close; anti-armor threats halt it from
/// the policy's contact range.
const MARCH_CLOSE_CONTACT_TILES: f32 = 10.0;
const CONTAINMENT_FORMATION_REISSUE_TICKS: u32 = config::TICK_HZ * 2;
const CONTAINMENT_ASSEMBLY_TIMEOUT_TICKS: u32 = config::TICK_HZ * 8;
const CONTAINMENT_ASSEMBLY_HARD_TIMEOUT_TICKS: u32 = config::TICK_HZ * 12;
/// On Crossroads the push does not assemble with fewer Tanks than this.
pub(super) const CROSSROADS_PUSH_MIN_TANKS: usize = 6;
/// On Crossroads the push only leaves with this many more Tanks than the enemy Tanks seen in the
/// last 90 seconds.
const CROSSROADS_PUSH_TANK_LEAD: usize = 3;
const RIVER_OPENING_GUARD_TICKS: u32 = config::TICK_HZ * 30;
const RIVER_OPENING_CLEAR_TICKS: u32 = config::TICK_HZ * 5;
const RIVER_OPENING_PRESSURE_RADIUS_TILES: f32 = 22.0;
const CONTAINMENT_WAYPOINT_TIMEOUT_TICKS: u32 = config::TICK_HZ * 4;
const CONTAINMENT_CONTACT_MEMORY_TICKS: u32 = config::TICK_HZ * 2;
const CONTAINMENT_FOCUS_STABLE_TICKS: u32 = 9;
const CONTAINMENT_SMOKE_RANGE_TILES: f32 = 13.5;
const CONTAINMENT_SMOKE_BASE_RADIUS_TILES: f32 = 2.0;
const CONTAINMENT_SMOKE_PLUS_RADIUS_TILES: f32 = 4.0;
const CONTAINMENT_SMOKE_SAFETY_TILES: f32 = 0.5;
const CONTAINMENT_SMOKE_DURATION_TICKS: u32 = config::TICK_HZ * 5;
const CONTAINMENT_SMOKE_AIM_INSET_TILES: f32 = 0.5;
const CONTAINMENT_LOCAL_SMOKE_TARGET_TILES: f32 = 22.0;
const CONTAINMENT_SCOUT_SMOKE_FORWARD_LIMIT_TILES: f32 = 3.5;
const CONTAINMENT_SCOUT_SMOKE_REAR_LIMIT_TILES: f32 = 2.0;
const CONTAINMENT_SCOUT_SMOKE_LATERAL_LIMIT_TILES: f32 = 4.5;
const MAX_LOCAL_DEFENSE_SMOKE_TANKS: usize = 2;
const TANK_VOLLEY_DAMAGE: u32 = 60;
const RIFLE_THREAT_LEASH_TILES: f32 = 4.0;
const RIFLE_THREAT_SECTOR_HALF_WIDTH_TILES: f32 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum FrontalWaveBlocker {
    WaitingForUnits,
    WaitingForTank,
    WaitingForMethamphetamines,
    Staging,
    AttackCadence,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct FrontalWavePlan {
    pub(super) ready_units: Vec<u32>,
    pub(super) desired_size: usize,
    pub(super) attack_due: bool,
    pub(super) required_unit_ready: bool,
    pub(super) methamphetamines_ready: bool,
    pub(super) blockers: Vec<FrontalWaveBlocker>,
}

impl FrontalWavePlan {
    pub(super) fn should_attack(&self) -> bool {
        self.blockers.is_empty()
    }

    pub(super) fn should_stage(&self) -> bool {
        !self.ready_units.is_empty() && !self.should_attack()
    }

    /// The same plan with no units free to attack or stage: only a push already under way is
    /// commanded.
    pub(super) fn push_only(&self) -> Self {
        Self {
            ready_units: Vec::new(),
            blockers: vec![FrontalWaveBlocker::WaitingForUnits],
            ..self.clone()
        }
    }
}

pub(super) fn plan_frontal_wave(
    observation: &AiObservation,
    attack: AttackPolicy,
    memory: &mut AiDecisionMemory,
    profile: &AiProfile,
    excluded_units: &BTreeSet<u32>,
) -> FrontalWavePlan {
    let owned_units: BTreeSet<u32> = observation.owned.iter().map(|entity| entity.id).collect();
    let launched_units =
        memory.launched_frontal_unit_exclusions(profile, observation.tick, &owned_units);
    let mut excluded_units = excluded_units.clone();
    excluded_units.extend(launched_units);
    let ready_units = actions::select_ready_combat_units_excluding(
        &observation.owned,
        attack.unit_kinds,
        &excluded_units,
    );
    let desired_size = memory.desired_attack_size_for(profile, attack, observation.tick);
    let attack_due = memory.attack_due_for(profile, attack, observation.tick);
    let required_unit_ready = attack
        .required_unit
        .map(|kind| {
            observation
                .owned
                .iter()
                .any(|entity| entity.kind == kind && ready_units.contains(&entity.id))
        })
        .unwrap_or(true);
    let methamphetamines_ready = profile.fast_tank_timing.is_some()
        || !attack.unit_kinds.contains(&EntityKind::Tank)
        || observation
            .upgrades
            .contains(&UpgradeKind::Methamphetamines);

    let mut blockers = Vec::new();
    if ready_units.len() < desired_size {
        blockers.push(FrontalWaveBlocker::WaitingForUnits);
    }
    if !required_unit_ready && attack.required_unit == Some(EntityKind::Tank) {
        blockers.push(FrontalWaveBlocker::WaitingForTank);
    } else if !required_unit_ready {
        blockers.push(FrontalWaveBlocker::WaitingForUnits);
    }
    if !methamphetamines_ready {
        blockers.push(FrontalWaveBlocker::WaitingForMethamphetamines);
    }
    if !attack_due {
        blockers.push(FrontalWaveBlocker::AttackCadence);
    }
    blockers.sort();
    blockers.dedup();

    FrontalWavePlan {
        ready_units,
        desired_size,
        attack_due,
        required_unit_ready,
        methamphetamines_ready,
        blockers,
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn issue_frontal_wave(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    profile: &AiProfile,
    attack: AttackPolicy,
    plan: &FrontalWavePlan,
    enemy_base: EnemyBaseFact,
    map_analysis: Option<&AiMapAnalysis>,
    recall_target: Option<u32>,
    memory: &mut AiDecisionMemory,
) -> Option<AiIntent> {
    let containment_active = memory.containment.wave_launched
        || memory.containment.recovery_active
        || !memory.containment.active_tanks.is_empty();
    if let Some(containment) = profile.expansion_containment {
        if profile.id != JEFFS_AI_BETA_ID {
            if let Some(target) = recall_target {
                if let Some(intent) = issue_containment_recall(actions, observation, memory, target)
                {
                    return Some(intent);
                }
            } else if memory.containment.recall_active {
                memory.containment.recall_active = false;
                memory.containment.last_formation_command_tick = None;
                reset_containment_route(memory);
            }
        }
        let relaxed_start = !containment_active
            && plan.attack_due
            && plan.required_unit_ready
            && plan.methamphetamines_ready
            && plan.ready_units.len() >= containment.minimum_tanks_to_continue + 3;
        if profile.id == JEFFS_AI_BETA_ID && plan.should_attack() {
            if let Some(intent) = legacy_beta::issue_expansion_containment_wave(
                actions,
                observation,
                plan,
                enemy_base,
                containment,
                true,
                memory,
            ) {
                return Some(intent);
            }
        } else if profile.id != JEFFS_AI_BETA_ID
            && (plan.should_attack() || relaxed_start || containment_active)
        {
            let orders_start = actions.emitted_len();
            let intent = issue_expansion_containment_wave(
                actions,
                observation,
                plan,
                enemy_base,
                containment,
                is_jeffs_ai_profile(profile.id),
                profile.id != JEFFS_AI_PRE_TANK_CATCHUP_ID,
                uses_current_jeffs_ai_policy(profile.id),
                map_analysis,
                memory,
            );
            note_containment_holds(actions, memory, orders_start);
            if intent.is_some() {
                return intent;
            }
        }
    }

    // Armor too few for the push stays staged at home: the plain attack wave would send the same
    // two or three Tanks, home reserve included, that the push is waiting to outgrow. This holds
    // for the current Jeff everywhere and for any Jeff on Crossroads.
    let hold_for_push = profile.expansion_containment.is_some()
        && profile.id != JEFFS_AI_BETA_ID
        && attack.unit_kinds.contains(&EntityKind::Tank)
        && (uses_current_jeffs_ai_policy(profile.id)
            || defense::crossroads_wall_aware_approach_direction(observation).is_some());
    if plan.should_attack() && !hold_for_push {
        let attack_units =
            if let Some(target) = visible_combat_target_for_wave(observation, &plan.ready_units) {
                actions::attack_units(actions, plan.ready_units.clone(), target)
            } else {
                actions::attack_move_units(
                    actions,
                    plan.ready_units.clone(),
                    enemy_base.x,
                    enemy_base.y,
                )
            };
        return attack_units.map(|units| AiIntent::Attack { units });
    }

    if !plan.should_stage() && !(hold_for_push && !plan.ready_units.is_empty()) {
        return None;
    }

    // The current Jeff holds its armor on the home post once that has moved off the main's line:
    // toward the natural, or kept after the main's steel ran out.
    let home_post = memory
        .home_post
        .filter(|post| uses_current_jeffs_ai_policy(profile.id) && !post.on_main_line);
    let staged = if let Some(post) = home_post {
        defense::stage_defensive_line_at(
            actions,
            observation,
            &plan.ready_units,
            post.center(),
            post.facing(),
        )
    } else if profile.frontal_wave.line_staging {
        stage_main_steel_defensive_line(
            actions,
            observation,
            &plan.ready_units,
            enemy_base,
            attack.stage_distance_tiles,
        )
    } else {
        let own_base = tile_center(observation.own_start_tile, observation.map.tile_size);
        actions::stage_units_toward(
            actions,
            plan.ready_units.clone(),
            own_base,
            (enemy_base.x, enemy_base.y),
            observation.map.tile_size,
            attack.stage_distance_tiles,
        )
    };
    staged.map(|units| AiIntent::Stage { units })
}

pub(super) fn containment_wave_needs_control(memory: &AiDecisionMemory) -> bool {
    memory.containment.wave_launched
        || memory.containment.recovery_active
        || !memory.containment.active_tanks.is_empty()
}

pub(super) fn sync_containment_recovery(
    observation: &AiObservation,
    profile: &AiProfile,
    memory: &mut AiDecisionMemory,
) {
    let Some(_) = profile.expansion_containment else {
        memory.containment.recovery_active = false;
        memory.containment.active_tanks.clear();
        memory.containment.active_scout = None;
        memory.containment.active_riflemen.clear();
        memory.containment.march_waypoint = None;
        memory.containment.route.clear();
        memory.containment.route_index = 0;
        memory.containment.route_objective = None;
        memory.containment.last_formation_command_tick = None;
        memory.containment.assembly_started_tick = None;
        memory.containment.waypoint_started_tick = None;
        memory.containment.recall_active = false;
        memory.containment.contact_last_tick = None;
        return;
    };
    if !memory.containment.wave_launched || memory.enemy_main_destroyed {
        return;
    }
    if memory.containment.recovery_active || memory.containment.active_tanks.is_empty() {
        return;
    }
    let owned: BTreeSet<u32> = observation.owned.iter().map(|entity| entity.id).collect();
    // The current Jeff's push, most of its Tanks, carries on through losses until half of it is gone
    // or it drops below the policy minimum. Other profiles fall back on the first loss.
    let tanks_left = memory
        .containment
        .active_tanks
        .iter()
        .filter(|tank| owned.contains(tank))
        .count();
    let tanks_intact = if uses_current_jeffs_ai_policy(profile.id) {
        tanks_left * 2 >= memory.containment.launch_tanks
            && profile
                .expansion_containment
                .is_some_and(|policy| tanks_left >= policy.minimum_tanks_to_continue)
    } else {
        tanks_left == memory.containment.active_tanks.len()
    };
    let scout_intact = memory
        .containment
        .active_scout
        .is_some_and(|scout| owned.contains(&scout));
    if tanks_intact && scout_intact {
        return;
    }
    // The current Jeff comes at the target from another side next time.
    if uses_current_jeffs_ai_policy(profile.id) {
        memory
            .approach
            .note_failed_push(observation.player_id, observation.tick);
    }
    begin_containment_recovery(memory);
}

/// End the current push: the next one assembles at the regroup point, one Tank larger.
/// A launched push this small comes home when enemy Tanks shelling a base outnumber the home Tanks.
const SMALL_PUSH_TANKS: usize = 4;

/// Bring a small launched push home to the home post (the HQ without one). It re-forms from there
/// like a push that fell back, but a recall is not a failed push, so the next push is no larger.
/// In the lost games 2-4 Tanks sat out at the enemy's bases while 2-6 AI Tanks took the natural.
pub(super) fn recall_small_push_home(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &mut AiDecisionMemory,
) -> Option<Vec<u32>> {
    let push = &memory.containment;
    if !push.wave_launched
        || push.recovery_active
        || push.active_tanks.is_empty()
        || push.active_tanks.len() > SMALL_PUSH_TANKS
        || memory.enemy_main_destroyed
    {
        return None;
    }
    let owned: BTreeSet<u32> = observation.owned.iter().map(|unit| unit.id).collect();
    let units: Vec<u32> = push
        .active_tanks
        .iter()
        .copied()
        .chain(push.active_scout)
        .chain(push.active_riflemen.iter().copied())
        .filter(|id| owned.contains(id))
        .collect();
    let home = memory.home_post.map_or_else(
        || tile_center(observation.own_start_tile, observation.map.tile_size),
        |post| post.center(),
    );
    let repush_count = memory.containment.repush_count;
    begin_containment_recovery(memory);
    memory.containment.repush_count = repush_count;
    actions::move_units(actions, units, home.0, home.1)
}

fn begin_containment_recovery(memory: &mut AiDecisionMemory) {
    memory.containment.repush_count = memory.containment.repush_count.saturating_add(1);
    memory.containment.recovery_active = true;
    memory.containment.active_tanks.clear();
    memory.containment.active_scout = None;
    memory.containment.active_riflemen.clear();
    memory.containment.march_waypoint = None;
    memory.containment.route.clear();
    memory.containment.route_index = 0;
    memory.containment.route_objective = None;
    memory.containment.last_formation_command_tick = None;
    memory.containment.assembly_started_tick = None;
    memory.containment.waypoint_started_tick = None;
    memory.containment.stationary_since = None;
    memory.containment.contact_last_tick = None;
}

/// Record which units the push left holding: a Hold marks a unit, any other order replaces it.
fn note_containment_holds(
    actions: &AiActionContext<'_>,
    memory: &mut AiDecisionMemory,
    orders_start: usize,
) {
    for (unit, hold) in actions.unit_orders_since(orders_start) {
        if hold {
            memory.containment.held_tanks.insert(unit);
        } else {
            memory.containment.held_tanks.remove(&unit);
        }
    }
}

/// A Tank the push put on Hold that is still standing (not moving or under an attack order).
/// It shoots whatever enters its range on its own and never chases, so it needs no new order.
fn tank_is_holding(observation: &AiObservation, memory: &AiDecisionMemory, tank_id: u32) -> bool {
    memory.containment.held_tanks.contains(&tank_id)
        && observation
            .owned
            .iter()
            .any(|unit| unit.id == tank_id && unit.state == AiEntityState::Idle)
}

/// Hold the given push Tanks, skipping those already holding. Holding again would clear the
/// target a holding Tank picked for itself: while it reloads the turret swings back toward the
/// hull, and it has to re-acquire and turn again before the next shot.
fn hold_containment_tanks(
    actions: &mut AiActionContext<'_>,
    observation: &AiObservation,
    memory: &AiDecisionMemory,
    tanks: impl IntoIterator<Item = u32>,
) -> Option<Vec<u32>> {
    actions::hold_position_units(
        actions,
        tanks
            .into_iter()
            .filter(|tank| !tank_is_holding(observation, memory, *tank)),
    )
}

fn river_opening_guard_active(
    observation: &AiObservation,
    tanks: &[u32],
    assembly_started: u32,
    memory: &mut AiDecisionMemory,
) -> bool {
    let pressure_visible =
        visible_combat_target_within_tiles(observation, tanks, RIVER_OPENING_PRESSURE_RADIUS_TILES)
            .is_some();
    if pressure_visible {
        memory.containment.contact_last_tick = Some(observation.tick);
    }
    let pressure_recent = memory
        .containment
        .contact_last_tick
        .is_some_and(|last| observation.tick.saturating_sub(last) <= RIVER_OPENING_CLEAR_TICKS);
    observation.tick.saturating_sub(assembly_started) < RIVER_OPENING_GUARD_TICKS || pressure_recent
}

fn update_enemy_main_state(
    observation: &AiObservation,
    enemy_base: EnemyBaseFact,
    tanks: &[u32],
    scouts: &[u32],
    memory: &mut AiDecisionMemory,
) {
    if memory.enemy_main_destroyed {
        return;
    }
    let tile_size = observation.map.tile_size as f32;
    let main_radius2 = (8.0 * tile_size).powi(2);
    let visible_main = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::ResourceDepot)
        .filter(|enemy| dist2(enemy.x, enemy.y, enemy_base.x, enemy_base.y) <= main_radius2)
        .min_by_key(|enemy| enemy.id);
    if let Some(resource_depot) = visible_main {
        memory.enemy_main_resource_depot = Some(resource_depot.id);
        return;
    }
    if memory.enemy_main_resource_depot.is_none() {
        return;
    }
    let confirmation_radius2 = (14.0 * tile_size).powi(2);
    let force_confirms_site = observation
        .owned
        .iter()
        .filter(|unit| tanks.contains(&unit.id) || scouts.contains(&unit.id))
        .any(|unit| dist2(unit.x, unit.y, enemy_base.x, enemy_base.y) <= confirmation_radius2);
    if force_confirms_site {
        memory.enemy_main_destroyed = true;
        memory.endgame_search_waypoint = 0;
        memory.containment.stationary_since = None;
    }
}

fn endgame_search_point(
    own_base: (f32, f32),
    enemy_base: EnemyBaseFact,
    map: AiMapSummary,
    waypoint: usize,
) -> (f32, f32) {
    let offset = ENDGAME_SEARCH_OFFSETS[waypoint % ENDGAME_SEARCH_OFFSETS.len()];
    let orientation = canonical_half_turn_orientation(own_base, (enemy_base.x, enemy_base.y));
    let tile_size = map.tile_size as f32;
    clamp_to_map(
        (
            enemy_base.x + offset.0 * tile_size * orientation,
            enemy_base.y + offset.1 * tile_size * orientation,
        ),
        map,
    )
}

fn update_enemy_natural_state(
    observation: &AiObservation,
    natural: (f32, f32),
    enemy_base: EnemyBaseFact,
    scouts: &[u32],
    memory: &mut AiDecisionMemory,
) {
    if memory.enemy_natural_destroyed {
        return;
    }
    let tile_size = observation.map.tile_size as f32;
    let natural_radius2 = (8.0 * tile_size) * (8.0 * tile_size);
    let main_exclusion2 = (config::START_RESOURCE_MAX_DIST_TILES * tile_size)
        * (config::START_RESOURCE_MAX_DIST_TILES * tile_size);
    let visible_natural = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::ResourceDepot)
        .filter(|enemy| dist2(enemy.x, enemy.y, enemy_base.x, enemy_base.y) > main_exclusion2)
        .filter(|enemy| dist2(enemy.x, enemy.y, natural.0, natural.1) <= natural_radius2)
        .min_by_key(|enemy| enemy.id);
    if let Some(resource_depot) = visible_natural {
        memory.enemy_natural_resource_depot = Some(resource_depot.id);
        return;
    }
    // At the containment anchor the Tanks sit 13.5 tiles from the resource
    // edge and the Scout moves two tiles ahead. Allow one tile of formation
    // separation so that the intended 11.5-tile observation point can
    // confirm that a destroyed (or absent) natural is clear.
    let scout_confirmation_tiles = 12.5;
    let scout_confirms_site = observation
        .owned
        .iter()
        .filter(|unit| scouts.contains(&unit.id))
        .any(|unit| {
            dist2(unit.x, unit.y, natural.0, natural.1)
                <= (scout_confirmation_tiles * tile_size).powi(2)
        });
    if scout_confirms_site {
        memory.enemy_natural_destroyed = true;
        memory.containment.stationary_since = None;
    }
}

fn containment_points(
    own_base: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
    policy: ExpansionContainmentPolicy,
) -> Option<((f32, f32), (f32, f32))> {
    let toward_expansion = normalized_direction(own_base, objective)?;
    let tile_size = map.tile_size as f32;
    let perpendicular = (-toward_expansion.1, toward_expansion.0);
    let approach_origin = (
        own_base.0 + perpendicular.0 * policy.flank_tiles * tile_size,
        own_base.1 + perpendicular.1 * policy.flank_tiles * tile_size,
    );
    let toward_expansion = normalized_direction(approach_origin, objective)?;
    let tank_point = clamp_to_map(
        (
            objective.0 - toward_expansion.0 * policy.tank_standoff_tiles * tile_size,
            objective.1 - toward_expansion.1 * policy.tank_standoff_tiles * tile_size,
        ),
        map,
    );
    let scout_point = clamp_to_map(
        (
            tank_point.0 + toward_expansion.0 * policy.scout_forward_tiles * tile_size,
            tank_point.1 + toward_expansion.1 * policy.scout_forward_tiles * tile_size,
        ),
        map,
    );
    Some((tank_point, scout_point))
}

/// Express half-turn-sensitive search offsets in Jeff's local own-base-to-enemy frame. A
/// rotationally mirrored start flips both axes, so the same local search pattern mirrors instead
/// of retaining a global top-left bias.
fn canonical_half_turn_orientation(from: (f32, f32), to: (f32, f32)) -> f32 {
    if from.0 < to.0 || (from.0 == to.0 && from.1 <= to.1) {
        1.0
    } else {
        -1.0
    }
}

fn scout_trailing_point(
    tank_center: (f32, f32),
    own_base: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
    trailing_tiles: f32,
) -> Option<(f32, f32)> {
    let toward_expansion = normalized_direction(own_base, objective)?;
    let tile_size = map.tile_size as f32;
    Some(clamp_to_map(
        (
            tank_center.0 - toward_expansion.0 * trailing_tiles * tile_size,
            tank_center.1 - toward_expansion.1 * trailing_tiles * tile_size,
        ),
        map,
    ))
}

fn scout_forward_from_tanks(
    tank_center: (f32, f32),
    own_base: (f32, f32),
    objective: (f32, f32),
    map: AiMapSummary,
    forward_tiles: f32,
) -> Option<(f32, f32)> {
    let toward_expansion = normalized_direction(own_base, objective)?;
    let tile_size = map.tile_size as f32;
    Some(clamp_to_map(
        (
            tank_center.0 + toward_expansion.0 * forward_tiles * tile_size,
            tank_center.1 + toward_expansion.1 * forward_tiles * tile_size,
        ),
        map,
    ))
}

fn compact_tank_formation_assignments(
    observation: &AiObservation,
    tank_ids: &[u32],
    center: (f32, f32),
    toward_objective: (f32, f32),
    map: AiMapSummary,
    spacing_tiles: f32,
) -> Vec<(u32, (f32, f32))> {
    let mut tank_ids = tank_ids.to_vec();
    let perpendicular = (-toward_objective.1, toward_objective.0);
    let by_id: BTreeMap<u32, &AiEntitySummary> = observation
        .owned
        .iter()
        .map(|unit| (unit.id, unit))
        .collect();
    let along = |id: &u32, axis: (f32, f32)| {
        by_id
            .get(id)
            .map(|unit| unit.x * axis.0 + unit.y * axis.1)
            .unwrap_or(0.0)
    };
    // A large push forms ranks: the frontmost Tanks take the front rank, the next ones the rank
    // behind. One line abreast of 18 Tanks would be 26 tiles wide.
    tank_ids.sort_by(|left, right| {
        along(right, toward_objective)
            .total_cmp(&along(left, toward_objective))
            .then_with(|| left.cmp(right))
    });
    let tile_size = map.tile_size as f32;
    let mut assignments = Vec::with_capacity(tank_ids.len());
    for (rank, rank_ids) in tank_ids.chunks(TANK_FORMATION_RANK_WIDTH).enumerate() {
        let mut rank_ids = rank_ids.to_vec();
        rank_ids.sort_by(|left, right| {
            along(left, perpendicular)
                .total_cmp(&along(right, perpendicular))
                .then_with(|| left.cmp(right))
        });
        let middle = rank_ids.len().saturating_sub(1) as f32 / 2.0;
        let back = rank as f32 * TANK_FORMATION_RANK_DEPTH_TILES * tile_size;
        for (index, tank_id) in rank_ids.into_iter().enumerate() {
            let offset = (index as f32 - middle) * spacing_tiles * tile_size;
            assignments.push((
                tank_id,
                clamp_to_map(
                    (
                        center.0 + perpendicular.0 * offset - toward_objective.0 * back,
                        center.1 + perpendicular.1 * offset - toward_objective.1 * back,
                    ),
                    map,
                ),
            ));
        }
    }
    assignments
}

/// Tanks per rank of a push formation, and how far each rank sits behind the one in front.
const TANK_FORMATION_RANK_WIDTH: usize = 6;
const TANK_FORMATION_RANK_DEPTH_TILES: f32 = 2.0;

fn frontmost_unit_position(
    observation: &AiObservation,
    unit_ids: &[u32],
    toward_objective: (f32, f32),
) -> Option<(f32, f32)> {
    observation
        .owned
        .iter()
        .filter(|unit| unit_ids.contains(&unit.id))
        .max_by(|left, right| {
            let left_progress = left.x * toward_objective.0 + left.y * toward_objective.1;
            let right_progress = right.x * toward_objective.0 + right.y * toward_objective.1;
            left_progress
                .total_cmp(&right_progress)
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|unit| (unit.x, unit.y))
}

fn enemy_natural_edge(
    observation: &AiObservation,
    enemy_base: EnemyBaseFact,
) -> Option<(f32, f32)> {
    let tile_size = observation.map.tile_size as f32;
    let start_exclusion = (config::START_RESOURCE_MAX_DIST_TILES + 1.5) * tile_size;
    let start_exclusion2 = start_exclusion * start_exclusion;
    observation
        .resources
        .iter()
        .filter(|resource| resource.kind == EntityKind::Steel && resource.remaining > 0)
        .filter(|resource| {
            dist2(resource.x, resource.y, enemy_base.x, enemy_base.y) > start_exclusion2
        })
        .min_by(|left, right| {
            dist2(left.x, left.y, enemy_base.x, enemy_base.y)
                .total_cmp(&dist2(right.x, right.y, enemy_base.x, enemy_base.y))
                .then_with(|| left.id.cmp(&right.id))
        })
        .map(|resource| (resource.x, resource.y))
}

fn tank_can_fire_at_visible_target(
    observation: &AiObservation,
    tank_id: u32,
    target_id: u32,
    range_tiles: f32,
) -> bool {
    let Some(tank) = observation.owned.iter().find(|unit| unit.id == tank_id) else {
        return false;
    };
    let Some(target) = observation
        .visible_enemies
        .iter()
        .find(|enemy| enemy.id == target_id)
    else {
        return false;
    };
    dist2(tank.x, tank.y, target.x, target.y)
        <= (range_tiles * observation.map.tile_size as f32).powi(2)
}

fn target_is_in_shared_tank_range(
    observation: &AiObservation,
    tanks: &[u32],
    target_id: u32,
    range_tiles: f32,
) -> bool {
    !tanks.is_empty()
        && tanks
            .iter()
            .all(|tank| tank_can_fire_at_visible_target(observation, *tank, target_id, range_tiles))
}

fn shared_stationary_tank_targets<'a>(
    observation: &'a AiObservation,
    tanks: &[u32],
    range_tiles: f32,
    excluded_target: Option<u32>,
) -> Vec<&'a AiEntitySummary> {
    let center = group_center(observation, tanks).unwrap_or((0.0, 0.0));
    let mut targets = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .filter(|enemy| Some(enemy.id) != excluded_target)
        .filter(|enemy| target_is_in_shared_tank_range(observation, tanks, enemy.id, range_tiles))
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| {
        stationary_tank_target_priority(left.kind)
            .cmp(&stationary_tank_target_priority(right.kind))
            .then_with(|| {
                (left.hp > TANK_VOLLEY_DAMAGE * tanks.len() as u32)
                    .cmp(&(right.hp > TANK_VOLLEY_DAMAGE * tanks.len() as u32))
            })
            .then_with(|| left.hp.cmp(&right.hp))
            .then_with(|| {
                dist2(center.0, center.1, left.x, left.y)
                    .total_cmp(&dist2(center.0, center.1, right.x, right.y))
            })
            .then_with(|| left.id.cmp(&right.id))
    });
    targets
}

fn shared_stationary_tank_target(
    observation: &AiObservation,
    tanks: &[u32],
    range_tiles: f32,
    preferred_target: Option<u32>,
    excluded_target: Option<u32>,
) -> Option<u32> {
    let targets = shared_stationary_tank_targets(observation, tanks, range_tiles, excluded_target);
    preferred_target
        .filter(|preferred| targets.iter().any(|target| target.id == *preferred))
        .or_else(|| targets.first().map(|target| target.id))
}

fn stationary_tank_target_priority(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::AntiTankGun => 0,
        EntityKind::Tank => 1,
        EntityKind::Panzerfaust => 2,
        EntityKind::Artillery | EntityKind::MortarTeam => 3,
        EntityKind::MachineGunner => 4,
        EntityKind::ScoutCar | EntityKind::Rifleman => 5,
        _ => 6,
    }
}

fn note_containment_focus(memory: &mut AiDecisionMemory, tick: u32, target: u32) {
    if memory.containment.focus_target == Some(target) {
        return;
    }
    memory.containment.focus_target = Some(target);
    memory.containment.focus_stable_since = Some(tick);
}

fn rifle_sector_target(
    observation: &AiObservation,
    rifleman: u32,
    screen_point: (f32, f32),
    tank_anchor: (f32, f32),
    objective: (f32, f32),
) -> Option<u32> {
    let rifle = observation.owned.iter().find(|unit| unit.id == rifleman)?;
    let direction = normalized_direction(tank_anchor, objective)?;
    let perpendicular = (-direction.1, direction.0);
    let tile_size = observation.map.tile_size as f32;
    if dist2(rifle.x, rifle.y, screen_point.0, screen_point.1)
        > (RIFLE_THREAT_LEASH_TILES * tile_size).powi(2)
    {
        return None;
    }
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            matches!(
                enemy.kind,
                EntityKind::Panzerfaust
                    | EntityKind::MachineGunner
                    | EntityKind::Rifleman
                    | EntityKind::ScoutCar
            )
        })
        .filter_map(|enemy| {
            let delta = (enemy.x - screen_point.0, enemy.y - screen_point.1);
            let lateral = (delta.0 * perpendicular.0 + delta.1 * perpendicular.1).abs();
            let longitudinal = delta.0 * direction.0 + delta.1 * direction.1;
            (lateral <= RIFLE_THREAT_SECTOR_HALF_WIDTH_TILES * tile_size
                && longitudinal >= -tile_size
                && longitudinal <= RIFLE_THREAT_LEASH_TILES * tile_size)
                .then_some((
                    enemy.id,
                    match enemy.kind {
                        EntityKind::Panzerfaust => 0,
                        EntityKind::MachineGunner => 1,
                        EntityKind::Rifleman => 2,
                        _ => 3,
                    },
                    dist2(rifle.x, rifle.y, enemy.x, enemy.y),
                ))
        })
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|target| target.0)
}

pub(super) fn visible_combat_target_for_wave(
    observation: &AiObservation,
    unit_ids: &[u32],
) -> Option<u32> {
    let center = group_center(observation, unit_ids)?;
    let max_distance = OUTBOUND_WAVE_VISIBLE_TARGET_RADIUS_TILES * observation.map.tile_size as f32;
    let max_distance2 = max_distance * max_distance;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .map(|enemy| {
            let distance2 = geometry::dist2(center.0, center.1, enemy.x, enemy.y);
            (
                enemy.id,
                outbound_wave_target_priority(enemy.kind),
                distance2,
            )
        })
        .filter(|(_, _, distance2)| *distance2 <= max_distance2)
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

fn visible_combat_target_within_tiles(
    observation: &AiObservation,
    unit_ids: &[u32],
    radius_tiles: f32,
) -> Option<u32> {
    let center = group_center(observation, unit_ids)?;
    let max_distance = radius_tiles * observation.map.tile_size as f32;
    let max_distance2 = max_distance * max_distance;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_unit() && enemy.kind != EntityKind::Worker)
        .map(|enemy| {
            (
                enemy.id,
                outbound_wave_target_priority(enemy.kind),
                geometry::dist2(center.0, center.1, enemy.x, enemy.y),
            )
        })
        .filter(|(_, _, distance2)| *distance2 <= max_distance2)
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

/// Enemies close enough, or dangerous enough, to halt a marching push and fight from where it
/// stands: anti-armor units and guns out to `stop_tiles`, other combat units within
/// `MARCH_CLOSE_CONTACT_TILES`. Halting for every Rifleman or Scout Car seen at 18 tiles made pushes
/// crawl across the map.
fn march_contact_target(
    observation: &AiObservation,
    unit_ids: &[u32],
    stop_tiles: f32,
) -> Option<u32> {
    let center = group_center(observation, unit_ids)?;
    let tile_size = observation.map.tile_size as f32;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            enemy.kind.is_unit()
                && !matches!(enemy.kind, EntityKind::Worker | EntityKind::ScoutPlane)
        })
        .filter_map(|enemy| {
            let reach = match enemy.kind {
                EntityKind::Tank
                | EntityKind::AntiTankGun
                | EntityKind::Panzerfaust
                | EntityKind::Artillery
                | EntityKind::MortarTeam
                | EntityKind::RocketLauncher => stop_tiles,
                _ => MARCH_CLOSE_CONTACT_TILES,
            };
            let distance2 = geometry::dist2(center.0, center.1, enemy.x, enemy.y);
            (distance2 <= geometry::squared(reach * tile_size)).then_some((
                enemy.id,
                outbound_wave_target_priority(enemy.kind),
                distance2,
            ))
        })
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

fn visible_anti_armor_target_within_tiles(
    observation: &AiObservation,
    unit_ids: &[u32],
    radius_tiles: f32,
) -> Option<u32> {
    let center = group_center(observation, unit_ids)?;
    let max_distance = radius_tiles * observation.map.tile_size as f32;
    let max_distance2 = max_distance * max_distance;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| {
            matches!(
                enemy.kind,
                EntityKind::Tank | EntityKind::AntiTankGun | EntityKind::Panzerfaust
            )
        })
        .map(|enemy| {
            (
                enemy.id,
                outbound_wave_target_priority(enemy.kind),
                geometry::dist2(center.0, center.1, enemy.x, enemy.y),
            )
        })
        .filter(|(_, _, distance2)| *distance2 <= max_distance2)
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

fn visible_strategic_building_target_within_tiles(
    observation: &AiObservation,
    unit_ids: &[u32],
    radius_tiles: f32,
) -> Option<u32> {
    let center = group_center(observation, unit_ids)?;
    let max_distance = radius_tiles * observation.map.tile_size as f32;
    let max_distance2 = max_distance * max_distance;
    observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind.is_building())
        .map(|enemy| {
            let priority = match enemy.kind {
                EntityKind::ResourceDepot => 0,
                EntityKind::Factory | EntityKind::Steelworks => 1,
                EntityKind::EngineeringComplex | EntityKind::TrainingCentre => 2,
                _ => 3,
            };
            (
                enemy.id,
                priority,
                geometry::dist2(center.0, center.1, enemy.x, enemy.y),
            )
        })
        .filter(|(_, _, distance2)| *distance2 <= max_distance2)
        .min_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.total_cmp(&right.2))
                .then_with(|| left.0.cmp(&right.0))
        })
        .map(|(id, _, _)| id)
}

fn outbound_wave_target_priority(kind: EntityKind) -> u8 {
    match kind {
        EntityKind::Tank | EntityKind::AntiTankGun | EntityKind::Panzerfaust => 0,
        EntityKind::Artillery | EntityKind::MortarTeam => 1,
        EntityKind::MachineGunner | EntityKind::Rifleman | EntityKind::ScoutCar => 2,
        _ => 3,
    }
}

fn group_center(observation: &AiObservation, unit_ids: &[u32]) -> Option<(f32, f32)> {
    let (sum_x, sum_y, count) = observation
        .owned
        .iter()
        .filter(|entity| unit_ids.contains(&entity.id))
        .fold((0.0, 0.0, 0usize), |(sum_x, sum_y, count), entity| {
            (sum_x + entity.x, sum_y + entity.y, count + 1)
        });
    (count > 0).then_some((sum_x / count as f32, sum_y / count as f32))
}

#[cfg(test)]
mod tests;

/// How many of `ready` Tanks the push takes: all but `keep_home`, or none while that would be
/// fewer than `minimum`. Jeff used to push with exactly two or three Tanks while twenty sat at
/// home, and then with most of them while its main was left with one.
fn push_tank_count(ready: usize, minimum: usize, keep_home: usize) -> Option<usize> {
    let size = ready.saturating_sub(keep_home);
    (size >= minimum).then_some(size)
}

/// Ready Tanks the push leaves at home: the home reserve, less the home Tank already kept there.
fn push_keep_home(observation: &AiObservation, memory: &AiDecisionMemory) -> usize {
    let home_tank_alive = memory.home_defensive_tank.is_some_and(|id| {
        observation
            .owned
            .iter()
            .any(|unit| unit.id == id && unit.hp > 0)
    });
    memory
        .home_tank_reserve()
        .saturating_sub(usize::from(home_tank_alive))
}

/// Enemy Tanks this close to any push Tank count toward the push being outnumbered.
const PUSH_OUTNUMBERED_RADIUS_TILES: f32 = 16.0;

/// Whether more enemy Tanks than the push has are in sight around it.
fn push_outnumbered(observation: &AiObservation, tanks: &[u32]) -> bool {
    let radius2 = (PUSH_OUTNUMBERED_RADIUS_TILES * observation.map.tile_size as f32).powi(2);
    let positions: Vec<(f32, f32)> = observation
        .owned
        .iter()
        .filter(|unit| tanks.contains(&unit.id) && unit.hp > 0)
        .map(|unit| (unit.x, unit.y))
        .collect();
    let enemy_tanks = observation
        .visible_enemies
        .iter()
        .filter(|enemy| enemy.kind == EntityKind::Tank && enemy.hp > 0)
        .filter(|enemy| {
            positions
                .iter()
                .any(|tank| dist2(tank.0, tank.1, enemy.x, enemy.y) <= radius2)
        })
        .count();
    enemy_tanks > positions.len()
}
