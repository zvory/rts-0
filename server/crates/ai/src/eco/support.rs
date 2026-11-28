//! Command Cars follow the largest engaged Tank group, otherwise the armor reserve.
use crate::sdk::{
    AbilityKind, AiActions, AiEntity, AiEntityState, AiFrame, EntityKind, UnitGroup, WorldQueries,
};
use rts_rules::math::GameplayMath as _;
use std::collections::{BTreeMap, BTreeSet};
type Point = (f32, f32);
#[derive(Default)]
pub(super) struct CommandSupport {
    issued: BTreeMap<u32, (Point, u32)>,
    active_until: u32,
    last_car: Option<u32>,
}
fn dist2(a: Point, b: Point) -> f32 {
    (a.0 - b.0) * (a.0 - b.0) + (a.1 - b.1) * (a.1 - b.1)
}
impl CommandSupport {
    pub(super) fn step(
        &mut self,
        frame: &AiFrame,
        actions: &mut AiActions,
        mobilized: &BTreeSet<u32>,
    ) {
        self.step_mobilized(frame, actions, mobilized, false);
    }
    pub(super) fn step_mobilized(
        &mut self,
        frame: &AiFrame,
        actions: &mut AiActions,
        mobilized: &BTreeSet<u32>,
        expansion_push: bool,
    ) {
        self.issued.retain(|id, _| {
            frame
                .owned()
                .iter()
                .any(|e| e.id == *id && e.health.current > 0)
        });
        let tanks = frame
            .owned()
            .iter()
            .chain(frame.visible_allies())
            .filter(|e| e.kind == EntityKind::Tank && e.health.current > 0)
            .collect::<Vec<_>>();
        let engaged = tanks
            .iter()
            .copied()
            .filter(|e| mobilized.contains(&e.id) || e.state == AiEntityState::Attack)
            .collect::<Vec<_>>();
        let attacking = !engaged.is_empty();
        let candidates = if attacking { &engaged } else { &tanks };
        let size = frame.map().tile_size as f32;
        let radius = rts_rules::balance::BREAKTHROUGH_RADIUS_TILES * size;
        let Some(seed) = candidates.iter().max_by(|a, b| {
            let count = |seed: &AiEntity| {
                candidates
                    .iter()
                    .filter(|e| dist2(e.position, seed.position) <= radius * radius)
                    .count()
            };
            count(a).cmp(&count(b)).then_with(|| b.id.cmp(&a.id))
        }) else {
            return;
        };
        let cluster = candidates
            .iter()
            .filter(|e| dist2(e.position, seed.position) <= radius * radius)
            .collect::<Vec<_>>();
        let center = cluster
            .iter()
            .fold((0., 0.), |p, e| (p.0 + e.position.0, p.1 + e.position.1));
        let center = (
            center.0 / cluster.len() as f32,
            center.1 / cluster.len() as f32,
        );
        let home = frame
            .players()
            .iter()
            .find(|p| p.id == frame.player_id())
            .map(|p| {
                (
                    (p.start_tile.0 as f32 + 0.5) * size,
                    (p.start_tile.1 as f32 + 0.5) * size,
                )
            })
            .unwrap_or(center);
        let length = dist2(center, home).det_sqrt().max(1.);
        // Stay two tiles behind the Tank concentration, with an independently legal parking spot.
        let goal = (
            center.0 + (home.0 - center.0) / length * 2. * size,
            center.1 + (home.1 - center.1) / length * 2. * size,
        );
        let q = WorldQueries::new(frame);
        let mut cars = frame
            .owned()
            .iter()
            .filter(|e| e.kind == EntityKind::CommandCar && e.health.current > 0)
            .collect::<Vec<_>>();
        let observed_active = cars.iter().any(|e| {
            e.abilities
                .iter()
                .any(|a| a.kind == AbilityKind::Breakthrough && a.active_ticks > 0)
        });
        cars.sort_by_key(|e| (self.last_car.is_some_and(|last| e.id <= last), e.id));
        for (index, car) in cars.into_iter().enumerate() {
            let across = if index % 2 == 0 {
                (index / 2) as f32
            } else {
                -(index.div_ceil(2) as f32)
            };
            let goal = (
                goal.0 - (home.1 - center.1) / length * across * 2. * size,
                goal.1 + (home.0 - center.0) / length * across * 2. * size,
            );
            let Ok(group) = UnitGroup::new([car.id]) else {
                continue;
            };
            let ready = car.abilities.iter().any(|a| {
                a.kind == AbilityKind::Breakthrough
                    && a.cooldown_left == 0
                    && a.remaining_uses.is_none_or(|n| n > 0)
                    && a.available_tick.is_none_or(|t| frame.tick() >= t)
                    && a.lockout_until_tick.is_none_or(|t| frame.tick() >= t)
            });
            let covered = cluster
                .iter()
                .any(|t| dist2(t.position, car.position) <= radius * radius);
            let enemy = frame.visible_enemies().iter().any(|e| {
                e.health.current > 0
                    && cluster
                        .iter()
                        .any(|t| dist2(e.position, t.position) <= (16. * size) * (16. * size))
            });
            if attacking
                && frame.tick() >= self.active_until
                && !observed_active
                && ready
                && covered
                && (enemy || expansion_push)
                && actions
                    .use_ability(&group, AbilityKind::Breakthrough, None, false)
                    .is_ok()
            {
                self.active_until = frame
                    .tick()
                    .saturating_add(u32::from(rts_rules::balance::BREAKTHROUGH_DURATION_TICKS));
                self.last_car = Some(car.id);
                continue;
            }
            if dist2(car.position, goal) <= (2. * size) * (2. * size) {
                continue;
            }
            if self.issued.get(&car.id).is_some_and(|(old, t)| {
                dist2(*old, goal) <= (2. * size) * (2. * size)
                    && (car.state == AiEntityState::Move || frame.tick().saturating_sub(*t) < 45)
            }) {
                continue;
            }
            let Some(from) = q.world_point(car.position.0, car.position.1) else {
                continue;
            };
            let mut route = None;
            for offset in [(0., 0.), (-2., 0.), (2., 0.), (0., -2.), (0., 2.)] {
                let p = (goal.0 + offset.0 * size, goal.1 + offset.1 * size);
                if let Some(to) = q.world_point(p.0, p.1) {
                    if let Some(path) = q.ground_route(from, to, EntityKind::CommandCar) {
                        route = Some(path);
                        break;
                    }
                }
            }
            let Some(route) = route else {
                continue;
            };
            let points = crate::sdk::ground_route_corners(
                &route.iter().map(|p| p.as_tuple()).collect::<Vec<_>>(),
            );
            let points = points.into_iter().skip(1).take(24).collect::<Vec<_>>();
            if let Some((first, rest)) = points.split_first() {
                if actions.move_route(&group, *first, rest).is_ok() {
                    self.issued.insert(car.id, (goal, frame.tick()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
