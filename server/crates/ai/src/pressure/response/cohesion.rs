//! Eco's opt-in concentration gate. Other profiles retain their original response orders.
use super::*;
use crate::sdk::WorldQueries;
use rts_rules::math::GameplayMath as _;

fn fighting_value(book: AiRulebook, unit: &AiEntity) -> f64 {
    if unit.health.current == 0 || unit.health.maximum == 0 {
        return 0.;
    }
    f64::from(value(book, unit)) * f64::from(unit.health.current) / f64::from(unit.health.maximum)
}
pub(super) fn weapon_reach(frame: &AiFrame, unit: &AiEntity) -> f32 {
    let base = rts_rules::combat::attack_profile(unit.kind).range_tiles;
    // The native stationary ramp reaches14tiles. Enemy ramp timers are private: use the
    // public maximum, not an inferred timer or the five-tile moving cannon range.
    let range = if unit.kind == EntityKind::Tank {
        base.max(14.)
    } else {
        base
    };
    let trench = if unit.occupied_trench_id.is_some() {
        rts_rules::balance::ENTRENCHMENT_RANGE_BONUS_TILES as f32
    } else {
        0.
    };
    (range.max(0.) + trench) * frame.map().tile_size as f32
}
impl PressureResponse {
    fn armor_target(&self, size: f32) -> Option<Point> {
        // Concentrate on one armored incident. Independently choosing every Tank's nearest
        // enemy would split the reserve between attacks and recreate piecemeal arrivals.
        self.contacts
            .iter()
            .filter(|(_, c)| c.kind == EntityKind::Tank)
            .max_by(|(aid, a), (bid, b)| {
                let cluster = |p| {
                    self.contacts
                        .values()
                        .filter(|c| {
                            c.kind == EntityKind::Tank && distance(c.position, p) <= 15. * size
                        })
                        .count()
                };
                cluster(a.position)
                    .cmp(&cluster(b.position))
                    .then(bid.cmp(aid))
            })
            .map(|(_, c)| c.position)
    }

    pub(super) fn recruit_cohesive(
        &mut self,
        frame: &AiFrame,
        eligible: &BTreeSet<u32>,
        book: AiRulebook,
        infantry_required: u64,
    ) {
        let tanks_required = self
            .contacts
            .values()
            .filter(|c| c.kind == EntityKind::Tank)
            .count()
            .saturating_mul(2);
        self.responders.retain(|id| {
            eligible.contains(id)
                && frame.owned().iter().any(|u| {
                    u.id == *id
                        && u.health.current > 0
                        && (u.kind != EntityKind::Tank || tanks_required > 0)
                })
        });
        let armor_target = self.armor_target(frame.map().tile_size as f32);
        for armor in [false, true] {
            let mut committed: u64 = frame
                .owned()
                .iter()
                .filter(|u| {
                    self.responders.contains(&u.id) && (u.kind == EntityKind::Tank) == armor
                })
                .map(|u| {
                    if armor {
                        1
                    } else {
                        fighting_value(book, u) as u64
                    }
                })
                .sum();
            let required = if armor {
                tanks_required as u64
            } else {
                infantry_required
            };
            let nearest = |p: Point| {
                if armor {
                    armor_target.map_or(f32::INFINITY, |target| distance(p, target))
                } else {
                    self.contacts
                        .values()
                        .filter(|c| !armor || c.kind == EntityKind::Tank)
                        .map(|c| distance(p, c.position))
                        .min_by(f32::total_cmp)
                        .unwrap_or(f32::INFINITY)
                }
            };
            let mut candidates: Vec<_> = frame
                .owned()
                .iter()
                .filter(|u| {
                    eligible.contains(&u.id)
                        && u.health.current > 0
                        && u.completion == AiCompletion::Complete
                        && (u.kind == EntityKind::Tank) == armor
                        && (mobile(u.kind) || u.kind == EntityKind::MachineGunner)
                        && !self.responders.contains(&u.id)
                })
                .collect();
            candidates.sort_by(|a, b| {
                nearest(a.position)
                    .total_cmp(&nearest(b.position))
                    .then(a.id.cmp(&b.id))
            });
            for unit in candidates {
                if committed >= required {
                    break;
                }
                self.responders.insert(unit.id);
                committed += if armor {
                    1
                } else {
                    fighting_value(book, unit) as u64
                };
            }
        }
    }
    pub(super) fn order_cohesive(
        &mut self,
        frame: &AiFrame,
        eligible: &BTreeSet<u32>,
        book: AiRulebook,
        actions: &mut AiActions,
    ) {
        let size = frame.map().tile_size as f32;
        let radius = 15. * size;
        let visible: Vec<_> = frame
            .visible_enemies()
            .iter()
            .filter(|e| {
                e.health.current > 0
                    && (e.kind.is_unit() || e.kind.is_building())
                    && weapon_reach(frame, e) > 0.
            })
            .collect();
        let defenders: Vec<_> = frame
            .owned()
            .iter()
            .filter(|e| {
                eligible.contains(&e.id) && e.health.current > 0 && self.responders.contains(&e.id)
            })
            .collect();
        let rear = frame
            .owned()
            .iter()
            .filter(|e| e.kind == EntityKind::ResourceDepot && e.health.current > 0)
            .fold((0., 0., 0.), |a, e| {
                (a.0 + e.position.0, a.1 + e.position.1, a.2 + 1.)
            });
        let rear = if rear.2 > 0. {
            (rear.0 / rear.2, rear.1 / rear.2)
        } else {
            defenders.first().map_or((0., 0.), |e| e.position)
        };
        self.issued.retain(|id, _| self.responders.contains(id));
        self.fighting.retain(|id, _| self.responders.contains(id));
        let armor: Vec<_> = defenders
            .iter()
            .filter(|e| e.kind == EntityKind::Tank)
            .map(|e| e.id)
            .collect();
        let columns = (armor.len() as f32).det_sqrt().ceil().max(1.) as usize;
        let queries = std::cell::LazyCell::new(|| WorldQueries::new(frame));
        // Placement reservations only: each unit still decides independently whether
        // to fight. Preserve valid old destinations before any fallback search runs.
        let mut retreat_posts: BTreeMap<u32, Point> = BTreeMap::new();
        for unit in defenders.iter().filter(|e| e.kind == EntityKind::Tank) {
            if self.fighting.get(&unit.id) != Some(&false) {
                continue;
            }
            if let Some((_, point)) = self.issued.get(&unit.id) {
                if retreat_posts
                    .values()
                    .all(|p| distance(*p, *point) >= 2. * size - 0.01)
                    && retreat_route(frame, &queries, unit, *point, &visible).is_some()
                {
                    retreat_posts.insert(unit.id, *point);
                }
            }
        }
        for unit in &defenders {
            let contact = self
                .contacts
                .values()
                .filter(|c| unit.kind != EntityKind::Tank || c.kind == EntityKind::Tank)
                .min_by(|a, b| {
                    distance(unit.position, a.position)
                        .total_cmp(&distance(unit.position, b.position))
                });
            let Some(contact) = contact else { continue };
            let target = if unit.kind == EntityKind::Tank {
                self.armor_target(size).unwrap_or(contact.position)
            } else {
                contact.position
            };
            // Count forces that can actually support this unit and this engagement, not distant
            // commitments. Use visible enemy reinforcements too, even outside admitted incidents.
            let enemies: Vec<_> = visible
                .iter()
                .filter(|e| distance(e.position, target) <= radius)
                .collect();
            let enemy_value: f64 = enemies.iter().map(|e| fighting_value(book, e)).sum();
            let enemy_value = if enemy_value > 0. {
                enemy_value
            } else {
                f64::from(contact.value)
            };
            // Tank support counts inside the compact ten-tile core; infantry support has
            // the usual fifteen-tile radius. Composition does not impose another veto:
            // infantry and armor together may provide the90% needed to fight.
            let friendly_value: f64 = defenders
                .iter()
                .filter(|e| {
                    distance(e.position, unit.position)
                        <= if e.kind == EntityKind::Tank {
                            10. * size
                        } else {
                            radius
                        }
                })
                .map(|e| fighting_value(book, e))
                .sum();
            let ready = friendly_value >= enemy_value * 0.9;
            let cached = self
                .issued
                .get(&unit.id)
                .filter(|(tick, p)| {
                    self.fighting.get(&unit.id) == Some(&false)
                        && (unit.kind == EntityKind::Tank
                            || frame.tick().saturating_sub(*tick) < 90)
                        && visible.iter().all(|e| {
                            distance(unit.position, e.position)
                                >= weapon_reach(frame, e) + 2. * size
                        })
                        && visible
                            .iter()
                            .all(|e| distance(*p, e.position) >= weapon_reach(frame, e) + 2. * size)
                        && (unit.kind != EntityKind::Tank || retreat_posts.get(&unit.id) == Some(p))
                })
                .map(|(_, p)| *p);
            let destination = if ready {
                Some(target)
            } else {
                let slot = armor
                    .iter()
                    .position(|id| *id == unit.id)
                    .map(|i| {
                        (
                            (i % columns) as f32 - (columns - 1) as f32 / 2.,
                            (i / columns) as f32,
                        )
                    })
                    .unwrap_or((0., 0.));
                let occupied = if unit.kind == EntityKind::Tank {
                    retreat_posts
                        .iter()
                        .filter(|(id, _)| **id != unit.id)
                        .map(|(_, p)| *p)
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                cached
                    .or_else(|| staging_point(frame, unit, target, rear, &visible, slot, &occupied))
            };
            let changed_mode = self.fighting.get(&unit.id) != Some(&ready);
            let Some(point) = destination else {
                retreat_posts.remove(&unit.id);
                if changed_mode || self.issued.remove(&unit.id).is_some() {
                    if let Ok(group) = UnitGroup::new([unit.id]) {
                        let _ = actions.hold_position(&group, false);
                    }
                }
                self.fighting.insert(unit.id, ready);
                continue;
            };
            if unit.kind == EntityKind::Tank {
                if ready {
                    retreat_posts.remove(&unit.id);
                } else {
                    retreat_posts.insert(unit.id, point);
                }
            }
            let unsafe_goal = !ready
                && self.issued.get(&unit.id).is_some_and(|(_, p)| {
                    visible
                        .iter()
                        .any(|e| distance(*p, e.position) < weapon_reach(frame, e) + 2. * size)
                });
            let reissue = changed_mode
                || unsafe_goal
                || self.issued.get(&unit.id).is_none_or(|(tick, old)| {
                    distance(*old, point) >= size
                        || frame.tick().saturating_sub(*tick) >= 30
                            && unit.state == AiEntityState::Idle
                });
            if reissue {
                if let Ok(group) = UnitGroup::new([unit.id]) {
                    let result = if ready {
                        actions.attack_move(&group, point.0, point.1, false)
                    } else if unit.kind == EntityKind::Tank {
                        if let Some(route) = retreat_route(frame, &queries, unit, point, &visible) {
                            let points = crate::sdk::ground_route_corners(&route)
                                .into_iter()
                                .skip(1)
                                .take(24)
                                .collect::<Vec<_>>();
                            if let Some((first, rest)) = points.split_first() {
                                actions.move_route(&group, *first, rest)
                            } else {
                                actions.hold_position(&group, false)
                            }
                        } else {
                            actions.hold_position(&group, false)
                        }
                    } else {
                        actions.move_group(&group, point.0, point.1, false)
                    };
                    if result.is_ok() {
                        self.issued.insert(unit.id, (frame.tick(), point));
                        self.fighting.insert(unit.id, ready);
                    }
                }
            }
        }
    }
}
// Route safety is current-visible geometry only. Preserve the checked turns in
// native movement; this does not retain a threat formation or a group encounter.
fn retreat_route(
    frame: &AiFrame,
    q: &WorldQueries<'_>,
    unit: &AiEntity,
    point: Point,
    enemies: &[&AiEntity],
) -> Option<Vec<Point>> {
    let size = frame.map().tile_size as f32;
    let clearance = |p: Point| {
        enemies
            .iter()
            .map(|e| distance(p, e.position) - weapon_reach(frame, e) - 2. * size)
            .min_by(f32::total_cmp)
            .unwrap_or(f32::INFINITY)
    };
    if clearance(point) < 0. {
        return None;
    }
    let route = q
        .world_point(unit.position.0, unit.position.1)
        .zip(q.world_point(point.0, point.1))
        .and_then(|(from, to)| q.ground_route(from, to, unit.kind))?;
    route
        .iter()
        .all(|p| clearance(p.as_tuple()) >= clearance(unit.position).min(0.) - size * 0.25)
        .then(|| route.into_iter().map(|p| p.as_tuple()).collect())
}
fn staging_point(
    frame: &AiFrame,
    unit: &AiEntity,
    target: Point,
    rear: Point,
    enemies: &[&AiEntity],
    slot: Point,
    reserved: &[Point],
) -> Option<Point> {
    let size = frame.map().tile_size as f32;
    let clearance = |p: Point| {
        enemies
            .iter()
            .map(|e| distance(p, e.position) - weapon_reach(frame, e) - 2. * size)
            .min_by(f32::total_cmp)
            .unwrap_or(f32::INFINITY)
    };
    let under_fire = clearance(unit.position) < 0.;
    let axis = if under_fire {
        (unit.position.0 - target.0, unit.position.1 - target.1)
    } else {
        (rear.0 - target.0, rear.1 - target.1)
    };
    let length = axis.0.det_hypot(axis.1);
    let away = if length > 1. {
        (axis.0 / length, axis.1 / length)
    } else {
        (0., -1.)
    };
    let reach = enemies
        .iter()
        .filter(|e| distance(e.position, target) <= 15. * size)
        .map(|e| distance(e.position, target) + weapon_reach(frame, e))
        .max_by(f32::total_cmp)
        .unwrap_or(4. * size);
    let q = WorldQueries::new(frame);
    let from = q.world_point(unit.position.0, unit.position.1)?;
    for depth in [3., 5., 8., 12.] {
        for side in [0., 2., -2., 4., -4.] {
            let r = reach + depth * size + slot.1.max(0.) * 2. * size;
            let side = side + slot.0 * 2.;
            let p = (
                target.0 + away.0 * r - away.1 * side * size,
                target.1 + away.1 * r + away.0 * side * size,
            );
            if clearance(p) < 0.
                || reserved
                    .iter()
                    .any(|other| distance(*other, p) < 2. * size - 0.01)
            {
                continue;
            }
            let Some(to) = q.world_point(p.0, p.1) else {
                continue;
            };
            let Some(route) = q.ground_route(from, to, unit.kind) else {
                continue;
            };
            // Never walk through an enemy firing envelope to reach the rally. When already under
            // fire, allow a retreat route that does not go deeper into the observed danger.
            if route
                .iter()
                .all(|p| clearance(p.as_tuple()) >= clearance(unit.position).min(0.) - size * 0.25)
            {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests;
