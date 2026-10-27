//! Optional wounded-unit movement policy. Callers retain squad, route and combat ownership.
use std::collections::{BTreeMap, BTreeSet};

use super::{AiActions, AiCompletion, AiEntity, AiEntityState, AiFrame, UnitGroup};

/// Distances are tiles; duration is simulation ticks. Zero HP disables new pull-backs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PullbackParams {
    pub hp: u32,
    pub tiles: f32,
    pub ticks: u32,
}

impl Default for PullbackParams {
    fn default() -> Self {
        Self {
            hp: 15,
            tiles: 3.0,
            ticks: 45,
        }
    }
}

/// Public observation facts needed by the policy. Supply only live members of the caller's squad
/// and currently visible enemy units. Target IDs must come from the player's fog-filtered view.
#[derive(Clone, Copy, Debug)]
pub struct PullbackUnit {
    pub id: u32,
    pub position: (f32, f32),
    pub hp: u32,
    pub target: Option<u32>,
}

impl From<&AiEntity> for PullbackUnit {
    fn from(entity: &AiEntity) -> Self {
        Self {
            id: entity.id,
            position: entity.position,
            hp: entity.health.current,
            target: entity.target,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PullbackMove {
    pub unit: u32,
    pub position: (f32, f32),
}

/// Suppress ordinary orders for `active`; restore caller-owned movement for `released`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PullbackPlan {
    pub moves: Vec<PullbackMove>,
    pub active: BTreeSet<u32>,
    pub released: BTreeSet<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PullbackMemory {
    until: BTreeMap<u32, u32>,
}

impl PullbackMemory {
    /// Forget dead, transferred or expired members, even when the caller skips combat planning.
    pub fn retain(&mut self, members: &BTreeSet<u32>, tick: u32) {
        self.until
            .retain(|id, until| members.contains(id) && *until > tick);
    }

    /// Pure planning with explicit caller-owned memory. Destinations are map-clamped, not checked
    /// for reachability or safety. Callers emitting orders themselves must handle emission failure.
    /// Enemy selection and arithmetic preserve Jeff's original rifle squad policy.
    pub fn plan(
        &mut self,
        tick: u32,
        map: (u32, u32, u32),
        units: &[PullbackUnit],
        enemies: &[PullbackUnit],
        params: PullbackParams,
    ) -> PullbackPlan {
        let prior: BTreeSet<u32> = self.until.keys().copied().collect();
        let members = units.iter().map(|unit| unit.id).collect();
        self.retain(&members, tick);
        let mut result = PullbackPlan::default();
        if params.hp == 0 || !units.iter().any(|unit| unit.hp > params.hp) {
            self.until.clear();
        } else {
            let tile = map.2.max(1) as f32;
            let map_w = map.0 as f32 * tile;
            let map_h = map.1 as f32 * tile;
            for unit in units {
                if self.until.contains_key(&unit.id) || unit.hp > params.hp {
                    continue;
                }
                if !enemies.iter().any(|enemy| enemy.target == Some(unit.id)) {
                    continue;
                }
                let distance = |enemy: &PullbackUnit| {
                    (unit.position.0 - enemy.position.0).hypot(unit.position.1 - enemy.position.1)
                };
                let Some(threat) = enemies
                    .iter()
                    .min_by(|a, b| distance(a).total_cmp(&distance(b)).then(a.id.cmp(&b.id)))
                else {
                    continue;
                };
                let (mut dx, mut dy) = (
                    unit.position.0 - threat.position.0,
                    unit.position.1 - threat.position.1,
                );
                let len = dx.hypot(dy);
                if len <= f32::EPSILON {
                    (dx, dy) = (1.0, 0.0);
                } else {
                    (dx, dy) = (dx / len, dy / len);
                }
                let step = params.tiles.max(0.0) * tile;
                let x = (unit.position.0 + dx * step)
                    .clamp(tile * 0.5, (map_w - tile * 0.5).max(tile * 0.5));
                let y = (unit.position.1 + dy * step)
                    .clamp(tile * 0.5, (map_h - tile * 0.5).max(tile * 0.5));
                result.moves.push(PullbackMove {
                    unit: unit.id,
                    position: (x, y),
                });
                self.until
                    .insert(unit.id, tick.saturating_add(params.ticks));
            }
        }
        result.active = self.until.keys().copied().collect();
        result.released = prior.difference(&result.active).copied().collect();
        result
    }

    /// SDK adapter: select live owned squad members and visible enemy units, emit only new Moves,
    /// and remember a new retreat only when local action emission succeeds. Existing active
    /// members still require caller-side exclusion; action reservations last only one step.
    pub fn step(
        &mut self,
        frame: &AiFrame,
        members: &[u32],
        params: PullbackParams,
        actions: &mut AiActions,
    ) -> PullbackPlan {
        let ids: BTreeSet<_> = members.iter().copied().collect();
        let live = |entity: &&AiEntity| {
            entity.health.current > 0
                && entity.state != AiEntityState::Dead
                && entity.completion == AiCompletion::Complete
        };
        let units: Vec<_> = frame
            .owned()
            .iter()
            .filter(live)
            .filter(|entity| ids.contains(&entity.id))
            .map(PullbackUnit::from)
            .collect();
        let enemies: Vec<_> = frame
            .visible_enemies()
            .iter()
            .filter(live)
            .filter(|entity| entity.kind.is_unit())
            .map(PullbackUnit::from)
            .collect();
        let prior: BTreeSet<_> = self.until.keys().copied().collect();
        let map = frame.map();
        let mut plan = self.plan(
            frame.tick(),
            (map.width, map.height, map.tile_size),
            &units,
            &enemies,
            params,
        );
        plan.moves.retain(|movement| {
            let emitted = UnitGroup::new([movement.unit])
                .and_then(|group| {
                    actions.move_group(&group, movement.position.0, movement.position.1, false)
                })
                .is_ok();
            if !emitted {
                self.until.remove(&movement.unit);
                plan.active.remove(&movement.unit);
            }
            emitted
        });
        plan.released
            .extend(prior.difference(&plan.active).copied());
        plan
    }
}

#[cfg(test)]
mod tests;
