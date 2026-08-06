use crate::config;
use crate::game::artillery::ArtilleryShellStore;
use crate::game::entity::{Entity, EntityKind, EntityStore, MovePhase, Order, WeaponSetup};
use crate::game::firing_reveal::FiringRevealSource;
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::artillery_fire::{artillery_min_fire_radius_tiles, try_fire_artillery};
use crate::game::services::move_coordinator::MoveCoordinator;
use crate::game::services::spatial::SpatialIndex;
use crate::game::services::world_query;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::game::upgrade::UpgradeKind;
use crate::game::PlayerState;
use crate::protocol::Event;
use std::collections::HashMap;

const NO_TARGET_GRACE_TICKS: u16 = config::TICK_HZ as u16;

#[derive(Clone, Copy, Debug)]
struct Candidate {
    id: u32,
    x: f32,
    y: f32,
    target_priority: u8,
    value_in_dispersion: u64,
    distance_sq: f32,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run(
    map: &Map,
    entities: &mut EntityStore,
    players: &mut [PlayerState],
    spatial: &SpatialIndex,
    coordinator: &mut MoveCoordinator<'_>,
    artillery_shells: &mut ArtilleryShellStore,
    firing_reveals: &mut Vec<FiringRevealSource>,
    events: &mut HashMap<u32, Vec<Event>>,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    tick: u32,
) {
    let teams = TeamRelations::from_player_teams(players.iter().map(|p| (p.id, p.team_id)));
    let artillery_ids: Vec<u32> = entities
        .iter()
        .filter(|entity| {
            entity.kind == EntityKind::Artillery
                && entity.hp > 0
                && matches!(entity.order(), Order::AttackMove(_))
        })
        .map(|entity| entity.id)
        .collect();

    for id in artillery_ids {
        let Some((owner, position, setup, move_goal)) = entities.get(id).map(|entity| {
            (
                entity.owner,
                (entity.pos_x, entity.pos_y),
                entity.weapon_setup(),
                entity.move_intent(),
            )
        }) else {
            continue;
        };
        let has_fire_control = players
            .iter()
            .any(|player| player.id == owner && player.has_upgrade(UpgradeKind::BallisticTables));
        let dispersion_tiles = artillery_min_fire_radius_tiles(has_fire_control);
        let candidates = candidates(
            map,
            entities,
            spatial,
            &teams,
            fog,
            smokes,
            id,
            owner,
            position,
            dispersion_tiles,
        );
        let target = choose_for_setup(entities.get(id), setup, &candidates);

        let Some(target) = target else {
            handle_no_target(entities, coordinator, id, setup, move_goal);
            continue;
        };

        let target_angle = (target.y - position.1).atan2(target.x - position.0);
        let already_inside_field = entities
            .get(id)
            .is_some_and(|entity| target_inside_current_field(entity, target_angle));
        engage_target(entities, id, target, target_angle, already_inside_field);
        if already_inside_field && matches!(setup, WeaponSetup::Deployed) {
            try_fire_artillery(
                entities,
                players,
                &teams,
                fog,
                artillery_shells,
                firing_reveals,
                events,
                owner,
                id,
                target.x,
                target.y,
                tick,
                crate::game::ability::AbilityKind::PointFire,
                dispersion_tiles,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn candidates(
    map: &Map,
    entities: &EntityStore,
    spatial: &SpatialIndex,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: &SmokeCloudStore,
    artillery_id: u32,
    owner: u32,
    origin: (f32, f32),
    dispersion_tiles: f32,
) -> Vec<Candidate> {
    let min_range_px = config::ARTILLERY_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let max_range_px = config::ARTILLERY_MAX_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let visible_enemy = |target: &Entity| {
        (target.is_unit() || target.is_building())
            && world_query::is_enemy_targetable(target, teams, owner, artillery_id)
            && crate::rules::projection::team_visible_world(
                owner,
                target.pos_x,
                target.pos_y,
                fog,
                teams,
            )
            && !crate::rules::projection::entity_hidden_by_concealment_from_team(
                owner, target, map, fog, teams,
            )
            && !smokes.point_inside(target.pos_x, target.pos_y)
    };
    let dispersion_px = dispersion_tiles * config::TILE_SIZE as f32;
    let mut unit_candidates = Vec::new();
    let mut building_candidates = Vec::new();
    for target_id in spatial.ids_in_circle_bbox(origin.0, origin.1, max_range_px) {
        let Some(target) = entities.get(target_id) else {
            continue;
        };
        if !visible_enemy(target) {
            continue;
        }
        let distance_sq = (target.pos_x - origin.0).powi(2) + (target.pos_y - origin.1).powi(2);
        if !distance_sq.is_finite()
            || distance_sq < min_range_px * min_range_px
            || distance_sq > max_range_px * max_range_px
        {
            continue;
        }
        let value_in_dispersion = spatial
            .ids_in_circle_bbox(target.pos_x, target.pos_y, dispersion_px)
            .filter_map(|covered_id| entities.get(covered_id))
            .filter(|covered| {
                visible_enemy(covered)
                    && (covered.pos_x - target.pos_x).powi(2)
                        + (covered.pos_y - target.pos_y).powi(2)
                        <= dispersion_px * dispersion_px
            })
            .map(entity_value)
            .sum();
        let candidate = Candidate {
            id: target.id,
            x: target.pos_x,
            y: target.pos_y,
            target_priority: artillery_target_priority(target),
            value_in_dispersion,
            distance_sq,
        };
        if target.is_unit() {
            unit_candidates.push(candidate);
        } else {
            building_candidates.push(candidate);
        }
    }
    if unit_candidates.is_empty() {
        building_candidates
    } else {
        unit_candidates
    }
}

fn entity_value(entity: &Entity) -> u64 {
    let (steel, oil) = crate::rules::economy::cost(entity.kind);
    1 + u64::from(steel) + u64::from(oil)
}

fn artillery_target_priority(entity: &Entity) -> u8 {
    if crate::rules::combat::is_armored(entity.kind) {
        0
    } else if config::is_entrenchment_eligible_infantry(entity.kind) {
        1
    } else {
        2
    }
}

fn choose_for_setup(
    artillery: Option<&Entity>,
    setup: WeaponSetup,
    candidates: &[Candidate],
) -> Option<Candidate> {
    let artillery = artillery?;
    let inside: Vec<Candidate> = candidates
        .iter()
        .copied()
        .filter(|candidate| {
            target_inside_current_field(
                artillery,
                (candidate.y - artillery.pos_y).atan2(candidate.x - artillery.pos_x),
            )
        })
        .collect();
    match setup {
        WeaponSetup::SettingUp { .. } => best_candidate(&inside),
        WeaponSetup::Deployed if !inside.is_empty() => best_candidate(&inside),
        _ => best_candidate(candidates),
    }
}

fn best_candidate(candidates: &[Candidate]) -> Option<Candidate> {
    candidates.iter().copied().max_by(|left, right| {
        left.target_priority
            .cmp(&right.target_priority)
            .then_with(|| left.value_in_dispersion.cmp(&right.value_in_dispersion))
            .then_with(|| right.distance_sq.total_cmp(&left.distance_sq))
            .then_with(|| right.id.cmp(&left.id))
    })
}

fn target_inside_current_field(artillery: &Entity, target_angle: f32) -> bool {
    let Some(center) = artillery
        .emplacement_facing()
        .or_else(|| artillery.weapon_facing())
        .filter(|facing| facing.is_finite())
    else {
        return false;
    };
    target_angle.is_finite()
        && angle_delta(center, target_angle).abs() <= config::ARTILLERY_FIELD_OF_FIRE_RAD * 0.5
}

fn angle_delta(a: f32, b: f32) -> f32 {
    let mut delta = (a - b).rem_euclid(std::f32::consts::TAU);
    if delta > std::f32::consts::PI {
        delta -= std::f32::consts::TAU;
    }
    delta
}

fn engage_target(
    entities: &mut EntityStore,
    id: u32,
    target: Candidate,
    target_angle: f32,
    already_inside_field: bool,
) {
    let Some(artillery) = entities.get_mut(id) else {
        return;
    };
    artillery.clear_path();
    artillery.set_target_id(Some(target.id));
    artillery.reset_attack_move_no_target_ticks();
    match artillery.weapon_setup() {
        WeaponSetup::Packed => {
            artillery.set_emplacement_facing(Some(target_angle));
            artillery.set_desired_weapon_facing(target_angle);
        }
        WeaponSetup::SettingUp { .. } => {}
        WeaponSetup::Deployed if already_inside_field => {
            artillery.set_desired_weapon_facing(target_angle);
        }
        WeaponSetup::Deployed => {
            artillery.set_pending_redeploy_facing(Some(target_angle));
            artillery.set_weapon_setup(WeaponSetup::TearingDownToRedeploy {
                ticks: config::ARTILLERY_SETUP_TICKS,
            });
        }
        WeaponSetup::TearingDownToRedeploy { .. } => {
            artillery.set_pending_redeploy_facing(Some(target_angle));
        }
        WeaponSetup::TearingDown { ticks } => {
            artillery.set_pending_redeploy_facing(Some(target_angle));
            artillery.set_weapon_setup(WeaponSetup::TearingDownToRedeploy { ticks });
        }
    }
}

fn handle_no_target(
    entities: &mut EntityStore,
    coordinator: &mut MoveCoordinator<'_>,
    id: u32,
    setup: WeaponSetup,
    move_goal: Option<(f32, f32)>,
) {
    let should_resume = entities
        .get_mut(id)
        .map(|artillery| {
            artillery.set_target_id(None);
            match setup {
                WeaponSetup::SettingUp { .. } => {
                    artillery.reset_attack_move_no_target_ticks();
                    false
                }
                WeaponSetup::Deployed => {
                    if artillery.increment_attack_move_no_target_ticks() < NO_TARGET_GRACE_TICKS {
                        return false;
                    }
                    artillery.begin_weapon_teardown_for_movement();
                    true
                }
                WeaponSetup::Packed
                | WeaponSetup::TearingDown { .. }
                | WeaponSetup::TearingDownToRedeploy { .. } => {
                    artillery.reset_attack_move_no_target_ticks();
                    artillery.begin_weapon_teardown_for_movement();
                    true
                }
            }
        })
        .unwrap_or(false);
    if !should_resume {
        return;
    }
    let Some(goal) = move_goal else {
        return;
    };
    let needs_path = entities.get(id).is_some_and(|artillery| {
        artillery.move_phase() != Some(MovePhase::Arrived) && artillery.path_is_empty()
    });
    if needs_path {
        coordinator.request_attack_move_path(entities, id, goal);
    }
}
