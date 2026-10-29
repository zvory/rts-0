use crate::game::entity::{ResearchItem, MAX_PRODUCTION_QUEUE};

use super::Entity;

impl Entity {
    pub(in crate::game) fn complete_industrial_center(&mut self) {
        if self.hp == 0 || self.is_industrial_center() {
            return;
        }
        let Some(production) = self.production.as_mut() else {
            return;
        };
        production.industrial_center = true;
        let max_hp = self
            .max_hp
            .saturating_mul(crate::config::INDUSTRIAL_CENTER_HP_MULTIPLIER);
        let gained_hp = max_hp.saturating_sub(self.max_hp);
        self.max_hp = max_hp;
        self.hp = self.hp.saturating_add(gained_hp).min(max_hp);
    }

    pub(crate) fn is_industrial_center(&self) -> bool {
        self.kind
            == crate::rules::faction::upgrade_definition(
                crate::game::upgrade::UpgradeKind::IndustrialCenter,
            )
            .researched_at
            && self
                .production
                .as_ref()
                .is_some_and(|p| p.industrial_center)
    }

    pub(crate) fn industrial_center_slot_reserved(&self) -> bool {
        self.hp > 0
            && (self.is_industrial_center()
                || self.research_queue().iter().any(|item| {
                    item.upgrade == crate::game::upgrade::UpgradeKind::IndustrialCenter
                }))
    }

    pub(crate) fn research_queue(&self) -> &[ResearchItem] {
        self.production
            .as_ref()
            .map(|p| p.research_queue.as_slice())
            .unwrap_or(&[])
    }

    pub(crate) fn research_queue_mut(&mut self) -> Option<&mut Vec<ResearchItem>> {
        self.production.as_mut().map(|p| &mut p.research_queue)
    }

    pub(crate) fn push_research(&mut self, item: ResearchItem) -> bool {
        let Some(p) = self.production.as_mut() else {
            return false;
        };
        if p.research_queue.len() >= MAX_PRODUCTION_QUEUE {
            return false;
        }
        p.research_queue.push(item);
        true
    }

    pub(in crate::game) fn mark_front_research_paid(&mut self) -> bool {
        let Some(front) = self
            .production
            .as_mut()
            .and_then(|p| p.research_queue.first_mut())
        else {
            return false;
        };
        front.paid = true;
        true
    }

    pub(crate) fn pop_last_research(&mut self) -> Option<ResearchItem> {
        self.production.as_mut()?.research_queue.pop()
    }
}
