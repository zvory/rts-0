//! Jeff's wounded-rifleman pull-back for his rifle squad planner.
//!
//! This is Jeff's own copy of the policy. Rush and Pressure use their own copy,
//! `crate::sdk::pullback`, which adds an SDK adapter: changes there do not reach Jeff, and changes
//! here do not reach them.
use std::collections::{BTreeMap, BTreeSet};

/// Distances are tiles; duration is simulation ticks. Zero HP disables new pull-backs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PullbackParams {
    pub(super) hp: u32,
    pub(super) tiles: f32,
    pub(super) ticks: u32,
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

/// Observation facts the policy needs: live squad members, and currently visible enemy units with
/// target IDs from the player's fog-filtered view.
#[derive(Clone, Copy, Debug)]
pub(super) struct PullbackUnit {
    pub(super) id: u32,
    pub(super) position: (f32, f32),
    pub(super) hp: u32,
    pub(super) target: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PullbackMove {
    pub(super) unit: u32,
    pub(super) position: (f32, f32),
}

/// Suppress ordinary orders for `active`; restore squad movement for `released`.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct PullbackPlan {
    pub(super) moves: Vec<PullbackMove>,
    pub(super) active: BTreeSet<u32>,
    pub(super) released: BTreeSet<u32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct PullbackMemory {
    until: BTreeMap<u32, u32>,
}

impl PullbackMemory {
    /// Forget dead, transferred or expired members, even when the caller skips combat planning.
    pub(super) fn retain(&mut self, members: &BTreeSet<u32>, tick: u32) {
        self.until
            .retain(|id, until| members.contains(id) && *until > tick);
    }

    /// Pure planning with explicit squad memory. Destinations are map-clamped, not checked for
    /// reachability or safety.
    pub(super) fn plan(
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
}

#[cfg(test)]
mod tests;
