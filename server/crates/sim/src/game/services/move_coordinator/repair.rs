//! Repair routes: outside building footprints, or following the current position of mobile targets.
use super::*;

pub(super) fn needs_path(worker: &Entity, entities: &EntityStore) -> bool {
    let Order::Repair(order) = worker.order() else {
        return false;
    };
    let Some(target) = entities.get(order.intent.target) else {
        return false;
    };
    let range = interact_range_for_kind(target.kind);
    let out_of_range = super::super::dist2(
        worker.pos_x(),
        worker.pos_y(),
        target.pos_x(),
        target.pos_y(),
    ) > range * range;
    out_of_range && (!target.is_building() || worker.path_is_empty())
}

impl MoveCoordinator<'_> {
    pub fn order_repair(&mut self, entities: &mut EntityStore, id: u32, target: u32) -> bool {
        let valid = entities.get(id).is_some_and(|w| {
            entities
                .get(target)
                .is_some_and(|t| t.can_be_repaired_by(w.owner))
        });
        if !valid {
            return false;
        }
        entities.release_miner(id);
        if let Some(w) = entities.get_mut(id) {
            w.replace_active_order(Order::repair(target));
            w.set_target_id(Some(target));
            w.reset_gather_state();
            w.set_path_goal(None);
            let (x, y) = (w.pos_x(), w.pos_y());
            w.reset_stuck(x, y);
        }
        self.request_repair_path(entities, id, target);
        entities
            .get(id)
            .is_some_and(|w| matches!(w.order(), Order::Repair(_)))
    }

    pub(super) fn request_repair_path(&mut self, entities: &mut EntityStore, id: u32, target: u32) {
        let Some(t) = entities.get(target) else {
            return;
        };
        let kind = t.kind;
        let goal = (t.pos_x(), t.pos_y());
        if let Some(stats) = config::building_stats(kind) {
            let (cx, cy) = self.map.tile_of(goal.0, goal.1);
            let (Some(tx), Some(ty)) = (
                cx.checked_sub(stats.foot_w / 2),
                cy.checked_sub(stats.foot_h / 2),
            ) else {
                return;
            };
            if matches!(
                self.plan_footprint_interaction_path(
                    entities,
                    id,
                    kind,
                    tx,
                    ty,
                    PathingRequestSource::Build
                ),
                PathAttempt::Failed
            ) {
                if let Some(w) = entities.get_mut(id) {
                    w.clear_active_order();
                }
            }
            return;
        }
        // Repath moving targets through the normal throttled, budgeted coordinator. The repair
        // tick stops the worker at interaction range before it reaches the target's occupied body.
        let Some(w) = entities.get(id) else {
            return;
        };
        if !needs_path(w, entities)
            || (w.path_goal().is_some() && !self.can_repath(entities, id, goal))
        {
            return;
        }
        self.request_path(entities, id, goal, false, PathingRequestSource::Build);
    }
}
