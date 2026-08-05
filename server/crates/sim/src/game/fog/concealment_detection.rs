use crate::config;
use crate::game::entity::EntityStore;
use crate::game::map::Map;
use crate::game::services::spatial::SpatialIndex;
use crate::game::smoke::SmokeCloudStore;
use crate::game::teams::TeamRelations;
use crate::rules::terrain::{
    CONCEALMENT_CLOSE_DETECTION_RANGE_TILES, CONCEALMENT_DETECTION_PERSIST_TICKS,
};

use super::{stamp_point, Fog};

impl Fog {
    /// Refresh entity-level close detection and stamp only each detected target's occupied tile.
    /// Detection belongs to the spotting unit's whole team and persists briefly after separation.
    pub(in crate::game) fn refresh_concealment_detection_for_teams(
        &mut self,
        tick: u32,
        entities: &EntityStore,
        map: &Map,
        smokes: &SmokeCloudStore,
        teams: &TeamRelations,
        spatial: &SpatialIndex,
    ) {
        self.concealment_detection_until.retain(|_, by_entity| {
            by_entity.retain(|entity_id, expires_at| {
                *expires_at > tick
                    && entities
                        .get(*entity_id)
                        .is_some_and(|entity| entity.hp > 0 && entity.is_unit())
            });
            !by_entity.is_empty()
        });

        let max_unit_radius = entities
            .iter()
            .filter(|entity| entity.hp > 0 && entity.is_unit())
            .map(|entity| entity.radius())
            .filter(|radius| radius.is_finite())
            .fold(0.0_f32, f32::max);
        let close_gap_px = CONCEALMENT_CLOSE_DETECTION_RANGE_TILES * config::TILE_SIZE as f32;

        for target in entities.iter().filter(|entity| {
            entity.owner != 0
                && entity.hp > 0
                && entity.is_unit()
                && map.world_point_is_concealed(entity.pos_x, entity.pos_y)
                && !smokes.point_inside(entity.pos_x, entity.pos_y)
        }) {
            let query_radius = target.radius() + max_unit_radius + close_gap_px;
            for spotter_id in spatial.ids_in_circle_bbox(target.pos_x, target.pos_y, query_radius) {
                let Some(spotter) = entities.get(spotter_id) else {
                    continue;
                };
                if spotter.hp == 0
                    || !spotter.is_unit()
                    || !teams.is_enemy_owner(spotter.owner, target.owner)
                    || smokes.point_inside(spotter.pos_x, spotter.pos_y)
                {
                    continue;
                }
                let dx = target.pos_x - spotter.pos_x;
                let dy = target.pos_y - spotter.pos_y;
                let range = target.radius() + spotter.radius() + close_gap_px;
                if !dx.is_finite()
                    || !dy.is_finite()
                    || !range.is_finite()
                    || dx * dx + dy * dy > range * range
                {
                    continue;
                }
                let expires_at = tick.saturating_add(CONCEALMENT_DETECTION_PERSIST_TICKS);
                let mut recipients = teams.same_team_player_ids(spotter.owner);
                if recipients.is_empty() {
                    recipients.push(spotter.owner);
                }
                for viewer in recipients {
                    self.concealment_detection_until
                        .entry(viewer)
                        .or_default()
                        .entry(target.id)
                        .and_modify(|expiry| *expiry = (*expiry).max(expires_at))
                        .or_insert(expires_at);
                }
            }
        }

        let width = self.width;
        let height = self.height;
        for (&viewer, by_entity) in &self.concealment_detection_until {
            let Some(grid) = self.grids.get_mut(&viewer) else {
                continue;
            };
            for entity_id in by_entity.keys() {
                let Some(entity) = entities.get(*entity_id) else {
                    continue;
                };
                if entity.hp == 0
                    || !entity.is_unit()
                    || !map.world_point_is_concealed(entity.pos_x, entity.pos_y)
                    || smokes.point_inside(entity.pos_x, entity.pos_y)
                {
                    continue;
                }
                stamp_point(grid, width, height, entity.pos_x, entity.pos_y);
            }
        }
    }

    pub(crate) fn has_concealment_detection(&self, viewer: u32, entity_id: u32) -> bool {
        self.concealment_detection_until
            .get(&viewer)
            .is_some_and(|by_entity| by_entity.contains_key(&entity_id))
    }
}
