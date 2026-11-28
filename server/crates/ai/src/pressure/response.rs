//! Persistent 150%-value response: Pressure's home half, or opt-in protected defensive roles.
use super::{distance, Point};
use crate::sdk::{
    AiActions, AiCompletion, AiEntity, AiEntityState, AiFrame, AiRulebook, EntityKind, UnitGroup,
};
use std::collections::{BTreeMap, BTreeSet};

mod cohesion;
mod defended;
pub(crate) use defended::DefendedArea;

const MEMORY_TICKS: u32 = 30 * 30;

#[derive(Default, Debug)]
pub(crate) struct PressureResponse {
    include_machine_gunners: bool,
    cohesive: bool,
    fighting: BTreeMap<u32, bool>,
    release_cleared_contacts: bool,
    defended: defended::DefendedResponse,
    contacts: BTreeMap<u32, Contact>,
    responders: BTreeSet<u32>,
    issued: BTreeMap<u32, (u32, Point)>,
}
#[derive(Debug)]
struct Contact {
    position: Point,
    value: u32,
    kind: EntityKind,
    seen: u32,
}
#[derive(Default)]
pub(crate) struct ResponsePlan {
    pub(crate) active: BTreeSet<u32>,
    pub(crate) released: BTreeSet<u32>,
}
fn mobile(kind: EntityKind) -> bool {
    matches!(
        kind,
        EntityKind::Rifleman | EntityKind::Panzerfaust | EntityKind::Tank
    )
}
fn value(book: AiRulebook, entity: &AiEntity) -> u32 {
    book.cost(entity.kind)
        .map_or(0, |cost| cost.steel.saturating_add(cost.oil))
}
// The perpendicular bisector of the public starting-base axis splits the map in half.
fn on_home_half(position: Point, home: Point, enemy: Point) -> bool {
    let mid = ((home.0 + enemy.0) * 0.5, (home.1 + enemy.1) * 0.5);
    (position.0 - mid.0) * (enemy.0 - home.0) + (position.1 - mid.1) * (enemy.1 - home.1) <= 0.0
}

impl PressureResponse {
    /// Read-only tactical ownership for support vehicles; staging/withdrawal is not an attack.
    pub(crate) fn attacking_ids(&self) -> BTreeSet<u32> {
        self.fighting
            .iter()
            .filter(|(id, fighting)| **fighting && self.responders.contains(id))
            .map(|(id, _)| *id)
            .collect()
    }

    pub(crate) fn with_cohesive_reserve() -> Self {
        Self {
            cohesive: true,
            ..Self::with_machine_gunners()
        }
    }
    /// Eco borrows the same persistent response policy with its defensive MGs eligible.
    /// Default remains Pressure's original Rifleman/Panzerfaust/Tank roster.
    pub(crate) fn with_machine_gunners() -> Self {
        Self {
            include_machine_gunners: true,
            release_cleared_contacts: true,
            ..Self::default()
        }
    }
    pub(crate) fn step(
        &mut self,
        frame: &AiFrame,
        eligible: &BTreeSet<u32>,
        actions: &mut AiActions,
    ) -> ResponsePlan {
        self.step_inner(frame, eligible, None, actions)
    }
    /// Opt into defended-sector relevance without changing Pressure's default half-map policy.
    pub(crate) fn step_defended(
        &mut self,
        frame: &AiFrame,
        eligible: &BTreeSet<u32>,
        areas: &[DefendedArea],
        actions: &mut AiActions,
    ) -> ResponsePlan {
        self.step_inner(frame, eligible, Some(areas), actions)
    }
    fn step_inner(
        &mut self,
        frame: &AiFrame,
        eligible: &BTreeSet<u32>,
        areas: Option<&[DefendedArea]>,
        actions: &mut AiActions,
    ) -> ResponsePlan {
        let Some(book) = AiRulebook::for_frame(frame) else {
            return ResponsePlan::default();
        };
        let size = frame.map().tile_size as f32;
        let base_position = |player: &crate::sdk::AiPlayer| {
            (
                (player.start_tile.0 as f32 + 0.5) * size,
                (player.start_tile.1 as f32 + 0.5) * size,
            )
        };
        let home = frame
            .players()
            .iter()
            .find(|player| player.id == frame.player_id())
            .map(base_position);
        let enemy = frame
            .players()
            .iter()
            .find(|player| player.team_id != frame.team_id() && player.is_alive)
            .map(base_position);
        let owned: BTreeSet<_> = frame.owned().iter().map(|unit| unit.id).collect();
        self.responders.retain(|id| owned.contains(id));
        // Construction clearance may replace the unit's order while we cannot command it.
        // Its next eligible decision must issue a fresh response, even if it is still moving.
        self.issued.retain(|id, _| eligible.contains(id));
        self.contacts
            .retain(|_, contact| frame.tick().saturating_sub(contact.seen) < MEMORY_TICKS);
        if let Some(areas) = areas {
            let contacts = self.defended.update(frame, book, areas);
            // A released/leashed-out attacker must not keep a stale movement goal through
            // the normal retry cooldown when another defensive incident remains active.
            if self.defended.changed_incident
                || self.contacts.keys().any(|id| !contacts.contains_key(id))
            {
                self.issued.clear();
            }
            self.contacts = contacts;
        } else {
            // Only sightings on our half start or refresh a response. Crossing the midpoint cannot
            // drag the response into the enemy base: retain the last in-bounds position for 30 s.
            // Read only this player's fog-filtered frame; unknown enemy positions stay unknown.
            if let (Some(home), Some(enemy_base)) = (home, enemy) {
                for enemy in frame.visible_enemies().iter().filter(|enemy| {
                    enemy.kind.is_unit()
                        && enemy.health.current > 0
                        && on_home_half(enemy.position, home, enemy_base)
                }) {
                    self.contacts.insert(
                        enemy.id,
                        Contact {
                            position: enemy.position,
                            value: value(book, enemy),
                            kind: enemy.kind,
                            seen: frame.tick(),
                        },
                    );
                }
            }
            if self.release_cleared_contacts {
                let q = crate::sdk::WorldQueries::new(frame);
                self.contacts.retain(|id, contact| {
                    if let Some(current) = frame
                        .visible_enemies()
                        .iter()
                        .find(|e| e.id == *id && e.health.current > 0)
                    {
                        return home.zip(enemy).is_none_or(|(home, enemy)| {
                            on_home_half(current.position, home, enemy)
                        });
                    }
                    !q.world_point(contact.position.0, contact.position.1)
                        .is_some_and(|point| q.ground_position_is_observable(point))
                });
            }
        }
        if self.contacts.is_empty() {
            self.issued.clear();
            self.fighting.clear();
            return ResponsePlan {
                active: BTreeSet::new(),
                released: std::mem::take(&mut self.responders),
            };
        }
        let required = self
            .contacts
            .values()
            .map(|c| u64::from(c.value))
            .sum::<u64>()
            .saturating_mul(3)
            .div_ceil(2);
        if self.cohesive {
            self.recruit_cohesive(frame, eligible, book, required);
        } else {
            let nearest = |position: Point| {
                self.contacts
                    .values()
                    .min_by(|a, b| {
                        distance(position, a.position).total_cmp(&distance(position, b.position))
                    })
                    .map(|contact| contact.position)
                    .unwrap_or(position)
            };
            let mut committed: u64 = frame
                .owned()
                .iter()
                .filter(|unit| self.responders.contains(&unit.id) && eligible.contains(&unit.id))
                .map(|unit| u64::from(value(book, unit)))
                .sum();
            let mut candidates: Vec<_> = frame
                .owned()
                .iter()
                .filter(|unit| {
                    eligible.contains(&unit.id)
                        && (mobile(unit.kind)
                            || self.include_machine_gunners
                                && unit.kind == EntityKind::MachineGunner)
                        && unit.completion == AiCompletion::Complete
                        && unit.health.current > 0
                        && !self.responders.contains(&unit.id)
                })
                .collect();
            candidates.sort_by(|a, b| {
                distance(a.position, nearest(a.position))
                    .total_cmp(&distance(b.position, nearest(b.position)))
                    .then(a.id.cmp(&b.id))
            });
            for unit in candidates {
                if committed >= required {
                    break;
                }
                self.responders.insert(unit.id);
                committed += u64::from(value(book, unit));
            }
        }
        if self.cohesive {
            self.order_cohesive(frame, eligible, book, actions);
            return ResponsePlan {
                active: self.responders.clone(),
                released: BTreeSet::new(),
            };
        }
        let nearest = |position: Point| {
            self.contacts
                .values()
                .min_by(|a, b| {
                    distance(position, a.position).total_cmp(&distance(position, b.position))
                })
                .map(|contact| contact.position)
                .unwrap_or(position)
        };
        self.issued.retain(|id, _| self.responders.contains(id));
        for unit in frame
            .owned()
            .iter()
            .filter(|unit| self.responders.contains(&unit.id) && eligible.contains(&unit.id))
        {
            let point = nearest(unit.position);
            let reissue = self.issued.get(&unit.id).is_none_or(|(tick, previous)| {
                frame.tick().saturating_sub(*tick) >= 90
                    && (distance(*previous, point) >= frame.map().tile_size as f32 * 2.0
                        || unit.state == AiEntityState::Idle)
            });
            if reissue {
                if let Ok(group) = UnitGroup::new([unit.id]) {
                    if actions.attack_move(&group, point.0, point.1, false).is_ok() {
                        self.issued.insert(unit.id, (frame.tick(), point));
                    }
                }
            }
        }
        ResponsePlan {
            active: self.responders.clone(),
            released: BTreeSet::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::on_home_half;

    #[test]
    fn home_half_uses_the_start_axis_in_both_orientations() {
        let home = (100.0, 200.0);
        let enemy = (900.0, 800.0);
        assert!(on_home_half(home, home, enemy));
        assert!(!on_home_half(enemy, home, enemy));
        assert!(on_home_half((500.0, 500.0), home, enemy));
        assert!(on_home_half((400.0, 600.0), home, enemy));
        assert!(!on_home_half((600.0, 400.0), home, enemy));
        assert!(on_home_half(enemy, enemy, home));
        assert!(!on_home_half(home, enemy, home));
    }
}
