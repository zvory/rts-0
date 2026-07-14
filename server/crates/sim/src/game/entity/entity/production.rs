use crate::game::entity::{EntityKind, ProdItem, RallyIntent, ResearchItem, MAX_PRODUCTION_QUEUE};

use super::Entity;

impl Entity {
    pub fn prod_queue(&self) -> &[ProdItem] {
        self.production
            .as_ref()
            .map(|p| p.queue.as_slice())
            .unwrap_or(&[])
    }

    pub fn push_production(&mut self, item: ProdItem) -> bool {
        let Some(p) = self.production.as_mut() else {
            return false;
        };
        if p.queue.len() >= MAX_PRODUCTION_QUEUE {
            return false;
        }
        p.queue.push(item);
        true
    }

    pub(in crate::game) fn mark_front_production_paid(&mut self) -> bool {
        let Some(front) = self.production.as_mut().and_then(|p| p.queue.first_mut()) else {
            return false;
        };
        front.paid = true;
        true
    }

    pub(crate) fn repeat_production(&self) -> Option<EntityKind> {
        let production = self.production.as_ref()?;
        let count = production.repeat_units.len();
        if count == 0 {
            return None;
        }
        production
            .repeat_units
            .get(production.repeat_unit_cursor % count)
            .copied()
    }

    /// `Some(unit)` toggles that unit; `None` clears (false) or advances the repeat cursor (true).
    pub(crate) fn set_repeat_production(
        &mut self,
        unit: Option<EntityKind>,
        enabled: bool,
    ) -> bool {
        let Some(production) = self.production.as_mut() else {
            return false;
        };
        match unit {
            Some(unit) if enabled => {
                if !production.repeat_units.contains(&unit) {
                    production.repeat_units.push(unit);
                }
            }
            Some(unit) => {
                let count = production.repeat_units.len();
                if let Some(removed_index) = production
                    .repeat_units
                    .iter()
                    .position(|&current| current == unit)
                {
                    let cursor = production.repeat_unit_cursor % count;
                    production.repeat_units.remove(removed_index);
                    if production.repeat_units.is_empty() {
                        production.repeat_unit_cursor = 0;
                    } else if removed_index < cursor {
                        production.repeat_unit_cursor = cursor.saturating_sub(1);
                    } else {
                        production.repeat_unit_cursor = cursor % production.repeat_units.len();
                    }
                }
            }
            None if enabled => {
                let count = production.repeat_units.len();
                if count > 0 {
                    production.repeat_unit_cursor =
                        (production.repeat_unit_cursor % count + 1) % count;
                }
            }
            None => {
                production.repeat_units.clear();
                production.repeat_unit_cursor = 0;
            }
        }
        true
    }

    pub fn pop_last_production(&mut self) -> Option<ProdItem> {
        self.production.as_mut()?.queue.pop()
    }

    pub fn tick_front_production(&mut self) -> Option<EntityKind> {
        let front = self.production.as_mut()?.queue.first_mut()?;
        if !front.paid {
            return None;
        }
        if front.progress < front.total {
            front.progress = front.progress.saturating_add(1);
        }
        (front.progress >= front.total).then_some(front.unit)
    }

    pub fn remove_front_production(&mut self) -> Option<ProdItem> {
        let queue = &mut self.production.as_mut()?.queue;
        if queue.is_empty() {
            None
        } else {
            Some(queue.remove(0))
        }
    }

    pub fn set_front_production_progress(&mut self, progress: u32) -> bool {
        let Some(front) = self.production.as_mut().and_then(|p| p.queue.first_mut()) else {
            return false;
        };
        front.progress = progress.min(front.total);
        true
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

    /// Rally point for a unit-producing building, if one has been set.
    pub fn rally_point(&self) -> Option<(f32, f32)> {
        self.production
            .as_ref()
            .and_then(|p| p.rally_point)
            .map(|r| (r.point.x, r.point.y))
    }

    /// Set (or clear with `None`) this building's rally point. No-op on entities without a
    /// production component.
    pub fn set_rally_point(&mut self, rally: Option<RallyIntent>) {
        if let Some(p) = self.production.as_mut() {
            p.rally_point = rally;
        }
    }

    #[allow(dead_code)]
    pub fn rally_stages(&self) -> &[RallyIntent] {
        self.production
            .as_ref()
            .map(|p| p.rally_queue.as_slice())
            .unwrap_or(&[])
    }

    pub fn rally_plan(&self) -> Vec<RallyIntent> {
        let Some(p) = self.production.as_ref() else {
            return Vec::new();
        };
        p.rally_point
            .into_iter()
            .chain(p.rally_queue.iter().copied())
            .collect()
    }

    pub fn clear_rally_stages(&mut self) {
        if let Some(p) = self.production.as_mut() {
            p.rally_queue.clear();
        }
    }

    pub fn append_rally_stage(&mut self, rally: RallyIntent, max_stages: usize) -> bool {
        let Some(p) = self.production.as_mut() else {
            return false;
        };
        if p.rally_point.is_none() {
            p.rally_point = Some(rally);
            return true;
        }
        let total = 1usize.saturating_add(p.rally_queue.len());
        if total >= max_stages {
            return false;
        }
        p.rally_queue.push(rally);
        true
    }
}
