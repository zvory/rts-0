use std::collections::BTreeSet;

use crate::ai_core::observation::{AiBuildIntent, AiObservation};
use crate::ai_shared;
use crate::config;
use rts_rules;
use rts_sim::game::entity::EntityKind;
use rts_sim::protocol::{kinds, EntityView, MapInfo, Snapshot, StartPayload};

pub(super) fn kind_of(e: &EntityView) -> Option<EntityKind> {
    e.kind.parse().ok()
}

/// Convenience: check whether an `EntityView` has a given internal kind.
pub(super) fn is_kind(e: &EntityView, kind: EntityKind) -> bool {
    e.kind == rts_sim::protocol::kind_to_wire(kind)
}

#[derive(Clone, Copy)]
pub(crate) struct PlayerView<'a> {
    pub(crate) player_id: u32,
    pub(crate) tick: u32,
    pub(crate) start: &'a StartPayload,
    pub(crate) snapshot: &'a Snapshot,
    pub(crate) alive_player_ids: &'a [u32],
}

impl PlayerView<'_> {
    pub(super) fn observation(
        self,
        pending_builds: impl IntoIterator<Item = AiBuildIntent>,
    ) -> Option<AiObservation> {
        AiObservation::from_snapshot_with_alive(
            self.start,
            self.snapshot,
            self.player_id,
            pending_builds,
            Some(self.alive_player_ids),
        )
    }
}

pub(super) fn is_complete(entity: &EntityView) -> bool {
    entity.build_progress.is_none()
}

fn own_start_tile(start: &StartPayload, player_id: u32) -> Option<(u32, u32)> {
    start
        .players
        .iter()
        .find(|p| p.id == player_id)
        .map(|p| (p.start_tile_x, p.start_tile_y))
}

pub(super) fn player_start_world(start: &StartPayload, player_id: u32) -> Option<(f32, f32)> {
    let (tile_x, tile_y) = own_start_tile(start, player_id)?;
    let ts = start.map.tile_size as f32;
    Some((tile_x as f32 * ts + ts * 0.5, tile_y as f32 * ts + ts * 0.5))
}

pub(crate) fn occupied_tiles_from_snapshot(
    map: &MapInfo,
    snapshot: &Snapshot,
) -> BTreeSet<(u32, u32)> {
    let mut occupied = BTreeSet::new();
    for resource in &map.resources {
        if matches!(resource.kind.as_str(), kinds::STEEL | kinds::OIL) {
            occupied.insert(tile_of(map, resource.x, resource.y));
        }
    }
    for e in &snapshot.entities {
        if e.owner != 0 && kind_of(e).map(|k| k.is_building()).unwrap_or(false) {
            for (tx, ty) in building_footprint_tiles(map, e) {
                let Some(kind) = kind_of(e) else {
                    continue;
                };
                let clearance = ai_shared::building_clearance_tiles(kind);
                for dy in -clearance..=clearance {
                    for dx in -clearance..=clearance {
                        let nx = tx as i32 + dx;
                        let ny = ty as i32 + dy;
                        if nx >= 0 && ny >= 0 && (nx as u32) < map.width && (ny as u32) < map.height
                        {
                            occupied.insert((nx as u32, ny as u32));
                        }
                    }
                }
            }
        } else if e.owner == 0 && (is_kind(e, EntityKind::Steel) || is_kind(e, EntityKind::Oil)) {
            occupied.insert(tile_of(map, e.x, e.y));
        }
    }
    occupied
}

pub(crate) fn footprint_placeable_from_snapshot(
    map: &MapInfo,
    snapshot: &Snapshot,
    building: EntityKind,
    tile_x: u32,
    tile_y: u32,
    occupied: &BTreeSet<(u32, u32)>,
) -> bool {
    let Some(stats) = config::building_stats(building) else {
        return false;
    };
    for e in &snapshot.entities {
        let Some(existing_kind) = kind_of(e) else {
            continue;
        };
        if e.owner == 0 || !existing_kind.is_building() {
            continue;
        }
        let existing_tile = tile_of(map, e.x, e.y);
        let existing_tile_x = existing_tile.0.saturating_sub(
            config::building_stats(existing_kind)
                .map(|building| building.foot_w / 2)
                .unwrap_or(0),
        );
        let existing_tile_y = existing_tile.1.saturating_sub(
            config::building_stats(existing_kind)
                .map(|building| building.foot_h / 2)
                .unwrap_or(0),
        );
        if !ai_shared::footprints_respect_clearance(
            building,
            tile_x,
            tile_y,
            existing_kind,
            existing_tile_x,
            existing_tile_y,
        ) {
            return false;
        }
    }
    for dy in 0..stats.foot_h {
        for dx in 0..stats.foot_w {
            let Some(tx) = tile_x.checked_add(dx) else {
                return false;
            };
            let Some(ty) = tile_y.checked_add(dy) else {
                return false;
            };
            if tx >= map.width || ty >= map.height {
                return false;
            }
            let idx = (ty * map.width + tx) as usize;
            if !map
                .terrain
                .get(idx)
                .copied()
                .is_some_and(rts_rules::terrain::is_passable_map_code)
            {
                return false;
            }
            if occupied.contains(&(tx, ty)) {
                return false;
            }
        }
    }
    if !rts_rules::economy::trainable_units(building).is_empty() {
        let spawn_x = tile_x + stats.foot_w / 2;
        let Some(spawn_y) = tile_y.checked_add(stats.foot_h) else {
            return false;
        };
        if spawn_x >= map.width || spawn_y >= map.height {
            return false;
        }
        let spawn_idx = (spawn_y * map.width + spawn_x) as usize;
        if !map
            .terrain
            .get(spawn_idx)
            .copied()
            .is_some_and(rts_rules::terrain::is_passable_map_code)
        {
            return false;
        }
        if occupied.contains(&(spawn_x, spawn_y)) {
            return false;
        }
    }
    true
}

pub(super) fn building_footprint_tiles(map: &MapInfo, entity: &EntityView) -> Vec<(u32, u32)> {
    let kind = match kind_of(entity) {
        Some(k) => k,
        None => return Vec::new(),
    };
    let Some(stats) = config::building_stats(kind) else {
        return Vec::new();
    };
    let (cx, cy) = tile_of(map, entity.x, entity.y);
    let ox = stats.foot_w as i32 / 2;
    let oy = stats.foot_h as i32 / 2;
    let mut out = Vec::new();
    for dy in 0..stats.foot_h as i32 {
        for dx in 0..stats.foot_w as i32 {
            let tx = cx as i32 + dx - ox;
            let ty = cy as i32 + dy - oy;
            if tx >= 0 && ty >= 0 {
                out.push((tx as u32, ty as u32));
            }
        }
    }
    out
}

fn tile_of(map: &MapInfo, x: f32, y: f32) -> (u32, u32) {
    let ts = map.tile_size as f32;
    let tx = (x / ts).floor().max(0.0) as u32;
    let ty = (y / ts).floor().max(0.0) as u32;
    (tx.min(map.width - 1), ty.min(map.height - 1))
}
