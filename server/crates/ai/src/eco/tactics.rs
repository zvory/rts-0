use super::construction::dist2;
use crate::sdk::UnitGroup;
use crate::sdk::{AiActions, AiBaseSiteId, AiEntityState, AiFrame, DefensePlanner, EntityKind};
use rts_rules::math::GameplayMath as _;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub(super) struct DefenseAssignments {
    destinations: BTreeMap<u32, (f32, f32)>,
    held: BTreeSet<u32>,
}
impl DefenseAssignments {
    pub(super) fn step(
        &mut self,
        frame: &AiFrame,
        actions: &mut AiActions,
        planner: &mut DefensePlanner,
        sites: &[AiBaseSiteId],
    ) {
        let infantry: Vec<_> = frame
            .owned()
            .iter()
            .filter(|e| matches!(e.kind, EntityKind::MachineGunner | EntityKind::Panzerfaust))
            .collect();
        if infantry.is_empty() {
            self.destinations.clear();
            self.held.clear();
            return;
        }
        let plan = planner.plan(frame, sites, infantry.len());
        let mut available: BTreeSet<_> = (0..plan.slots.len()).collect();
        let mut assigned = BTreeMap::new();
        // Preserve exact surviving posts before matching newly produced/repositioned infantry.
        for e in &infantry {
            if let Some(old) = self.destinations.get(&e.id) {
                if let Some(i) = available
                    .iter()
                    .copied()
                    .find(|i| dist2(plan.slots[*i].position, *old) < 1.)
                {
                    available.remove(&i);
                    assigned.insert(e.id, plan.slots[i].position);
                }
            }
        }
        for e in &infantry {
            if assigned.contains_key(&e.id) {
                continue;
            }
            if let Some(i) = available.iter().copied().min_by(|a, b| {
                dist2(e.position, plan.slots[*a].position)
                    .total_cmp(&dist2(e.position, plan.slots[*b].position))
            }) {
                available.remove(&i);
                assigned.insert(e.id, plan.slots[i].position);
            }
        }
        for e in infantry {
            let Some(point) = assigned.get(&e.id).copied() else {
                continue;
            };
            let changed = self
                .destinations
                .get(&e.id)
                .is_none_or(|p| dist2(*p, point) > 1.);
            let Ok(group) = UnitGroup::new([e.id]) else {
                continue;
            };
            // Leave nearby fights in place. Move commands, rather than chase orders, bring the
            // defenders back to legal posts after a fight or an expansion changes the plan.
            if e.state == AiEntityState::Attack {
                continue;
            }
            if dist2(e.position, point) > (frame.map().tile_size as f32 * 0.3).det_powi(2) {
                if (changed || e.state != AiEntityState::Move)
                    && actions.move_group(&group, point.0, point.1, false).is_ok()
                {
                    self.held.remove(&e.id);
                }
            } else if (changed || !self.held.contains(&e.id) || e.state != AiEntityState::Idle)
                && actions.hold_position(&group, false).is_ok()
            {
                self.held.insert(e.id);
            }
        }
        self.held.retain(|id| assigned.contains_key(id));
        self.destinations = assigned;
    }
}
