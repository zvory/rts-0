//! Recruit a persistent 150%-value response to contacts on Pressure's half of the map.
use super::{distance, Point};
use crate::sdk::{
    AiActions, AiCompletion, AiEntity, AiEntityState, AiFrame, AiRulebook, EntityKind, UnitGroup,
};
use std::collections::{BTreeMap, BTreeSet};

const MEMORY_TICKS: u32 = 30 * 30;

#[derive(Default)]
pub(super) struct PressureResponse {
    contacts: BTreeMap<u32, Contact>,
    responders: BTreeSet<u32>,
    issued: BTreeMap<u32, (u32, Point)>,
}
struct Contact {
    position: Point,
    value: u32,
    seen: u32,
}
#[derive(Default)]
pub(super) struct ResponsePlan {
    pub active: BTreeSet<u32>,
    pub released: BTreeSet<u32>,
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
    pub(super) fn step(
        &mut self,
        frame: &AiFrame,
        eligible: &BTreeSet<u32>,
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
                        seen: frame.tick(),
                    },
                );
            }
        }
        if self.contacts.is_empty() {
            self.issued.clear();
            return ResponsePlan {
                active: BTreeSet::new(),
                released: std::mem::take(&mut self.responders),
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
        let required = self
            .contacts
            .values()
            .map(|c| u64::from(c.value))
            .sum::<u64>()
            .saturating_mul(3)
            .div_ceil(2);
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
                    && mobile(unit.kind)
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
