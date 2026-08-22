use super::*;

impl MoveCoordinator<'_> {
    pub(super) fn request_same_tile_tree_path(
        &mut self,
        entities: &mut EntityStore,
        id: u32,
        goal: (f32, f32),
        source: PathingRequestSource,
    ) -> Option<bool> {
        let request_start = self.diagnostics.as_ref().map(|_| Instant::now());
        let entity = entities.get(id)?;
        let kind = entity.kind;
        let start = (entity.pos_x, entity.pos_y);
        let start_tile = self.map.tile_of(start.0, start.1);
        let goal_tile = self.map.tile_of(goal.0, goal.1);
        if start_tile != goal_tile
            || matches!(
                source,
                PathingRequestSource::Gather | PathingRequestSource::DirectAttack
            )
            || standability::unit_tree_segment_clear(self.occ, kind, start, goal)
            || standability::unit_static_segment_standable(self.map, self.occ, kind, start, goal)
        {
            return None;
        }

        let path = tree_detour_between(self.map, self.occ, kind, start, goal).map(|mut forward| {
            forward.push(goal);
            forward.reverse();
            forward
        });
        let path_ok = path.is_some();
        let policy = if (movement_body_class(kind) == MovementBodyClass::InfantryLike
            || uses_oriented_vehicle_body(kind))
            && matches!(
                source,
                PathingRequestSource::Move | PathingRequestSource::AttackMove
            ) {
            RoutePolicy::FastestTerrainTime
        } else {
            RoutePolicy::LegacyShape
        };
        if let Some(entity) = entities.get_mut(id) {
            entity.set_path_with_policy(path.unwrap_or_default(), policy);
            entity.set_last_repath_tick(self.tick);
            entity.set_path_goal(Some(goal));
            entity.mark_move_phase(if path_ok {
                MovePhase::Moving
            } else {
                MovePhase::PathFailed
            });
        }
        self.consume_request_budget(None);
        self.record_path_request(
            source,
            path_ok,
            true,
            None,
            request_start
                .map(|start| start.elapsed())
                .unwrap_or_default(),
        );
        Some(path_ok)
    }
}
