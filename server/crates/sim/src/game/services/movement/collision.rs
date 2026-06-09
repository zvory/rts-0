use crate::config;
use crate::game::entity::{uses_oriented_vehicle_body, EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::services::geometry::{unit_body_for_entity, unit_body_overlap, UnitBody};
use crate::game::services::occupancy::Occupancy;
use crate::game::services::spatial::SpatialIndex;

use super::standability::{
    footing_profile, footing_resistance, unit_static_standable, FootingProfile,
};
use super::MAX_UNIT_BOUNDING_RADIUS_PX;

/// Extra slack added to the broad-phase query so small per-pass position drift never causes a
/// missed pair. One tile is generous: the largest per-tick displacement is bounded by speed
/// (~2 px) plus a single push (<= overlap distance), both well under a tile.
const COLLISION_SEARCH_SLACK_PX: f32 = config::TILE_SIZE as f32;

/// Maximum number of pair-resolution passes per tick. Each pass pushes overlapping pairs apart
/// by the full violation; two-body cases converge in one pass and dense clusters typically
/// converge in 2-3.
const COLLISION_PASSES: usize = 8;

/// Pairs whose center distance is at least `sum_radii - COLLISION_EPS_PX` are considered
/// non-overlapping. Avoids endless micro-pushes from floating-point noise.
pub(super) const COLLISION_EPS_PX: f32 = 0.001;

/// Scale applied to the extra lateral nudge that moves stationary blockers away from a moving
/// vehicle's centerline. The normal overlap-resolution push is unchanged.
const CENTERLINE_LATERAL_PUSH_SCALE: f32 = 0.5;

/// Resolve unit-unit overlaps with iterative pair-wise pushes so units do not stack on top of
/// each other. Push direction and depth come from the shared body geometry: infantry resolve as
/// circles while tanks resolve as oriented hulls. Non-ghost units split the overlap by footing
/// resistance, so lower-resistance units move more.
///
/// **Ghost pass-through exception.** Workers in [`GatherPhase::Harvesting`] or
/// [`BuildPhase::Constructing`] are latched onto their resource/build site and are *fully
/// exempt* from collision: they neither push nor are pushed. This is intentional — walking
/// units must be able to pass through harvesters and active builders without kicking them
/// backward each tick, which would deadlock the economy or strand construction.
///
/// Pushes that would land on impassable terrain or a building footprint are skipped, so a
/// unit cornered by terrain may keep a small residual overlap. The invariant
/// [`Game::assert_invariants`] tolerates ≤ `OVERLAP_TOLERANCE_PX` of overlap to absorb this
/// and floating-point noise.
///
/// Pair iteration is deterministic (sorted ids, then spatial-index order, both stable per
/// tick), which is required by the replay harness.
pub(crate) fn resolve_collisions(
    entities: &mut EntityStore,
    spatial: &SpatialIndex,
    map: &Map,
    occ: &Occupancy,
) {
    let world_max = map.world_size_px() - 0.01;
    let ids = entities.ids();
    let mut candidates = Vec::new();

    for pass in 0..COLLISION_PASSES {
        let mut moved_any = false;
        let rebuilt_spatial;
        let pass_spatial = if pass == 0 {
            spatial
        } else {
            rebuilt_spatial = SpatialIndex::build(entities, map.size);
            &rebuilt_spatial
        };

        for &a in &ids {
            // Ghost units neither push nor are pushed. Other units can transit through their
            // position freely.
            let (ar, a_profile) = match entities.get(a) {
                Some(e) if e.is_unit() => {
                    let profile = footing_profile(e);
                    if profile == FootingProfile::Ghost {
                        continue;
                    }
                    let Some(body) = unit_body_for_entity(e) else {
                        continue;
                    };
                    (body.bounding_radius(), profile)
                }
                _ => continue,
            };
            let (ax_idx, ay_idx) = match entities.get(a) {
                Some(e) => (e.pos_x, e.pos_y),
                None => continue,
            };

            // Broad-phase: collect candidate neighbor ids using the (possibly stale) spatial
            // index plus a one-tile slack so small intra-tick drift never hides an overlap.
            let search_r = ar + MAX_UNIT_BOUNDING_RADIUS_PX + COLLISION_SEARCH_SLACK_PX;
            candidates.clear();
            candidates.extend(
                pass_spatial
                    .ids_in_circle_bbox(ax_idx, ay_idx, search_r)
                    .filter(|&b| b > a),
            );
            candidates.sort_unstable();

            for &b in &candidates {
                let (b_kind, b_profile, b_facing, bx, by, b_path_empty, b_body) =
                    match entities.get(b) {
                        Some(e) if e.is_unit() => {
                            let profile = footing_profile(e);
                            if profile == FootingProfile::Ghost {
                                continue;
                            }
                            let Some(body) = unit_body_for_entity(e) else {
                                continue;
                            };
                            (
                                e.kind,
                                profile,
                                e.facing(),
                                e.pos_x,
                                e.pos_y,
                                e.path_is_empty(),
                                body,
                            )
                        }
                        _ => continue,
                    };
                // Re-read A so we account for displacement applied by earlier pairs in this pass.
                let (a_kind, a_profile_now, a_facing, ax, ay, a_path_empty, a_body) =
                    match entities.get(a) {
                        Some(e) => {
                            let Some(body) = unit_body_for_entity(e) else {
                                break;
                            };
                            (
                                e.kind,
                                footing_profile(e),
                                e.facing(),
                                e.pos_x,
                                e.pos_y,
                                e.path_is_empty(),
                                body,
                            )
                        }
                        None => break,
                    };

                let Some((nx, ny, overlap)) =
                    collision_axis_and_depth(a_body, b_body, ax, ay, bx, by)
                else {
                    continue;
                };
                let a_side = CollisionSide {
                    id: a,
                    kind: a_kind,
                    profile: a_profile_now,
                    facing: a_facing,
                    x: ax,
                    y: ay,
                    path_empty: a_path_empty,
                };
                let b_side = CollisionSide {
                    id: b,
                    kind: b_kind,
                    profile: b_profile,
                    facing: b_facing,
                    x: bx,
                    y: by,
                    path_empty: b_path_empty,
                };
                let (a_lateral, b_lateral) = centerline_lateral_offsets(a_side, b_side, overlap);
                // Both sides are non-ghost at this point: split overlap by resistance. If one
                // side's weighted push lands on impassable terrain or a building footprint, the
                // other side tries to absorb the blocked side's remaining share.
                let (a_share, b_share) = collision_push_shares(a_profile, b_profile);
                let a_base_target = (ax - nx * overlap * a_share, ay - ny * overlap * a_share);
                let b_base_target = (bx + nx * overlap * b_share, by + ny * overlap * b_share);
                let a_target = legal_lateral_target(occ, map, a_side, a_base_target, a_lateral);
                let b_target = legal_lateral_target(occ, map, b_side, b_base_target, b_lateral);
                let a_ok =
                    unit_static_standable(occ, map, a_kind, a_target.0, a_target.1, a_facing);
                let b_ok =
                    unit_static_standable(occ, map, b_kind, b_target.0, b_target.1, b_facing);

                let (a_push, b_push) = match (a_ok, b_ok) {
                    (true, true) => (Some(a_target), Some(b_target)),
                    (true, false) => {
                        let a_full = (ax - nx * overlap, ay - ny * overlap);
                        (
                            if unit_static_standable(occ, map, a_kind, a_full.0, a_full.1, a_facing)
                            {
                                Some(a_full)
                            } else {
                                Some(a_target)
                            },
                            None,
                        )
                    }
                    (false, true) => {
                        let b_full = (bx + nx * overlap, by + ny * overlap);
                        (
                            None,
                            if unit_static_standable(occ, map, b_kind, b_full.0, b_full.1, b_facing)
                            {
                                Some(b_full)
                            } else {
                                Some(b_target)
                            },
                        )
                    }
                    (false, false) => {
                        // Both line-of-centers pushes land on impassable terrain or a building
                        // footprint. Happens when two units meet head-on inside a 1-tile-wide
                        // corridor with a slight lateral offset: the diagonal connecting line has
                        // a perpendicular component that clips into the corridor wall on both
                        // sides. The line-of-centers nudge is hopeless, but a pure axial slide
                        // (along ±X or ±Y by the full overlap) typically frees one side along the
                        // corridor's open axis. Try both axes and accept the first push that
                        // works for each side, so subsequent passes can finish the separation.
                        let need = overlap + COLLISION_EPS_PX;
                        // Push along the cardinal axis whose component most aligns with the
                        // line-of-centers, so the axial slide actually increases separation
                        // instead of accidentally driving the unit toward its partner.
                        let a_sx = if nx >= 0.0 { -need } else { need };
                        let a_sy = if ny >= 0.0 { -need } else { need };
                        let (a_primary, a_secondary) = if nx.abs() >= ny.abs() {
                            ((ax + a_sx, ay), (ax, ay + a_sy))
                        } else {
                            ((ax, ay + a_sy), (ax + a_sx, ay))
                        };
                        let b_sx = -a_sx;
                        let b_sy = -a_sy;
                        let (b_primary, b_secondary) = if nx.abs() >= ny.abs() {
                            ((bx + b_sx, by), (bx, by + b_sy))
                        } else {
                            ((bx, by + b_sy), (bx + b_sx, by))
                        };
                        let a_candidates = [a_primary, a_secondary];
                        let b_candidates = [b_primary, b_secondary];
                        let a_alt = a_candidates.into_iter().find(|&(x, y)| {
                            unit_static_standable(occ, map, a_kind, x, y, a_facing)
                        });
                        let b_alt = b_candidates.into_iter().find(|&(x, y)| {
                            unit_static_standable(occ, map, b_kind, x, y, b_facing)
                        });
                        (a_alt, b_alt)
                    }
                };

                if let Some((nax, nay)) = a_push {
                    if let Some(e) = entities.get_mut(a) {
                        e.pos_x = nax.clamp(0.0, world_max);
                        e.pos_y = nay.clamp(0.0, world_max);
                        moved_any = true;
                    }
                }
                if let Some((nbx, nby)) = b_push {
                    if let Some(e) = entities.get_mut(b) {
                        e.pos_x = nbx.clamp(0.0, world_max);
                        e.pos_y = nby.clamp(0.0, world_max);
                        moved_any = true;
                    }
                }
            }
        }

        if !moved_any {
            break;
        }
    }
}

#[derive(Clone, Copy)]
struct CollisionSide {
    id: u32,
    kind: EntityKind,
    profile: FootingProfile,
    facing: f32,
    x: f32,
    y: f32,
    path_empty: bool,
}

fn centerline_lateral_offsets(
    a: CollisionSide,
    b: CollisionSide,
    overlap: f32,
) -> ((f32, f32), (f32, f32)) {
    (
        lateral_offset_one_way(b, a, overlap).unwrap_or((0.0, 0.0)),
        lateral_offset_one_way(a, b, overlap).unwrap_or((0.0, 0.0)),
    )
}

fn lateral_offset_one_way(
    pusher: CollisionSide,
    blocker: CollisionSide,
    overlap: f32,
) -> Option<(f32, f32)> {
    if !uses_oriented_vehicle_body(pusher.kind)
        || pusher.path_empty
        || !blocker.path_empty
        || !pusher.facing.is_finite()
        || !overlap.is_finite()
        || overlap <= COLLISION_EPS_PX
    {
        return None;
    }
    if matches!(blocker.profile, FootingProfile::Ghost) {
        return None;
    }

    let forward = (pusher.facing.cos(), pusher.facing.sin());
    let side = (-forward.1, forward.0);
    if !forward.0.is_finite() || !forward.1.is_finite() {
        return None;
    }
    let dx = blocker.x - pusher.x;
    let dy = blocker.y - pusher.y;
    let ahead = dx * forward.0 + dy * forward.1;
    if ahead < -COLLISION_EPS_PX {
        return None;
    }

    let lateral = dx * side.0 + dy * side.1;
    let side_sign = if lateral.abs() <= 1.0e-4 {
        if pusher.id < blocker.id {
            1.0
        } else {
            -1.0
        }
    } else {
        lateral.signum()
    };
    let shift = overlap
        .max(config::TILE_SIZE as f32 * 0.125)
        .min(config::TILE_SIZE as f32 * 0.5)
        * CENTERLINE_LATERAL_PUSH_SCALE;
    Some((side.0 * side_sign * shift, side.1 * side_sign * shift))
}

fn legal_lateral_target(
    occ: &Occupancy,
    map: &Map,
    side: CollisionSide,
    base_target: (f32, f32),
    lateral: (f32, f32),
) -> (f32, f32) {
    if lateral.0.abs() <= 1.0e-4 && lateral.1.abs() <= 1.0e-4 {
        return base_target;
    }
    let target = (base_target.0 + lateral.0, base_target.1 + lateral.1);
    if unit_static_standable(occ, map, side.kind, target.0, target.1, side.facing) {
        target
    } else {
        base_target
    }
}

fn collision_axis_and_depth(
    a_body: UnitBody,
    b_body: UnitBody,
    ax: f32,
    ay: f32,
    bx: f32,
    by: f32,
) -> Option<(f32, f32, f32)> {
    if let (UnitBody::Circle(a_circle), UnitBody::Circle(b_circle)) = (a_body, b_body) {
        let dx = bx - ax;
        let dy = by - ay;
        let min_d = a_circle.radius + b_circle.radius;
        let d2 = dx * dx + dy * dy;
        if d2 + COLLISION_EPS_PX >= min_d * min_d {
            return None;
        }
        if d2 < 1.0e-4 {
            return Some((1.0, 0.0, min_d));
        }
        let d = d2.sqrt();
        return Some((dx / d, dy / d, min_d - d));
    }

    let overlap = unit_body_overlap(a_body, b_body)?;
    (overlap.depth > COLLISION_EPS_PX).then_some((
        overlap.normal_x,
        overlap.normal_y,
        overlap.depth,
    ))
}

fn collision_push_shares(a_profile: FootingProfile, b_profile: FootingProfile) -> (f32, f32) {
    match (a_profile, b_profile) {
        (FootingProfile::Heavy, FootingProfile::Soft) => (0.0, 1.0),
        (FootingProfile::Soft, FootingProfile::Heavy) => (1.0, 0.0),
        (FootingProfile::Heavy, FootingProfile::Firm) => (0.1, 0.9),
        (FootingProfile::Firm, FootingProfile::Heavy) => (0.9, 0.1),
        _ => {
            let a_resistance = footing_resistance(a_profile);
            let b_resistance = footing_resistance(b_profile);
            let total_resistance = a_resistance + b_resistance;
            (
                b_resistance / total_resistance,
                a_resistance / total_resistance,
            )
        }
    }
}
