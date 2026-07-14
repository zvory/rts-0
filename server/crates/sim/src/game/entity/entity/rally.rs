use crate::game::entity::RallyIntent;

use super::Entity;

impl Entity {
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
