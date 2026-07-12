//! Central world query helpers (Phase 2.2).
//!
//! Canonical iterators and predicates over [`EntityStore`] so that simulation systems, the AI,
//! and command handling stop reinventing the same scans / filters. Keeping these in one place
//! lets us evolve target acquisition, tech checks, and ownership logic without sweeping
//! find-and-replace through every service.
//!
//! Helpers fall into three buckets:
//! - **Ownership scans**: owned units, owned buildings, completed buildings, town halls.
//! - **Targeting / proximity**: `is_enemy_targetable`, `nearest_enemy_in_range`.
//! - **Reservation**: `node_holder` (who currently owns a node's harvest slot).
//!
//! Building placement and spawn search remain in [`super::standability`] and
//! [`super::move_coordinator::MoveCoordinator::find_spawn_point`]; this module points to them
//! through its docs so the placement / spawn surface is discoverable from one place.

use crate::config;
use crate::game::entity::{Entity, EntityKind, EntityStore, NEUTRAL};
use crate::game::fog::Fog;
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::rules::combat as combat_rules;
use crate::rules::projection;
use crate::rules::terrain::{self, TerrainKind};

// --- Ownership scans --------------------------------------------------------

/// All living units owned by `player`. Stable order (by id).
pub(crate) fn owned_units(
    entities: &EntityStore,
    player: u32,
) -> impl Iterator<Item = &Entity> + '_ {
    entities
        .iter()
        .filter(move |e| e.owner == player && e.is_unit())
}

/// All buildings owned by `player`, including those still under construction.
pub(crate) fn owned_buildings(
    entities: &EntityStore,
    player: u32,
) -> impl Iterator<Item = &Entity> + '_ {
    entities
        .iter()
        .filter(move |e| e.owner == player && e.is_building())
}

/// Finished buildings owned by `player` (excludes scaffolding under construction).
pub(crate) fn completed_buildings(
    entities: &EntityStore,
    player: u32,
) -> impl Iterator<Item = &Entity> + '_ {
    owned_buildings(entities, player).filter(|e| !e.under_construction())
}

/// Owned buildings that count as keeping a player alive. Field obstacles use generic building
/// mechanics for targeting, snapshots, and cleanup, but do not satisfy elimination rules.
pub(crate) fn owned_survival_buildings(
    entities: &EntityStore,
    player: u32,
) -> impl Iterator<Item = &Entity> + '_ {
    owned_buildings(entities, player).filter(|e| survival_building_kind(e.kind))
}

fn survival_building_kind(kind: EntityKind) -> bool {
    kind.is_building() && !matches!(kind, EntityKind::TankTrap | EntityKind::PumpJack)
}

/// Kinds of all owned buildings (any state).
#[cfg(test)]
pub(crate) fn owned_building_kinds(entities: &EntityStore, player: u32) -> Vec<EntityKind> {
    owned_buildings(entities, player).map(|e| e.kind).collect()
}

/// Kinds of finished buildings only — required for training requirement checks.
pub(crate) fn completed_building_kinds(entities: &EntityStore, player: u32) -> Vec<EntityKind> {
    completed_buildings(entities, player)
        .map(|e| e.kind)
        .collect()
}

/// Whether a resource node is mineable by `player` because a completed home-base mining anchor
/// (City Centre or Zamok) is close enough to receive attached-mining income from that node.
pub(crate) fn resource_has_completed_mining_cc(
    entities: &EntityStore,
    player: u32,
    node: u32,
) -> bool {
    let Some(resource) = entities.get(node) else {
        return false;
    };
    if !resource.is_node() || resource.remaining().unwrap_or(0) == 0 {
        return false;
    }
    nearest_completed_mining_anchor(entities, player, resource.pos_x, resource.pos_y)
        .map(|(_, dist2)| {
            let range_px = config::MINING_CC_RANGE_TILES * config::TILE_SIZE as f32;
            dist2 <= range_px * range_px + 0.01
        })
        .unwrap_or(false)
}

fn nearest_completed_mining_anchor(
    entities: &EntityStore,
    player: u32,
    x: f32,
    y: f32,
) -> Option<(u32, f32)> {
    completed_buildings(entities, player)
        .filter(|e| is_home_base_mining_anchor(e.kind) && e.hp > 0)
        .map(|e| {
            let dx = e.pos_x - x;
            let dy = e.pos_y - y;
            (e.id, dx * dx + dy * dy)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
}

fn is_home_base_mining_anchor(kind: EntityKind) -> bool {
    matches!(kind, EntityKind::CityCentre | EntityKind::Zamok)
}

/// Town halls (City Centres) owned by `player`, in any construction state.
/// Reserved for the AI GG/leave predicate (Phase 6.4) and future faction-aware queries.
#[allow(dead_code)]
pub(crate) fn town_halls(
    entities: &EntityStore,
    player: u32,
) -> impl Iterator<Item = &Entity> + '_ {
    owned_buildings(entities, player).filter(|e| e.kind == EntityKind::CityCentre)
}

/// Whether `player` still has at least one town hall (any state). Centralized so the
/// AI GG/leave check (Phase 6.4) and any future "defeated" predicate use one definition.
#[allow(dead_code)]
pub(crate) fn has_town_hall(entities: &EntityStore, player: u32) -> bool {
    town_halls(entities, player).next().is_some()
}

// --- Targeting / proximity --------------------------------------------------

/// Whether `candidate` is a legal hostile target for an attacker owned by `attacker_owner`.
/// Filters out self, neutrals, friendlies, dead, and non-targetable kinds.
pub(crate) fn is_enemy_targetable(
    candidate: &Entity,
    teams: &TeamRelations,
    attacker_owner: u32,
    attacker_id: u32,
) -> bool {
    candidate.id != attacker_id
        && candidate.owner != NEUTRAL
        && teams.is_enemy_owner(attacker_owner, candidate.owner)
        && candidate.is_targetable()
        && candidate.hp > 0
}

/// Whether `candidate` is a legal explicit attack target for an attacker owned by
/// `attacker_owner`. Explicit player commands may target the attacker's own entities, but not
/// allied teammates; automatic acquisition remains enemy-only through `is_enemy_targetable`.
pub(crate) fn is_explicit_attack_targetable(
    candidate: &Entity,
    teams: &TeamRelations,
    attacker_owner: u32,
    attacker_id: u32,
) -> bool {
    candidate.id != attacker_id
        && candidate.owner != NEUTRAL
        && (candidate.owner == attacker_owner
            || teams.is_enemy_owner(attacker_owner, candidate.owner))
        && candidate.is_targetable()
        && candidate.hp > 0
}

pub(crate) fn unit_explicit_attack_target_valid(
    entities: &EntityStore,
    teams: &TeamRelations,
    fog: &Fog,
    smokes: Option<&SmokeCloudStore>,
    attacker_owner: u32,
    attacker_id: u32,
    target_id: u32,
) -> bool {
    let Some(attacker) = entities.get(attacker_id) else {
        return false;
    };
    if attacker.owner != attacker_owner || attacker.hp == 0 || !attacker.is_unit() {
        return false;
    }
    matches!(entities.get(target_id),
        Some(target) if is_explicit_attack_targetable(target, teams, attacker_owner, attacker_id)
            && projection::team_visible_world(
                attacker_owner,
                target.pos_x,
                target.pos_y,
                fog,
                teams
            )
            && smokes.is_none_or(|smokes| !smokes.point_inside(target.pos_x, target.pos_y))
            && (attacker.kind != EntityKind::Panzerfaust
                || combat_rules::is_panzerfaust_loaded_shot_target(target.kind)))
}

/// Nearest hostile entity (`is_enemy_targetable`) to `(px, py)` within `radius_px`.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn nearest_enemy_in_range(
    entities: &EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    radius_px: f32,
) -> Option<u32> {
    nearest_matching_enemy_in_range(
        entities,
        teams,
        spatial,
        self_id,
        owner,
        px,
        py,
        radius_px,
        |_| true,
        |_| true,
    )
}

/// Nearest hostile Tank to `(px, py)` within `radius_px`, or `None` if no tank is in range.
#[allow(dead_code)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn nearest_tank_in_range(
    entities: &EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    radius_px: f32,
) -> Option<u32> {
    nearest_matching_enemy_in_range(
        entities,
        teams,
        spatial,
        self_id,
        owner,
        px,
        py,
        radius_px,
        |e| e.kind == EntityKind::Tank,
        |_| true,
    )
}

#[allow(clippy::too_many_arguments)]
fn nearest_matching_enemy_in_range(
    entities: &EntityStore,
    teams: &TeamRelations,
    spatial: &SpatialIndex,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    radius_px: f32,
    matches_kind: impl Fn(&Entity) -> bool,
    target_filter: impl Fn(&Entity) -> bool,
) -> Option<u32> {
    let mut best: Option<(u32, f32)> = None;
    for id in spatial.ids_in_circle_bbox(px, py, radius_px) {
        let Some(entity) = entities.get(id) else {
            continue;
        };
        if !is_enemy_targetable(entity, teams, owner, self_id)
            || !matches_kind(entity)
            || !target_filter(entity)
        {
            continue;
        }
        let concealment = terrain::concealment_modifier(entity.kind, TerrainKind::Open).max(0.0);
        let effective_radius = radius_px * concealment;
        if !effective_radius.is_finite() {
            continue;
        }
        let dx = entity.pos_x - px;
        let dy = entity.pos_y - py;
        let d2 = dx * dx + dy * dy;
        let r2 = effective_radius * effective_radius;
        if d2 <= r2
            && best
                .map(|(best_id, best_d2)| {
                    d2.total_cmp(&best_d2)
                        .then_with(|| id.cmp(&best_id))
                        .is_lt()
                })
                .unwrap_or(true)
        {
            best = Some((id, d2));
        }
    }
    best.map(|(id, _)| id)
}

// --- Cheap predicates re-exported for convenience ---------------------------

/// Whether `player` owns a *unit* with this id (buildings and nodes excluded).
pub(crate) fn owns_unit(entities: &EntityStore, player: u32, id: u32) -> bool {
    matches!(entities.get(id), Some(e) if e.owner == player && e.is_unit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::entity::EntityStore;

    fn ffa_teams() -> TeamRelations {
        TeamRelations::from_player_teams([(1, 1), (2, 2)])
    }

    fn store_with_two_players() -> EntityStore {
        let mut s = EntityStore::default();
        // P1: city centre (complete), barracks (under construction), worker.
        s.spawn_building(1, EntityKind::CityCentre, 100.0, 100.0, true)
            .unwrap();
        s.spawn_building(1, EntityKind::Barracks, 200.0, 100.0, false)
            .unwrap();
        s.spawn_unit(1, EntityKind::Worker, 100.0, 200.0).unwrap();
        // P2: rifleman.
        s.spawn_unit(2, EntityKind::Rifleman, 400.0, 400.0).unwrap();
        s
    }

    #[test]
    fn ownership_scans_partition_correctly() {
        let s = store_with_two_players();
        assert_eq!(owned_units(&s, 1).count(), 1);
        assert_eq!(owned_buildings(&s, 1).count(), 2);
        assert_eq!(owned_survival_buildings(&s, 1).count(), 2);
        assert_eq!(completed_buildings(&s, 1).count(), 1);
        assert_eq!(owned_units(&s, 2).count(), 1);
        assert_eq!(owned_buildings(&s, 2).count(), 0);
        assert!(has_town_hall(&s, 1));
        assert!(!has_town_hall(&s, 2));
        // Completed-kinds excludes the in-progress barracks.
        let ck = completed_building_kinds(&s, 1);
        assert!(ck.contains(&EntityKind::CityCentre));
        assert!(!ck.contains(&EntityKind::Barracks));
        // All-kinds includes it.
        let ak = owned_building_kinds(&s, 1);
        assert!(ak.contains(&EntityKind::Barracks));
    }

    #[test]
    fn survival_buildings_exclude_tank_traps_without_hiding_generic_buildings() {
        let mut s = EntityStore::default();
        s.spawn_building(1, EntityKind::TankTrap, 100.0, 100.0, true)
            .unwrap();
        s.spawn_building(1, EntityKind::PumpJack, 132.0, 100.0, true)
            .unwrap();
        s.spawn_building(1, EntityKind::Depot, 164.0, 100.0, true)
            .unwrap();

        assert_eq!(owned_buildings(&s, 1).count(), 3);
        assert_eq!(
            owned_survival_buildings(&s, 1)
                .map(|entity| entity.kind)
                .collect::<Vec<_>>(),
            vec![EntityKind::Depot]
        );
    }

    #[test]
    fn enemy_predicate_rejects_self_friendly_neutral() {
        let s = store_with_two_players();
        let p2_rifleman = s.iter().find(|e| e.owner == 2).unwrap();
        let p1_worker = s.iter().find(|e| e.owner == 1 && e.is_unit()).unwrap();
        let p1_cc = s.iter().find(|e| e.owner == 1 && e.is_building()).unwrap();
        let teams = ffa_teams();
        // P1 attacker can target the P2 rifleman.
        assert!(is_enemy_targetable(p2_rifleman, &teams, 1, p1_worker.id));
        // ... but not their own worker (self) or their own building.
        assert!(!is_enemy_targetable(p1_worker, &teams, 1, p1_worker.id));
        assert!(!is_enemy_targetable(p1_cc, &teams, 1, p1_worker.id));
    }

    #[test]
    fn enemy_predicate_rejects_allies() {
        let s = store_with_two_players();
        let p2_rifleman = s.iter().find(|e| e.owner == 2).unwrap();
        let p1_worker = s.iter().find(|e| e.owner == 1 && e.is_unit()).unwrap();
        let teams = TeamRelations::from_player_teams([(1, 7), (2, 7)]);

        assert!(!is_enemy_targetable(p2_rifleman, &teams, 1, p1_worker.id));
    }

    #[test]
    fn nearest_enemy_breaks_equal_distance_ties_by_id() {
        let mut s = EntityStore::default();
        let attacker = s.spawn_unit(1, EntityKind::Rifleman, 100.0, 100.0).unwrap();
        let lower_id = s.spawn_unit(2, EntityKind::Rifleman, 120.0, 100.0).unwrap();
        let higher_id = s.spawn_unit(2, EntityKind::Rifleman, 80.0, 100.0).unwrap();
        assert!(lower_id < higher_id);
        let spatial = SpatialIndex::build(&s, 10);
        let teams = ffa_teams();

        assert_eq!(
            nearest_enemy_in_range(&s, &teams, &spatial, attacker, 1, 100.0, 100.0, 50.0),
            Some(lower_id)
        );
    }

    #[test]
    fn resource_mining_requires_completed_home_base_anchor_in_range() {
        let ts = config::TILE_SIZE as f32;
        let mut s = EntityStore::default();
        s.spawn_building(1, EntityKind::CityCentre, 100.0, 100.0, true)
            .unwrap();
        s.spawn_building(2, EntityKind::Zamok, 500.0, 100.0, true)
            .unwrap();
        let near = s
            .spawn_node(
                EntityKind::Steel,
                100.0 + config::MINING_CC_RANGE_TILES * ts,
                100.0,
            )
            .unwrap();
        let forgiving = s
            .spawn_node(
                EntityKind::Steel,
                100.0 + (config::CC_RESOURCE_MAX_DIST_TILES + 1.5) * ts,
                100.0,
            )
            .unwrap();
        let far = s
            .spawn_node(
                EntityKind::Steel,
                100.0 + (config::MINING_CC_RANGE_TILES + 0.25) * ts,
                100.0,
            )
            .unwrap();
        let unfinished_cc = s
            .spawn_building(
                2,
                EntityKind::CityCentre,
                100.0 + config::MINING_CC_RANGE_TILES * ts,
                300.0,
                false,
            )
            .unwrap();
        let unfinished_near = s.spawn_node(EntityKind::Steel, 100.0, 300.0).unwrap();
        let zamok_near = s
            .spawn_node(
                EntityKind::Oil,
                500.0 + config::MINING_CC_RANGE_TILES * ts,
                100.0,
            )
            .unwrap();

        assert!(resource_has_completed_mining_cc(&s, 1, near));
        assert!(resource_has_completed_mining_cc(&s, 1, forgiving));
        assert!(resource_has_completed_mining_cc(&s, 2, zamok_near));
        assert!(!resource_has_completed_mining_cc(&s, 1, far));
        assert!(!resource_has_completed_mining_cc(&s, 2, unfinished_near));
        s.remove(unfinished_cc);
        assert!(!resource_has_completed_mining_cc(&s, 2, unfinished_near));
    }
}
