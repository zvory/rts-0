//! Tick invariant checks. See `PLAN.md` §1.1.
//!
//! These assertions run in debug builds and tests after every tick. They are intentionally
//! panic-on-failure so broken assumptions surface immediately during development.

use crate::config;
use crate::game::entity::{Entity, EntityKind, Order, NEUTRAL};
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::game::services::geometry::{
    building_rect_for_entity, circle_intersects_rect, segment_intersects_rect,
    unit_body_for_entity, unit_body_overlap, CircleBody, RectBody, UnitBody,
};
use crate::game::services::movement::is_collision_anchored;
use crate::game::services::occupancy::{building_footprint, Occupancy};
use crate::game::services::standability;
use crate::game::Game;
use crate::rules;

/// Maximum residual overlap (world px) tolerated between two non-anchored mobile units after
/// a tick. The iterative resolver converges to within numerical noise on flat ground; this
/// slack also absorbs units pinned against static terrain or building body clearance where the
/// resolver's only separating pushes are statically illegal and have to be skipped. Static
/// unit-vs-building contact has its own much smaller tolerance below.
const OVERLAP_TOLERANCE_PX: f32 = 12.5;
/// Maximum static body penetration tolerated by invariants. Movement/standability still use the
/// stricter geometry predicates; this only avoids failing self-play on exact tangent contact after
/// floating-point rotation.
const STATIC_BODY_OVERLAP_TOLERANCE_PX: f32 = 0.01;

impl Game {
    /// Assert that the current world state satisfies all simulation invariants.
    ///
    /// Called automatically at the end of [`Game::tick`] in debug builds. Tests may also call it
    /// explicitly after manual state mutations.
    pub fn assert_invariants(&self) {
        let world_max = self.state.map.world_size_px();
        let player_ids: Vec<u32> = self.state.players.iter().map(|p| p.id).collect();

        // ------------------------------------------------------------------
        // 1. Entity id / store-key consistency
        // ------------------------------------------------------------------
        for e in self.state.entities.iter() {
            assert!(
                self.state.entities.contains(e.id),
                "invariant: entity {} kind {:?} has id that does not exist in store",
                e.id,
                e.kind
            );
            // Also verify the entity we get back by id is the same record.
            if let Some(by_key) = self.state.entities.get(e.id) {
                assert_eq!(
                    by_key.id, e.id,
                    "invariant: store key {} does not match entity id {}",
                    by_key.id, e.id
                );
            }
        }

        // ------------------------------------------------------------------
        // 2. No NaN, invalid unit bodies, or out-of-world coordinates
        // ------------------------------------------------------------------
        for e in self.state.entities.iter() {
            assert!(
                e.pos_x.is_finite() && e.pos_y.is_finite(),
                "invariant: tick {} entity has non-finite position; {}",
                self.state.tick,
                entity_context(&self.state.map, e)
            );
            assert!(
                e.pos_x >= 0.0 && e.pos_x < world_max && e.pos_y >= 0.0 && e.pos_y < world_max,
                "invariant: tick {} entity position out of world bounds [0, {:.2}); {}",
                self.state.tick,
                world_max,
                entity_context(&self.state.map, e)
            );
            if e.is_unit() {
                let body = unit_body_for_entity(e);
                assert!(
                    body.is_some() || e.kind == EntityKind::ScoutPlane,
                    "invariant: tick {} unit has invalid body; {}",
                    self.state.tick,
                    entity_context(&self.state.map, e)
                );
            }
        }

        // ------------------------------------------------------------------
        // 3. Supply equals living plus queued units
        // ------------------------------------------------------------------
        for ps in &self.state.players {
            let catalog = rules::faction::catalog_for(&ps.faction_id);
            let mut expected_cap = 0u32;
            let mut expected_used = 0u32;
            for e in self.state.entities.iter() {
                if e.owner != ps.id {
                    continue;
                }
                if e.is_building() && !e.under_construction() {
                    if catalog.is_some_and(|catalog| catalog.allows_building(e.kind)) {
                        expected_cap += rules::economy::supply_provided(e.kind);
                    }
                    for item in e.prod_queue() {
                        if catalog.is_some_and(|catalog| catalog.allows_unit(item.unit)) {
                            expected_used += rules::economy::supply_cost(item.unit);
                        }
                    }
                } else if e.is_unit() && catalog.is_some_and(|catalog| catalog.allows_unit(e.kind))
                {
                    expected_used += rules::economy::supply_cost(e.kind);
                }
            }
            expected_cap = expected_cap.min(config::SUPPLY_CAP_MAX);
            assert_eq!(
                ps.supply_cap, expected_cap,
                "invariant: player {} supply_cap {} != expected {}",
                ps.id, ps.supply_cap, expected_cap
            );
            assert_eq!(
                ps.supply_used, expected_used,
                "invariant: player {} supply_used {} != expected {}",
                ps.id, ps.supply_used, expected_used
            );
        }

        // ------------------------------------------------------------------
        // 4. Buildings never overlap
        // ------------------------------------------------------------------
        let mut occupied: Vec<(u32, EntityKind, (u32, u32))> = Vec::new();
        for e in self.state.entities.iter() {
            if !e.is_building() {
                continue;
            }
            let footprint = building_footprint(&self.state.map, e);
            for tile in &footprint {
                let previous = occupied
                    .iter()
                    .find(|(_, _, occupied_tile)| occupied_tile == tile);
                assert!(
                    previous.is_none(),
                    "invariant: tick {} building footprint overlaps another building at tile {:?} {}; building={}; other={}; footprint={:?}",
                    self.state.tick,
                    tile,
                    tile_location_context(&self.state.map, *tile),
                    entity_context(&self.state.map, e),
                    previous
                        .map(|(id, kind, _)| format!("id={} kind={}", id, kind))
                        .unwrap_or_else(|| "unknown".to_string()),
                    footprint
                );
                occupied.push((e.id, e.kind, *tile));
            }
        }

        let building_rects: Vec<_> = self.state.entities
            .iter()
            .filter_map(|e| {
                if e.is_building() {
                    building_rect_for_entity(&self.state.map, e).map(|rect| (e.id, e.kind, rect))
                } else {
                    None
                }
            })
            .collect();
        for node in self.state.entities.iter().filter(|e| e.is_node()) {
            let body = CircleBody {
                x: node.pos_x,
                y: node.pos_y,
                radius: node.radius(),
            };
            for &(building_id, building_kind, rect) in &building_rects {
                assert!(
                    standability::resource_node_building_overlap_allowed(node, building_kind, rect)
                        || !circle_intersects_rect(body, rect),
                    "invariant: tick {} resource node body overlaps building footprint; node={}; building=id={} kind={} {}; collision={}",
                    self.state.tick,
                    entity_context(&self.state.map, node),
                    building_id,
                    building_kind,
                    rect_context(&self.state.map, rect),
                    circle_rect_collision_context(&self.state.map, body, rect)
                );
            }
        }

        // ------------------------------------------------------------------
        // 5. Non-ghost unit bodies never intersect static blockers.
        // ------------------------------------------------------------------
        for e in self.state.entities.iter().filter(|e| e.is_unit()) {
            if is_collision_anchored(e) {
                continue;
            }
            if let Some(body) = unit_body_for_entity(e) {
                for &(building_id, building_kind, rect) in &building_rects {
                    let overlap_depth = unit_body_rect_overlap_depth(body, rect);
                    assert!(
                        overlap_depth <= STATIC_BODY_OVERLAP_TOLERANCE_PX,
                        "invariant: tick {} unit body intersects building footprint; unit={}; building=id={} kind={} {}; collision={}",
                        self.state.tick,
                        entity_context(&self.state.map, e),
                        building_id,
                        building_kind,
                        rect_context(&self.state.map, rect),
                        unit_body_rect_collision_context(&self.state.map, body, rect)
                    );
                }
            }
        }

        let occ = Occupancy::build(&self.state.map, &self.state.entities);
        for e in self.state.entities.iter().filter(|e| e.is_unit()) {
            if is_collision_anchored(e) {
                continue;
            }
            if e.kind == EntityKind::ScoutPlane {
                continue;
            }
            assert!(
                standability::unit_static_standable_with_facing(
                    &self.state.map,
                    &occ,
                    e.kind,
                    e.pos_x,
                    e.pos_y,
                    e.facing()
                ),
                "invariant: tick {} unit body is not static-standable; {}",
                self.state.tick,
                entity_context(&self.state.map, e)
            );
        }

        // ------------------------------------------------------------------
        // 6. Resource-node miner reservations are valid or ignored
        // ------------------------------------------------------------------
        for e in self.state.entities.iter() {
            if !e.is_node() {
                continue;
            }
            if let Some(miner_id) = e.miner() {
                assert_eq!(
                    self.state.entities.node_slot_holder(e.id),
                    Some(miner_id),
                    "invariant: node {} miner {} is not a valid harvest-slot holder",
                    e.id,
                    miner_id
                );
            }
        }

        // ------------------------------------------------------------------
        // 7. Orders do not point at invalid required targets
        //    (transition windows where a target just died are allowed because
        //     death_system cleans them up on the same tick).
        // ------------------------------------------------------------------
        for e in self.state.entities.iter() {
            if !e.is_unit() {
                continue;
            }
            match e.order() {
                Order::Attack(_) => {
                    let Some(target) = e.order().attack_target() else {
                        continue;
                    };
                    if let Some(t) = self.state.entities.get(target) {
                        assert!(
                            t.is_targetable() && t.hp > 0,
                            "invariant: entity {} Attack order targets invalid entity {} (hp {} targetable {})",
                            e.id, target, t.hp, t.is_targetable()
                        );
                    }
                }
                Order::Gather(_) => {
                    let Some(node) = e.order().gather_node() else {
                        continue;
                    };
                    if let Some(n) = self.state.entities.get(node) {
                        assert!(
                            n.is_node() && n.remaining().unwrap_or(0) > 0,
                            "invariant: entity {} Gather order targets invalid node {} (kind {:?} remaining {})",
                            e.id, node, n.kind, n.remaining().unwrap_or(0)
                        );
                    }
                }
                Order::Build(_) => {
                    let Some(site) = e.order().build_site() else {
                        continue;
                    };
                    if let Some(b) = self.state.entities.get(site) {
                        assert!(
                            b.is_building() && b.under_construction(),
                            "invariant: entity {} Build order targets invalid site {} (building {} under_construction {})",
                            e.id, site, b.is_building(), b.under_construction()
                        );
                    }
                }
                Order::Deconstruct(_) => {
                    let Some(target) = e.order().deconstruct_target() else {
                        continue;
                    };
                    if let Some(t) = self.state.entities.get(target) {
                        assert!(
                            t.kind == EntityKind::TankTrap && t.hp > 0 && !t.under_construction(),
                            "invariant: entity {} Deconstruct order targets invalid trap {} (kind {:?} hp {} under_construction {})",
                            e.id,
                            target,
                            t.kind,
                            t.hp,
                            t.under_construction()
                        );
                    }
                }
                _ => {}
            }
        }

        // ------------------------------------------------------------------
        // 8. Fog grids exist for all players and never for neutral owner
        // ------------------------------------------------------------------
        for &pid in &player_ids {
            assert!(
                self.state.fog.has_grid(pid),
                "invariant: fog grid missing for player {}",
                pid
            );
        }
        assert!(
            !self.state.fog.has_grid(NEUTRAL),
            "invariant: fog grid must not exist for neutral owner (0)"
        );

        // ------------------------------------------------------------------
        // 9. Mobile units do not stack on top of each other (PLAN §4.3).
        //     Harvesting workers are anchored to their resource node and excluded — they
        //     intentionally cannot be pushed by collision. All other mobile-unit pairs must
        //     keep body overlap within `OVERLAP_TOLERANCE_PX` of floating-point and terrain-pinned
        //     residue.
        // ------------------------------------------------------------------
        let units: Vec<_> = self.state.entities.iter().filter(|e| e.is_unit()).collect();
        for i in 0..units.len() {
            let a = units[i];
            if is_collision_anchored(a) {
                continue;
            }
            for &b in units.iter().skip(i + 1) {
                if is_collision_anchored(b) {
                    continue;
                }
                let Some(a_body) = unit_body_for_entity(a) else {
                    continue;
                };
                let Some(b_body) = unit_body_for_entity(b) else {
                    continue;
                };
                let overlap = unit_body_overlap(a_body, b_body).map_or(0.0, |o| o.depth);
                assert!(
                    overlap <= OVERLAP_TOLERANCE_PX,
                    "invariant: tick {} unit bodies overlap by {:.2}px; a={}; b={}; midpoint={}",
                    self.state.tick,
                    overlap,
                    entity_context(&self.state.map, a),
                    entity_context(&self.state.map, b),
                    location_context(
                        &self.state.map,
                        (a.pos_x + b.pos_x) * 0.5,
                        (a.pos_y + b.pos_y) * 0.5
                    )
                );
            }
        }

        // ------------------------------------------------------------------
        // 10. Snapshots never expose hidden enemy ids through entities or targets
        // ------------------------------------------------------------------
        for &pid in &player_ids {
            let snap = self.snapshot_for(pid);
            let live_fog = self.invariant_team_current_fog_for(pid, &self.state.fog);
            for v in &snap.entities {
                if v.owner == pid || v.owner == NEUTRAL || self.same_team_owner(pid, v.owner) {
                    continue;
                }
                let live_visible = live_fog.is_visible_world(pid, v.x, v.y);
                // Enemy entities must either be live-visible or explicitly marked as legacy/special
                // render-only intel.
                if v.vision_only {
                    assert!(
                        !live_visible,
                        "invariant: tick {} snapshot for player {} marks live-visible enemy entity {} as vision-only at {}",
                        self.state.tick,
                        pid,
                        v.id,
                        location_context(&self.state.map, v.x, v.y)
                    );
                } else {
                    assert!(
                        live_visible,
                        "invariant: tick {} snapshot for player {} exposes hidden enemy entity {} at {}",
                        self.state.tick,
                        pid,
                        v.id,
                        location_context(&self.state.map, v.x, v.y)
                    );
                }
                // If a target_id is exposed, the target must be visible too.
                if let Some(tid) = v.target_id {
                    if let Some(t) = self.state.entities.get(tid) {
                        let visible =
                            v.owner == pid || self.state.fog.is_visible_world(pid, t.pos_x, t.pos_y);
                        assert!(
                            visible,
                            "invariant: tick {} snapshot for player {} exposes hidden target_id {}; target={}",
                            self.state.tick,
                            pid,
                            tid,
                            entity_context(&self.state.map, t)
                        );
                    }
                }
            }
        }
    }

    fn invariant_team_current_fog_for(&self, player: u32, fog: &Fog) -> Fog {
        let mut visible_players = self.living_team_player_ids_for_vision(player);
        if visible_players.is_empty() {
            visible_players.push(player);
        }
        fog.union_for(player, &visible_players)
    }
}

fn entity_context(map: &Map, e: &Entity) -> String {
    format!(
        "id={} kind={} owner={} hp={}/{} state={} pos={} radius={:.2} order={:?} target_id={:?} anchored={}",
        e.id,
        e.kind,
        e.owner,
        e.hp,
        e.max_hp,
        e.state_str(),
        location_context(map, e.pos_x, e.pos_y),
        e.radius(),
        e.order(),
        e.target_id(),
        is_collision_anchored(e)
    )
}

fn location_context(map: &Map, x: f32, y: f32) -> String {
    if !x.is_finite() || !y.is_finite() {
        return format!(
            "world=({x}, {y}) tile=(n/a) region=n/a map={}x{} tiles",
            map.size, map.size
        );
    }

    let (tile_x, tile_y) = map.tile_of(x, y);
    format!(
        "world=({:.2}, {:.2}) tile=({}, {}) region={} map={}x{} tiles/{:.0}px",
        x,
        y,
        tile_x,
        tile_y,
        map_region(map, x, y),
        map.size,
        map.size,
        map.world_size_px()
    )
}

fn tile_location_context(map: &Map, tile: (u32, u32)) -> String {
    let max_tile = map.size.saturating_sub(1);
    let (x, y) = map.tile_center(tile.0.min(max_tile), tile.1.min(max_tile));
    format!("center={}", location_context(map, x, y))
}

fn rect_context(map: &Map, rect: RectBody) -> String {
    let center_x = (rect.min_x + rect.max_x) * 0.5;
    let center_y = (rect.min_y + rect.max_y) * 0.5;
    format!(
        "rect=[{:.2},{:.2}]-[{:.2},{:.2}] center={}",
        rect.min_x,
        rect.min_y,
        rect.max_x,
        rect.max_y,
        location_context(map, center_x, center_y)
    )
}

fn unit_body_rect_collision_context(map: &Map, body: UnitBody, rect: RectBody) -> String {
    match body {
        UnitBody::Circle(circle) => circle_rect_collision_context(map, circle, rect),
        UnitBody::OrientedCapsule(capsule) => {
            let aabb = UnitBody::OrientedCapsule(capsule).aabb();
            format!(
                "oriented_capsule_center={} half_segment={:.2} radius={:.2} facing={:.3}rad bounding_aabb=[{:.2},{:.2}]-[{:.2},{:.2}] overlap_depth={:.4}px",
                location_context(map, capsule.x, capsule.y),
                capsule.half_segment,
                capsule.radius,
                capsule.facing,
                aabb.min_x,
                aabb.min_y,
                aabb.max_x,
                aabb.max_y,
                unit_body_rect_overlap_depth(body, rect)
            )
        }
        UnitBody::OrientedBox(oriented) => {
            let aabb = UnitBody::OrientedBox(oriented).aabb();
            format!(
                "oriented_box_center={} half_len={:.2} half_width={:.2} facing={:.3}rad bounding_aabb=[{:.2},{:.2}]-[{:.2},{:.2}] overlap_depth={:.4}px",
                location_context(map, oriented.x, oriented.y),
                oriented.half_len,
                oriented.half_width,
                oriented.facing,
                aabb.min_x,
                aabb.min_y,
                aabb.max_x,
                aabb.max_y,
                unit_body_rect_overlap_depth(body, rect)
            )
        }
    }
}

fn unit_body_rect_overlap_depth(body: UnitBody, rect: RectBody) -> f32 {
    match body {
        UnitBody::Circle(circle) => circle_rect_overlap_depth(circle, rect),
        UnitBody::OrientedCapsule(capsule) => {
            let (start, end) =
                capsule_endpoints(capsule.x, capsule.y, capsule.half_segment, capsule.facing);
            (capsule.radius - segment_rect_distance_sq(start, end, rect).sqrt()).max(0.0)
        }
        UnitBody::OrientedBox(oriented) => oriented_box_rect_overlap_depth(oriented, rect),
    }
}

fn circle_rect_overlap_depth(circle: CircleBody, rect: RectBody) -> f32 {
    let nearest_x = circle.x.clamp(rect.min_x, rect.max_x);
    let nearest_y = circle.y.clamp(rect.min_y, rect.max_y);
    let dx = circle.x - nearest_x;
    let dy = circle.y - nearest_y;
    (circle.radius - (dx * dx + dy * dy).sqrt()).max(0.0)
}

fn oriented_box_rect_overlap_depth(
    body: crate::game::services::geometry::OrientedBoxBody,
    rect: RectBody,
) -> f32 {
    let rect_center = (
        (rect.min_x + rect.max_x) * 0.5,
        (rect.min_y + rect.max_y) * 0.5,
    );
    let rect_half = (
        (rect.max_x - rect.min_x) * 0.5,
        (rect.max_y - rect.min_y) * 0.5,
    );
    let forward = (body.facing.cos(), body.facing.sin());
    let side = (-forward.1, forward.0);
    let delta = (rect_center.0 - body.x, rect_center.1 - body.y);
    let axes = [forward, side, (1.0, 0.0), (0.0, 1.0)];
    let mut best_depth = f32::INFINITY;

    for axis in axes {
        let center_dist = (delta.0 * axis.0 + delta.1 * axis.1).abs();
        let body_extent =
            body.half_len * dot_abs(axis, forward) + body.half_width * dot_abs(axis, side);
        let rect_extent = rect_half.0 * axis.0.abs() + rect_half.1 * axis.1.abs();
        let depth = body_extent + rect_extent - center_dist;
        if depth <= 0.0 {
            return 0.0;
        }
        best_depth = best_depth.min(depth);
    }

    best_depth
}

fn segment_rect_distance_sq(start: (f32, f32), end: (f32, f32), rect: RectBody) -> f32 {
    if segment_intersects_rect(start, end, rect).is_some() {
        return 0.0;
    }

    let corners = [
        (rect.min_x, rect.min_y),
        (rect.max_x, rect.min_y),
        (rect.max_x, rect.max_y),
        (rect.min_x, rect.max_y),
    ];
    let edges = [
        (corners[0], corners[1]),
        (corners[1], corners[2]),
        (corners[2], corners[3]),
        (corners[3], corners[0]),
    ];

    let mut best = point_rect_distance_sq(start, rect).min(point_rect_distance_sq(end, rect));
    for &(a, b) in &edges {
        best = best.min(segment_segment_distance_sq(start, end, a, b));
    }
    best
}

fn point_rect_distance_sq(point: (f32, f32), rect: RectBody) -> f32 {
    let nearest_x = point.0.clamp(rect.min_x, rect.max_x);
    let nearest_y = point.1.clamp(rect.min_y, rect.max_y);
    let dx = point.0 - nearest_x;
    let dy = point.1 - nearest_y;
    dx * dx + dy * dy
}

fn segment_segment_distance_sq(
    a0: (f32, f32),
    a1: (f32, f32),
    b0: (f32, f32),
    b1: (f32, f32),
) -> f32 {
    point_segment_distance_sq(a0, b0, b1)
        .min(point_segment_distance_sq(a1, b0, b1))
        .min(point_segment_distance_sq(b0, a0, a1))
        .min(point_segment_distance_sq(b1, a0, a1))
}

fn point_segment_distance_sq(point: (f32, f32), start: (f32, f32), end: (f32, f32)) -> f32 {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len_sq = dx * dx + dy * dy;
    if len_sq <= f32::EPSILON {
        let px = point.0 - start.0;
        let py = point.1 - start.1;
        return px * px + py * py;
    }
    let t = (((point.0 - start.0) * dx + (point.1 - start.1) * dy) / len_sq).clamp(0.0, 1.0);
    let closest = (start.0 + dx * t, start.1 + dy * t);
    let px = point.0 - closest.0;
    let py = point.1 - closest.1;
    px * px + py * py
}

fn capsule_endpoints(x: f32, y: f32, half_segment: f32, facing: f32) -> ((f32, f32), (f32, f32)) {
    let forward = (facing.cos(), facing.sin());
    (
        (x - forward.0 * half_segment, y - forward.1 * half_segment),
        (x + forward.0 * half_segment, y + forward.1 * half_segment),
    )
}

fn dot_abs(a: (f32, f32), b: (f32, f32)) -> f32 {
    (a.0 * b.0 + a.1 * b.1).abs()
}

fn circle_rect_collision_context(map: &Map, circle: CircleBody, rect: RectBody) -> String {
    let nearest_x = circle.x.clamp(rect.min_x, rect.max_x);
    let nearest_y = circle.y.clamp(rect.min_y, rect.max_y);
    let dx = circle.x - nearest_x;
    let dy = circle.y - nearest_y;
    let distance = (dx * dx + dy * dy).sqrt();
    let overlap = circle.radius - distance;
    format!(
        "circle_center={} circle_radius={:.2} nearest_rect_point={} overlap_depth={:.2}px",
        location_context(map, circle.x, circle.y),
        circle.radius,
        location_context(map, nearest_x, nearest_y),
        overlap.max(0.0)
    )
}

fn map_region(map: &Map, x: f32, y: f32) -> String {
    let world_size = map.world_size_px();
    if world_size <= 0.0 {
        return "unknown".to_string();
    }

    let horizontal = third_label(x / world_size, "left", "middle", "right");
    let vertical = third_label(y / world_size, "top", "middle", "bottom");
    if horizontal == "middle" && vertical == "middle" {
        "middle".to_string()
    } else {
        format!("{vertical} {horizontal}")
    }
}

fn third_label(
    value: f32,
    low: &'static str,
    mid: &'static str,
    high: &'static str,
) -> &'static str {
    if value < 1.0 / 3.0 {
        low
    } else if value < 2.0 / 3.0 {
        mid
    } else {
        high
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::str::FromStr;

    use super::{location_context, unit_body_rect_overlap_depth, STATIC_BODY_OVERLAP_TOLERANCE_PX};
    use crate::config;
    use crate::game::entity::EntityKind;
    use crate::game::map::Map;
    use crate::game::services::geometry::{building_rect_for_footprint, unit_body_with_facing};
    use crate::game::services::occupancy::footprint_center;
    use crate::game::{Game, PlayerInit};
    use crate::protocol::terrain;

    /// Steel patch placement must stay within City Centre distance bounds for any STEEL_PATCHES_PER_BASE.
    /// Regression: doubling patches to 24 caused rows 2/3 to exceed CC_RESOURCE_MAX_DIST_TILES.
    #[test]
    fn steel_patch_grid_fits_within_distance_bounds() {
        // Game::new triggers spawn_player_base which debug_asserts every patch is in bounds.
        let players = [
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "A".into(),
                color: "#fff".into(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "B".into(),
                color: "#000".into(),
                is_ai: true,
            },
        ];
        // This panics before the fix when STEEL_PATCHES_PER_BASE = 24.
        let _game = Game::new(&players, 0x1234_5678);
    }

    /// A freshly-created game must satisfy every invariant before any tick runs.
    #[test]
    fn invariants_hold_at_game_start() {
        let players = [
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "A".into(),
                color: "#fff".into(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "B".into(),
                color: "#000".into(),
                is_ai: true,
            },
        ];
        let game = Game::new(&players, 0x1234_5678);
        game.assert_invariants();
    }

    #[test]
    fn unit_body_vs_building_invariant_catches_manual_bad_state() {
        let players = [PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Solo".into(),
            color: "#fff".into(),
            is_ai: false,
        }];
        let mut game = Game::new(&players, 0x1234_5678);
        for tile in &mut game.state.map.terrain {
            *tile = crate::protocol::terrain::GRASS;
        }

        let (bx, by) = footprint_center(&game.state.map, EntityKind::Depot, 20, 20);
        game.state.entities
            .spawn_building(99, EntityKind::Depot, bx, by, true)
            .expect("building spawn");
        let rect = building_rect_for_footprint(EntityKind::Depot, 20, 20).expect("depot rect");
        let radius = config::unit_stats(EntityKind::Tank)
            .expect("tank stats")
            .radius;
        game.state.entities
            .spawn_unit(
                99,
                EntityKind::Tank,
                rect.max_x + radius - 1.0,
                rect.min_y + 32.0,
            )
            .expect("tank spawn");

        let message = invariant_panic_message(&game);
        assert!(message.contains("unit body intersects building footprint"));
        assert!(message.contains("tick 0"));
        assert!(message.contains("unit=id="));
        assert!(message.contains("building=id="));
        assert!(message.contains("world=("));
        assert!(message.contains("tile=("));
        assert!(message.contains("region="));
        assert!(message.contains("oriented_box_center="));
        assert!(message.contains("half_len="));
        assert!(message.contains("half_width="));
        assert!(message.contains("facing="));
    }

    #[test]
    fn unit_building_invariant_tolerates_tangent_contact_only() {
        let rect = building_rect_for_footprint(EntityKind::Depot, 10, 10).expect("depot rect");
        let tank_stats = config::unit_stats(EntityKind::Tank).expect("tank stats");
        let tangent_x =
            rect.max_x + config::TANK_BODY_LENGTH_PX * 0.5 + config::TANK_BODY_CLEARANCE_PX;
        let tangent = unit_body_with_facing(
            EntityKind::Tank,
            tangent_x,
            rect.min_y + config::TILE_SIZE as f32,
            0.0,
        )
        .expect("tank body");
        assert!(unit_body_rect_overlap_depth(tangent, rect) <= STATIC_BODY_OVERLAP_TOLERANCE_PX);

        let meaningful_overlap = unit_body_with_facing(
            EntityKind::Tank,
            tangent_x - STATIC_BODY_OVERLAP_TOLERANCE_PX * 2.0,
            rect.min_y + config::TILE_SIZE as f32,
            0.0,
        )
        .expect("tank body");
        assert!(
            unit_body_rect_overlap_depth(meaningful_overlap, rect)
                > STATIC_BODY_OVERLAP_TOLERANCE_PX,
            "overlap should exceed invariant tolerance; tank radius is {}",
            tank_stats.radius
        );
    }

    #[test]
    fn resource_body_vs_building_invariant_catches_manual_bad_state() {
        let players = [PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Solo".into(),
            color: "#fff".into(),
            is_ai: false,
        }];
        let mut game = Game::new(&players, 0x1234_5678);
        for tile in &mut game.state.map.terrain {
            *tile = crate::protocol::terrain::GRASS;
        }

        let (bx, by) = footprint_center(&game.state.map, EntityKind::Depot, 20, 20);
        game.state.entities
            .spawn_building(99, EntityKind::Depot, bx, by, true)
            .expect("building spawn");
        let rect = building_rect_for_footprint(EntityKind::Depot, 20, 20).expect("depot rect");
        game.state.entities
            .spawn_node(
                EntityKind::Steel,
                rect.max_x + config::TILE_SIZE as f32 * 0.25,
                rect.min_y + config::TILE_SIZE as f32 * 0.5,
            )
            .expect("resource spawn");

        let message = invariant_panic_message(&game);
        assert!(message.contains("resource node body overlaps building footprint"));
        assert!(message.contains("node=id="));
        assert!(message.contains("building=id="));
        assert!(message.contains("world=("));
        assert!(message.contains("tile=("));
        assert!(message.contains("region="));
        assert!(message.contains("overlap_depth="));
    }

    #[test]
    fn pump_jack_oil_overlap_is_valid_invariant_state() {
        let players = [PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Solo".into(),
            color: "#fff".into(),
            is_ai: false,
        }];
        let mut game = Game::new(&players, 0x1234_5678);
        for tile in &mut game.state.map.terrain {
            *tile = crate::protocol::terrain::GRASS;
        }

        let (x, y) = footprint_center(&game.state.map, EntityKind::PumpJack, 20, 20);
        let oil_kind = EntityKind::from_str("oil").expect("oil kind");
        game.state.entities
            .spawn_node(oil_kind, x, y)
            .expect("oil spawn");
        game.state.entities
            .spawn_building(1, EntityKind::PumpJack, x, y, true)
            .expect("pump jack spawn");

        game.assert_invariants();
    }

    #[test]
    fn location_context_describes_human_map_region() {
        let map = Map {
            size: 30,
            terrain: vec![terrain::GRASS; 30 * 30],
            starts: vec![],
            base_sites: vec![],
        };
        let ts = config::TILE_SIZE as f32;

        let context = location_context(&map, 15.25 * ts, 25.5 * ts);

        assert!(context.contains("world=("));
        assert!(context.contains("tile=(15, 25)"));
        assert!(context.contains("region=bottom middle"));
    }
    /// A human-only sandbox with no commands must keep invariants across ticks.
    #[test]
    fn invariants_hold_in_no_command_sandbox() {
        let players = [PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Solo".into(),
            color: "#fff".into(),
            is_ai: false,
        }];
        let mut game = Game::new(&players, 0x1234_5678);
        for _ in 0..300 {
            game.tick();
        }
        game.assert_invariants();
    }

    fn invariant_panic_message(game: &Game) -> String {
        let payload = catch_unwind(AssertUnwindSafe(|| game.assert_invariants()))
            .expect_err("expected invariant panic");
        if let Some(message) = payload.downcast_ref::<String>() {
            return message.clone();
        }
        if let Some(message) = payload.downcast_ref::<&'static str>() {
            return message.to_string();
        }
        "non-string panic payload".to_string()
    }
}
