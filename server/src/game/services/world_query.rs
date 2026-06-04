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
use crate::game::services::spatial::SpatialIndex;
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

/// Whether a resource node is mineable by `player` because a completed Industrial Center is close
/// enough to receive attached-mining income from that node.
pub(crate) fn resource_has_completed_mining_ic(
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
    nearest_completed_mining_ic(entities, player, resource.pos_x, resource.pos_y)
        .map(|(_, dist2)| {
            let range_px = config::MINING_IC_RANGE_TILES * config::TILE_SIZE as f32;
            dist2 <= range_px * range_px + 0.01
        })
        .unwrap_or(false)
}

fn nearest_completed_mining_ic(
    entities: &EntityStore,
    player: u32,
    x: f32,
    y: f32,
) -> Option<(u32, f32)> {
    completed_buildings(entities, player)
        .filter(|e| e.kind == EntityKind::IndustrialCenter && e.hp > 0)
        .map(|e| {
            let dx = e.pos_x - x;
            let dy = e.pos_y - y;
            (e.id, dx * dx + dy * dy)
        })
        .min_by(|a, b| a.1.total_cmp(&b.1).then_with(|| a.0.cmp(&b.0)))
}

/// Town halls (Industrial Centers) owned by `player`, in any construction state.
/// Reserved for the AI GG/leave predicate (Phase 6.4) and future faction-aware queries.
#[allow(dead_code)]
pub(crate) fn town_halls(
    entities: &EntityStore,
    player: u32,
) -> impl Iterator<Item = &Entity> + '_ {
    owned_buildings(entities, player).filter(|e| e.kind == EntityKind::IndustrialCenter)
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
    attacker_owner: u32,
    attacker_id: u32,
) -> bool {
    candidate.id != attacker_id
        && candidate.owner != attacker_owner
        && candidate.owner != NEUTRAL
        && candidate.is_targetable()
        && candidate.hp > 0
}

/// Nearest hostile entity (`is_enemy_targetable`) to `(px, py)` within `radius_px`.
pub(crate) fn nearest_enemy_in_range(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    radius_px: f32,
) -> Option<u32> {
    nearest_matching_enemy_in_range(entities, spatial, self_id, owner, px, py, radius_px, |_| {
        true
    })
}

/// Nearest hostile Tank to `(px, py)` within `radius_px`, or `None` if no tank is in range.
pub(crate) fn nearest_tank_in_range(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    radius_px: f32,
) -> Option<u32> {
    nearest_matching_enemy_in_range(entities, spatial, self_id, owner, px, py, radius_px, |e| {
        e.kind == EntityKind::Tank
    })
}

#[allow(clippy::too_many_arguments)]
fn nearest_matching_enemy_in_range(
    entities: &EntityStore,
    spatial: &SpatialIndex,
    self_id: u32,
    owner: u32,
    px: f32,
    py: f32,
    radius_px: f32,
    matches_kind: impl Fn(&Entity) -> bool,
) -> Option<u32> {
    let mut best: Option<(u32, f32)> = None;
    for id in spatial.ids_in_circle_bbox(px, py, radius_px) {
        let Some(entity) = entities.get(id) else {
            continue;
        };
        if !is_enemy_targetable(entity, owner, self_id) || !matches_kind(entity) {
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
        if d2 <= r2 && best.map(|(_, best_d2)| d2 < best_d2).unwrap_or(true) {
            best = Some((id, d2));
        }
    }
    best.map(|(id, _)| id)
}

/// Whichever worker currently holds `node`'s single harvest slot, if any. A reservation is
/// only honored when the worker is alive, still gathering this exact node, and in the
/// `Harvesting` phase — stale ids are ignored so commands can race for a freed slot without
/// being blocked by a dead/cancelled holder.
pub(crate) fn node_holder(entities: &EntityStore, node: u32) -> Option<u32> {
    entities.node_slot_holder(node)
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

    fn store_with_two_players() -> EntityStore {
        let mut s = EntityStore::default();
        // P1: industrial center (complete), barracks (under construction), worker.
        s.spawn_building(1, EntityKind::IndustrialCenter, 100.0, 100.0, true)
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
        assert_eq!(completed_buildings(&s, 1).count(), 1);
        assert_eq!(owned_units(&s, 2).count(), 1);
        assert_eq!(owned_buildings(&s, 2).count(), 0);
        assert!(has_town_hall(&s, 1));
        assert!(!has_town_hall(&s, 2));
        // Completed-kinds excludes the in-progress barracks.
        let ck = completed_building_kinds(&s, 1);
        assert!(ck.contains(&EntityKind::IndustrialCenter));
        assert!(!ck.contains(&EntityKind::Barracks));
        // All-kinds includes it.
        let ak = owned_building_kinds(&s, 1);
        assert!(ak.contains(&EntityKind::Barracks));
    }

    #[test]
    fn enemy_predicate_rejects_self_friendly_neutral() {
        let s = store_with_two_players();
        let p2_rifleman = s.iter().find(|e| e.owner == 2).unwrap();
        let p1_worker = s.iter().find(|e| e.owner == 1 && e.is_unit()).unwrap();
        let p1_ic = s.iter().find(|e| e.owner == 1 && e.is_building()).unwrap();
        // P1 attacker can target the P2 rifleman.
        assert!(is_enemy_targetable(p2_rifleman, 1, p1_worker.id));
        // ... but not their own worker (self) or their own building.
        assert!(!is_enemy_targetable(p1_worker, 1, p1_worker.id));
        assert!(!is_enemy_targetable(p1_ic, 1, p1_worker.id));
    }

    #[test]
    fn resource_mining_requires_completed_ic_in_range() {
        let ts = config::TILE_SIZE as f32;
        let mut s = EntityStore::default();
        s.spawn_building(1, EntityKind::IndustrialCenter, 100.0, 100.0, true)
            .unwrap();
        let near = s
            .spawn_node(
                EntityKind::Steel,
                100.0 + config::MINING_IC_RANGE_TILES * ts,
                100.0,
            )
            .unwrap();
        let far = s
            .spawn_node(
                EntityKind::Steel,
                100.0 + (config::MINING_IC_RANGE_TILES + 0.25) * ts,
                100.0,
            )
            .unwrap();
        let unfinished_ic = s
            .spawn_building(
                2,
                EntityKind::IndustrialCenter,
                100.0 + config::MINING_IC_RANGE_TILES * ts,
                300.0,
                false,
            )
            .unwrap();
        let unfinished_near = s.spawn_node(EntityKind::Steel, 100.0, 300.0).unwrap();

        assert!(resource_has_completed_mining_ic(&s, 1, near));
        assert!(!resource_has_completed_mining_ic(&s, 1, far));
        assert!(!resource_has_completed_mining_ic(&s, 2, unfinished_near));
        s.remove(unfinished_ic);
        assert!(!resource_has_completed_mining_ic(&s, 2, unfinished_near));
    }
}
