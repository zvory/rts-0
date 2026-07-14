use crate::game::entity::{EntityKind, ProdItem, MAX_PRODUCTION_QUEUE};

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
}
