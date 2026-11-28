//! Keep armor concentrated at a body-clear reserve near the remaining-Steel centroid.
use super::construction::dist2;
use crate::sdk::{
    AiActions, AiBaseSiteId, AiEntityState, AiFrame, AiGroundReachability, AiTile, DefensePlan,
    EntityKind, UnitGroup, WorldQueries,
};
use rts_rules::math::GameplayMath as _;
use std::collections::{BTreeMap, BTreeSet};

type SectorKey = (Vec<AiBaseSiteId>, u8, (u32, u32));
#[derive(Clone, Debug)]
struct Post {
    sector: SectorKey,
    position: (f32, f32),
}
#[derive(Default, Debug)]
pub(super) struct ArmorAssignments {
    posts: BTreeMap<u32, Post>,
    issued: BTreeMap<u32, ((f32, f32), u32)>,
    progress: BTreeMap<u32, ((f32, f32), u32)>,
    held: BTreeSet<u32>,
    assault: bool,
}
impl ArmorAssignments {
    pub(super) fn step(
        &mut self,
        frame: &AiFrame,
        actions: &mut AiActions,
        plan: &DefensePlan,
        responding: &BTreeSet<u32>,
        reserved: Option<AiTile>,
    ) {
        self.step_goal(frame, actions, plan, responding, reserved, (None, None));
    }
    pub(super) fn step_goal(
        &mut self,
        frame: &AiFrame,
        actions: &mut AiActions,
        plan: &DefensePlan,
        responding: &BTreeSet<u32>,
        reserved: Option<AiTile>,
        goal: (Option<AiBaseSiteId>, Option<(f32, f32)>),
    ) {
        let (future, attack) = goal;
        if self.assault != attack.is_some() {
            self.issued.clear();
            self.held.clear();
            self.assault = attack.is_some();
        }
        let tanks: Vec<_> = frame
            .owned()
            .iter()
            .filter(|e| {
                e.kind == EntityKind::Tank && e.health.current > 0 && !responding.contains(&e.id)
            })
            .collect();
        let alive: BTreeSet<_> = tanks.iter().map(|e| e.id).collect();
        self.posts.retain(|id, _| alive.contains(id));
        self.issued.retain(|id, _| alive.contains(id));
        self.progress.retain(|id, _| alive.contains(id));
        self.held.retain(|id| alive.contains(id));
        if tanks.is_empty() {
            return;
        }
        let q = WorldQueries::new(frame);
        let mut fields: Vec<AiGroundReachability> = vec![];
        let mut component = BTreeMap::new();
        for tank in &tanks {
            let Some(point) = q.world_point(tank.position.0, tank.position.1) else {
                continue;
            };
            let field = fields
                .iter()
                .position(|field| field.contains(point))
                .or_else(|| {
                    let field = q.ground_reachability(point, EntityKind::Tank)?;
                    fields.push(field);
                    Some(fields.len() - 1)
                });
            if let Some(field) = field {
                component.insert(tank.id, field);
            }
        }
        let size = frame.map().tile_size as f32;
        let reachable = |id: u32, p: (f32, f32)| {
            component.get(&id).is_some_and(|index| {
                q.world_point(p.0, p.1)
                    .is_some_and(|point| fields[*index].contains(point))
            })
        };
        let spaced = |p: (f32, f32), used: &[(f32, f32)]| {
            used.iter()
                .all(|other| dist2(*other, p) >= (2. * size).det_powi(2))
        };
        let clear = |p: (f32, f32), used: &[(f32, f32)]| {
            spaced(p, used)
                && plan
                    .slots
                    .iter()
                    .all(|slot| dist2(slot.position, p) >= (2. * size).det_powi(2))
                && !reserved.is_some_and(|tile| {
                    let Some(foot) = rts_rules::balance::building_stats(EntityKind::ResourceDepot)
                    else {
                        return true;
                    };
                    crate::sdk::unit_circle_touches_rect(
                        p,
                        crate::sdk::unit_placement_radius(EntityKind::Tank) + size,
                        (
                            tile.x() as f32 * size,
                            tile.y() as f32 * size,
                            (tile.x() + foot.foot_w) as f32 * size,
                            (tile.y() + foot.foot_h) as f32 * size,
                        ),
                    )
                })
        };
        let center = attack.unwrap_or_else(|| reserve_center(frame, future));
        let mut assigned: BTreeMap<u32, Post> = BTreeMap::new();
        let mut used = vec![];
        // One compact reserve per reachable component; disconnected armor cannot share a post.
        for (index, field) in fields.iter().enumerate() {
            let members: Vec<_> = tanks
                .iter()
                .filter(|t| component.get(&t.id) == Some(&index))
                .collect();
            let mut candidates = reserve_points(center, size, 24);
            candidates.retain(|p| {
                clear(*p, &[]) && q.world_point(p.0, p.1).is_some_and(|p| field.contains(p))
            });
            // Only fall back to a tank-local grid if the centroid grid cannot reach this pocket.
            // Mixing moving-tank grids into the normal reserve made its anchor drift with traffic.
            if candidates.is_empty() {
                for tank in &members {
                    candidates.extend(reserve_points(tank.position, size, 5));
                }
                candidates.retain(|p| {
                    clear(*p, &[]) && q.world_point(p.0, p.1).is_some_and(|p| field.contains(p))
                });
            }
            candidates.sort_by(|a, b| {
                dist2(*a, center)
                    .total_cmp(&dist2(*b, center))
                    .then(a.0.total_cmp(&b.0))
                    .then(a.1.total_cmp(&b.1))
            });
            let Some(best) = candidates.first().copied() else {
                continue;
            };
            // Small stock changes must not keep reshuffling a parked formation.
            let anchor = members
                .iter()
                .filter_map(|t| self.posts.get(&t.id))
                .map(|post| {
                    (
                        f32::from_bits(post.sector.2 .0),
                        f32::from_bits(post.sector.2 .1),
                    )
                })
                .find(|old| {
                    dist2(*old, best) <= (2. * size).det_powi(2)
                        && clear(*old, &[])
                        && q.world_point(old.0, old.1)
                            .is_some_and(|p| field.contains(p))
                })
                .unwrap_or(best);
            candidates.sort_by(|a, b| {
                dist2(*a, anchor)
                    .total_cmp(&dist2(*b, anchor))
                    .then(a.0.total_cmp(&b.0))
                    .then(a.1.total_cmp(&b.1))
            });
            let sector = (Vec::new(), 0, (anchor.0.to_bits(), anchor.1.to_bits()));
            for tank in &members {
                if let Some(old) = self.posts.get(&tank.id) {
                    if old.sector == sector
                        && reachable(tank.id, old.position)
                        && clear(old.position, &used)
                    {
                        used.push(old.position);
                        assigned.insert(tank.id, old.clone());
                    }
                }
            }
            for tank in members {
                if assigned.contains_key(&tank.id) {
                    continue;
                }
                if let Some(point) = candidates.iter().copied().find(|p| clear(*p, &used)) {
                    used.push(point);
                    assigned.insert(
                        tank.id,
                        Post {
                            sector: sector.clone(),
                            position: point,
                        },
                    );
                }
            }
        }
        for tank in tanks {
            let Some(post) = assigned.get(&tank.id) else {
                continue;
            };
            let progress = self
                .progress
                .entry(tank.id)
                .or_insert((tank.position, frame.tick()));
            if dist2(progress.0, tank.position) > (size * 0.5).det_powi(2) {
                *progress = (tank.position, frame.tick());
            }
            let Ok(group) = UnitGroup::new([tank.id]) else {
                continue;
            };
            if dist2(tank.position, post.position) <= (size * 0.5).det_powi(2) {
                if !self.held.contains(&tank.id) && actions.hold_position(&group, false).is_ok() {
                    self.held.insert(tank.id);
                }
                continue;
            }
            let changed = self
                .issued
                .get(&tank.id)
                .is_none_or(|(old, _)| dist2(*old, post.position) > 1.);
            let retry = self
                .issued
                .get(&tank.id)
                .is_none_or(|(_, tick)| frame.tick().saturating_sub(*tick) >= 90)
                && (tank.state == AiEntityState::Idle
                    || frame.tick().saturating_sub(progress.1) >= 90);
            if !changed && !retry {
                continue;
            }
            let route = q
                .world_point(tank.position.0, tank.position.1)
                .zip(q.world_point(post.position.0, post.position.1))
                .and_then(|(from, to)| q.ground_route(from, to, EntityKind::Tank));
            let Some(route) = route else { continue };
            let points = crate::sdk::ground_route_corners(
                &route.iter().map(|p| p.as_tuple()).collect::<Vec<_>>(),
            );
            // Bound queued route batches below the native order queue cap. An idle Tank
            // replans the remainder after reaching a partial route's last waypoint.
            let points: Vec<_> = points.into_iter().skip(1).take(24).collect();
            if let Some((first, rest)) = points.split_first() {
                if (if attack.is_some() {
                    actions.attack_move_route(&group, *first, rest)
                } else {
                    actions.move_route(&group, *first, rest)
                })
                .is_ok()
                {
                    self.issued.insert(tank.id, (post.position, frame.tick()));
                    self.progress.insert(tank.id, (tank.position, frame.tick()));
                    self.held.remove(&tank.id);
                }
            }
        }
        self.posts = assigned;
    }
    pub(super) fn guard_posts(&self) -> impl Iterator<Item = (u32, (f32, f32))> + '_ {
        self.posts.iter().map(|(id, p)| (*id, p.position))
    }
    #[cfg(test)]
    pub(super) fn posts(&self) -> BTreeMap<u32, (SectorKey, (f32, f32))> {
        self.posts
            .iter()
            .map(|(id, p)| (*id, (p.sector.clone(), p.position)))
            .collect()
    }
}
fn reserve_center(frame: &AiFrame, future: Option<AiBaseSiteId>) -> (f32, f32) {
    let mut sites: BTreeSet<_> = frame
        .owned()
        .iter()
        .filter(|e| e.kind == EntityKind::ResourceDepot && super::complete(e))
        .filter_map(|e| {
            frame
                .base_sites()
                .iter()
                .min_by(|a, b| {
                    dist2(a.position, e.position).total_cmp(&dist2(b.position, e.position))
                })
                .map(|s| s.id)
        })
        .collect();
    sites.extend(future);
    let mut weighted = (0., 0.);
    let mut total = 0.;
    let mut fallback = (0., 0.);
    let mut count = 0.;
    for site in frame.base_sites().iter().filter(|s| sites.contains(&s.id)) {
        let weight: f32 = frame
            .resources()
            .iter()
            .filter(|r| r.kind == EntityKind::Steel && site.resource_ids.contains(&r.id))
            .map(|r| match r.remaining {
                crate::sdk::AiResourceAmount::Known(n) => n as f32,
                crate::sdk::AiResourceAmount::Unknown => 312.5,
            })
            .sum();
        weighted.0 += site.position.0 * weight;
        weighted.1 += site.position.1 * weight;
        total += weight;
        fallback.0 += site.position.0;
        fallback.1 += site.position.1;
        count += 1.;
    }
    if total > 0. {
        (weighted.0 / total, weighted.1 / total)
    } else if count > 0. {
        (fallback.0 / count, fallback.1 / count)
    } else {
        frame
            .owned()
            .iter()
            .find(|e| e.kind == EntityKind::Tank)
            .map_or((0., 0.), |e| e.position)
    }
}
fn reserve_points(center: (f32, f32), size: f32, radius: i32) -> Vec<(f32, f32)> {
    // A modest two-tile square grid: rotating hulls fit without a single shared destination.
    let lattice = size * 2.;
    let center = (
        (center.0 / lattice).round() * lattice,
        (center.1 / lattice).round() * lattice,
    );
    let mut points = vec![];
    for y in -radius..=radius {
        for x in -radius..=radius {
            points.push((center.0 + x as f32 * lattice, center.1 + y as f32 * lattice));
        }
    }
    points
}
