use crate::game::entity::{ResearchItem, MAX_PRODUCTION_QUEUE};

use super::Entity;

impl Entity {
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
