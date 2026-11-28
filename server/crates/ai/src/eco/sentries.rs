//! Keep observation infantry at every completed base before borrowing a response force.
use super::construction::dist2;
use crate::sdk::{
    AiActions, AiBaseSiteId, AiCompletion, AiEntityState, AiFrame, DefensePlan, DefensePlanner,
    PullbackParams, UnitGroup,
};
use rts_rules::math::GameplayMath as _;
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Sentries {
    planner: DefensePlanner,
    plan: DefensePlan,
    posts: BTreeMap<u32, (f32, f32)>,
    issued: BTreeMap<u32, (f32, f32)>,
    held: BTreeSet<u32>,
}
impl Default for Sentries {
    fn default() -> Self {
        Self {
            planner: DefensePlanner::with_garrison_posts_per_base(4),
            plan: DefensePlan::default(),
            posts: BTreeMap::new(),
            issued: BTreeMap::new(),
            held: BTreeSet::new(),
        }
    }
}
impl std::fmt::Debug for Sentries {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sentries")
            .field("posts", &self.posts)
            .finish()
    }
}
impl Sentries {
    pub(super) fn refresh(&mut self, frame: &AiFrame, sites: &[AiBaseSiteId]) -> BTreeSet<u32> {
        let mut candidates = frame
            .owned()
            .iter()
            .filter(|e| {
                super::tactics::infantry(e.kind)
                    && e.completion == AiCompletion::Complete
                    && e.health.current > PullbackParams::default().hp
            })
            .map(|e| (e.id, e))
            .collect::<BTreeMap<_, _>>();
        let plan = self.planner.plan(frame, sites, candidates.len());
        let mut assigned = BTreeMap::new();
        let mut pending = Vec::new();
        // Reserve surviving exact posts everywhere before filling any vacancies, so a
        // casualty at the main cannot steal another base's whole observation garrison.
        for slot in &plan.slots {
            if let Some(id) = self.posts.iter().find_map(|(id, p)| {
                (candidates.contains_key(id) && dist2(*p, slot.position) < 1.).then_some(*id)
            }) {
                candidates.remove(&id);
                assigned.insert(id, slot.position);
            } else {
                pending.push(slot.position);
            }
        }
        for point in pending {
            let Some(id) = candidates
                .values()
                .min_by(|a, b| {
                    dist2(a.position, point)
                        .total_cmp(&dist2(b.position, point))
                        .then(a.id.cmp(&b.id))
                })
                .map(|e| e.id)
            else {
                break;
            };
            candidates.remove(&id);
            assigned.insert(id, point);
        }
        self.issued.retain(|id, _| assigned.contains_key(id));
        self.held.retain(|id| assigned.contains_key(id));
        self.plan = plan;
        self.posts = assigned;
        self.posts.keys().copied().collect()
    }
    // Observation guards get orders before the combat response can consume the
    // action budget. Plain movement and Hold keep them from chasing a passing raid.
    pub(super) fn step(
        &mut self,
        frame: &AiFrame,
        actions: &mut AiActions,
        unavailable: &BTreeSet<u32>,
    ) {
        let tolerance = (frame.map().tile_size as f32 * 0.3).det_powi(2);
        for unit in frame
            .owned()
            .iter()
            .filter(|e| self.posts.contains_key(&e.id) && !unavailable.contains(&e.id))
        {
            let Some(point) = self.posts.get(&unit.id).copied() else {
                continue;
            };
            let Ok(group) = UnitGroup::new([unit.id]) else {
                continue;
            };
            if dist2(unit.position, point) > tolerance {
                if (self
                    .issued
                    .get(&unit.id)
                    .is_none_or(|p| dist2(*p, point) > 1.)
                    || unit.state != AiEntityState::Move)
                    && actions.move_group(&group, point.0, point.1, false).is_ok()
                {
                    self.issued.insert(unit.id, point);
                    self.held.remove(&unit.id);
                }
            } else if unit.state != AiEntityState::Attack
                && (!self.held.contains(&unit.id) || unit.state != AiEntityState::Idle)
                && actions.hold_position(&group, false).is_ok()
            {
                self.held.insert(unit.id);
            }
        }
    }
    pub(super) fn plan(&self) -> &DefensePlan {
        &self.plan
    }
    pub(super) fn posts(&self) -> &BTreeMap<u32, (f32, f32)> {
        &self.posts
    }
}
