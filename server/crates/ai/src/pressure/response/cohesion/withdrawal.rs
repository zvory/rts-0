//! One armor encounter owns its members through withdrawal, regrouping and re-entry.
use super::*;
#[derive(Debug)]
pub(crate) struct ArmorEncounter {
    anchor: Point,
    target: Point,
    pub(super) members: BTreeSet<u32>,
    withdrawing: bool,
    rally: Option<Point>,
    slots: BTreeMap<u32, Point>,
}
pub(super) struct ArmorPlan {
    pub(super) target: Point,
    pub(super) fight: bool,
    pub(super) slots: BTreeMap<u32, Point>,
}
impl PressureResponse {
    pub(in crate::pressure::response) fn retain_encounter(
        &mut self,
        frame: &AiFrame,
        eligible: &BTreeSet<u32>,
        book: AiRulebook,
    ) {
        let Some(encounter) = &mut self.armor_encounter else {
            return;
        };
        encounter.members.retain(|id| {
            frame
                .owned()
                .iter()
                .any(|e| e.id == *id && e.health.current > 0)
        });
        let nearby = self
            .threat_memory
            .iter()
            .filter(|(_, (_, enemy))| {
                distance(enemy.position, encounter.anchor) <= 15. * frame.map().tile_size as f32
            })
            .collect::<Vec<_>>();
        let regrouped = encounter
            .members
            .iter()
            .filter(|id| {
                frame
                    .owned()
                    .iter()
                    .find(|e| e.id == **id)
                    .is_some_and(|e| {
                        encounter.slots.get(id).is_some_and(|p| {
                            distance(e.position, *p) <= 2. * frame.map().tile_size as f32
                        })
                    })
            })
            .count()
            * 10
            >= encounter.members.len() * 9;
        if encounter.members.is_empty()
            || (nearby.is_empty() && (!encounter.withdrawing || regrouped))
        {
            self.armor_encounter = None;
            return;
        }
        // The encounter anchor bounds pursuit, while the attack destination follows
        // the latest legal sightings inside that bound. Do not attack a frozen old point.
        if let Some((_, (_, enemy))) = nearby.iter().max_by_key(|(id, (seen, candidate))| {
            let strength: u64 = nearby
                .iter()
                .filter(|(_, (_, other))| {
                    distance(candidate.position, other.position)
                        <= 15. * frame.map().tile_size as f32
                })
                .map(|(_, (_, other))| fighting_value(book, other).ceil() as u64)
                .sum();
            (
                candidate.kind == EntityKind::Tank,
                strength,
                *seen,
                std::cmp::Reverse(**id),
            )
        }) {
            encounter.target = enemy.position;
        }
        // Admission bubbles decide when an encounter starts, not when an unfinished
        // withdrawal ends. Changing targets or losing the original guard cannot hand
        // these Tanks straight back to the expansion controller.
        self.responders
            .extend(encounter.members.iter().filter(|id| eligible.contains(id)));
        for (&id, (seen, enemy)) in nearby {
            self.contacts.insert(
                id,
                Contact {
                    position: enemy.position,
                    value: fighting_value(book, enemy).ceil() as u32,
                    kind: enemy.kind,
                    seen: *seen,
                },
            );
        }
    }
    pub(in crate::pressure::response) fn has_withdrawal(&self) -> bool {
        self.armor_encounter
            .as_ref()
            .is_some_and(|group| group.withdrawing)
    }
    pub(super) fn armor_plan(
        &mut self,
        frame: &AiFrame,
        book: AiRulebook,
        known: &[&AiEntity],
        defenders: &[&AiEntity],
        rear: Point,
    ) -> Option<ArmorPlan> {
        let tanks = defenders
            .iter()
            .copied()
            .filter(|e| e.kind == EntityKind::Tank)
            .collect::<Vec<_>>();
        if tanks.is_empty() {
            // Wounded-unit micro can temporarily own every member. Keep the encounter,
            // but issue no competing orders until those surviving Tanks become eligible.
            return None;
        }
        let size = frame.map().tile_size as f32;
        let target = self
            .armor_encounter
            .as_ref()
            .map(|e| e.target)
            .or_else(|| self.armor_target(size))?;
        // Evaluate the main concentration, so a single forward straggler cannot flip
        // the entire group back into retreat after the other90% have regrouped.
        let front = tanks.iter().copied().max_by(|a, b| {
            let count = |seed: &AiEntity| {
                tanks
                    .iter()
                    .filter(|e| distance(e.position, seed.position) <= 10. * size)
                    .count()
            };
            count(a)
                .cmp(&count(b))
                .then(distance(b.position, target).total_cmp(&distance(a.position, target)))
                .then(b.id.cmp(&a.id))
        })?;
        let enemy_value: f64 = known
            .iter()
            .filter(|e| distance(e.position, target) <= 15. * size)
            .map(|e| fighting_value(book, e))
            .sum();
        let supporting_value = |point: Point| {
            defenders
                .iter()
                .filter(|e| {
                    distance(e.position, point)
                        <= if e.kind == EntityKind::Tank {
                            10. * size
                        } else {
                            15. * size
                        }
                })
                .map(|e| fighting_value(book, e))
                .sum::<f64>()
        };
        let compact = tanks
            .iter()
            .filter(|e| distance(e.position, front.position) <= 10. * size)
            .count()
            * 10
            >= tanks.len() * 9;
        let strong = supporting_value(front.position) >= enemy_value * 0.9;
        let encounter = self.armor_encounter.get_or_insert_with(|| ArmorEncounter {
            anchor: target,
            target,
            members: BTreeSet::new(),
            withdrawing: !compact || !strong,
            rally: None,
            slots: BTreeMap::new(),
        });
        encounter.members.extend(tanks.iter().map(|e| e.id));
        encounter
            .slots
            .retain(|id, _| encounter.members.contains(id));
        if !strong {
            encounter.withdrawing = true;
        }
        if encounter.withdrawing {
            let safe = |p: Point| {
                known.iter().all(|enemy| {
                    distance(p, enemy.position) >= weapon_reach(frame, enemy) + 2. * size
                })
            };
            let usable =
                |unit: &AiEntity, point: Point| checked_route(frame, unit, point, known).is_some();
            if encounter.rally.is_none_or(|p| !usable(front, p)) {
                encounter.rally = staging_point(frame, front, target, rear, known)
                    .or_else(|| usable(front, front.position).then_some(front.position));
                encounter.slots.clear();
            }
            if let Some(center) = encounter.rally {
                let mut offsets = (-8i32..=8)
                    .flat_map(|y| (-8i32..=8).map(move |x| (x, y)))
                    .collect::<Vec<_>>();
                offsets.sort_by_key(|(x, y)| (x * x + y * y, *y, *x));
                for tank in &tanks {
                    if encounter
                        .slots
                        .get(&tank.id)
                        .is_some_and(|p| usable(tank, *p))
                    {
                        continue;
                    }
                    encounter.slots.remove(&tank.id);
                    for (x, y) in &offsets {
                        let p = (
                            center.0 + *x as f32 * 2. * size,
                            center.1 + *y as f32 * 2. * size,
                        );
                        if !safe(p)
                            || encounter
                                .slots
                                .values()
                                .any(|other| distance(*other, p) < 2. * size - 0.01)
                        {
                            continue;
                        }
                        if usable(tank, p) {
                            encounter.slots.insert(tank.id, p);
                            break;
                        }
                    }
                }
                let arrived = tanks
                    .iter()
                    .filter(|e| {
                        encounter
                            .slots
                            .get(&e.id)
                            .is_some_and(|p| distance(e.position, *p) <= 2. * size)
                    })
                    .count();
                if arrived * 10 >= encounter.members.len() * 9
                    && supporting_value(center) >= enemy_value * 0.9
                {
                    encounter.withdrawing = false;
                }
            }
        }
        Some(ArmorPlan {
            target,
            fight: !encounter.withdrawing,
            slots: encounter.slots.clone(),
        })
    }
}

// Keep the checked route's turns, as Eco's reserve routing does. Sending only the
// destination would let the simulator independently choose an unchecked detour.
fn checked_route(
    frame: &AiFrame,
    unit: &AiEntity,
    point: Point,
    known: &[&AiEntity],
) -> Option<Vec<Point>> {
    let size = frame.map().tile_size as f32;
    let clearance = |p: Point| {
        known
            .iter()
            .map(|e| distance(p, e.position) - weapon_reach(frame, e) - 2. * size)
            .min_by(f32::total_cmp)
            .unwrap_or(f32::INFINITY)
    };
    if clearance(point) < 0. {
        return None;
    }
    let q = WorldQueries::new(frame);
    let route = q
        .world_point(unit.position.0, unit.position.1)
        .zip(q.world_point(point.0, point.1))
        .and_then(|(a, b)| q.ground_route(a, b, EntityKind::Tank))?;
    if !route
        .iter()
        .all(|p| clearance(p.as_tuple()) >= clearance(unit.position).min(0.) - size * 0.25)
    {
        return None;
    }
    Some(route.into_iter().map(|p| p.as_tuple()).collect())
}
pub(super) fn move_withdrawal(
    frame: &AiFrame,
    actions: &mut AiActions,
    group: &UnitGroup,
    unit: &AiEntity,
    point: Point,
    known: &[&AiEntity],
) -> bool {
    let Some(route) = checked_route(frame, unit, point, known) else {
        return false;
    };
    let waypoints = crate::sdk::ground_route_corners(&route)
        .into_iter()
        .skip(1)
        .take(24)
        .collect::<Vec<_>>();
    if let Some((first, rest)) = waypoints.split_first() {
        actions.move_route(group, *first, rest).is_ok()
    } else {
        actions.hold_position(group, false).is_ok()
    }
}
