use std::collections::{BTreeMap, BTreeSet};

use crate::config;
use crate::game::entity::EntityKind;
use crate::game::fog::Fog;
use crate::game::map::Map;
use crate::protocol::{self, GroundDecalView, TankTrailView};
use crate::rules::{artillery_ground_decal_source_kind, mortar_ground_decal_source_kind};
use serde::{Deserialize, Serialize};

mod revision_log;
mod spatial_index;
mod tank_trail;
mod types;

use revision_log::{GroundDecalRevisionEntry, GroundDecalRevisionLog};
use spatial_index::GroundDecalSpatialIndex;
use tank_trail::TankTrailStore;
use types::{
    deserialize_source_kind, serialize_source_kind, valid_class_and_source, GroundDecalClass,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GroundDecal {
    id: u32,
    decal_class: GroundDecalClass,
    #[serde(
        serialize_with = "serialize_source_kind",
        deserialize_with = "deserialize_source_kind"
    )]
    source_kind: EntityKind,
    x: f32,
    y: f32,
    owner: u32,
    seed: u32,
    facing: Option<f32>,
    weapon_facing: Option<f32>,
    radius_tiles: Option<f32>,
    created_revision: u32,
}

impl GroundDecal {
    fn to_view(&self) -> GroundDecalView {
        GroundDecalView {
            id: self.id,
            decal_class: self.decal_class.wire_name().to_string(),
            source_kind: protocol::kind_to_wire(self.source_kind).to_string(),
            x: self.x,
            y: self.y,
            owner: self.owner,
            seed: self.seed,
            facing: self.facing,
            weapon_facing: self.weapon_facing,
            radius_tiles: self.radius_tiles,
        }
    }
}

/// Append-only authoritative ground marks plus per-player first-discovery cursors.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GroundDecalStore {
    next_id: u32,
    revision: u32,
    #[serde(default)]
    current_tick: u32,
    decals: Vec<GroundDecal>,
    discovered_by_player: BTreeMap<u32, BTreeMap<u32, u32>>,
    #[serde(default)]
    tank_trails: TankTrailStore,
    #[serde(default)]
    discovered_trails_by_player: BTreeMap<u32, BTreeMap<u32, u32>>,
    revision_log: GroundDecalRevisionLog,
    #[serde(skip)]
    spatial_index: GroundDecalSpatialIndex,
}

impl Default for GroundDecalStore {
    fn default() -> Self {
        Self::new()
    }
}

impl GroundDecalStore {
    pub(crate) fn new() -> Self {
        Self {
            next_id: 1,
            revision: 0,
            current_tick: 0,
            decals: Vec::new(),
            discovered_by_player: BTreeMap::new(),
            tank_trails: TankTrailStore::new(),
            discovered_trails_by_player: BTreeMap::new(),
            revision_log: GroundDecalRevisionLog::default(),
            spatial_index: GroundDecalSpatialIndex::new(),
        }
    }

    pub(crate) fn begin_tick(&mut self, tick: u32) {
        self.current_tick = tick;
    }

    pub(crate) fn update_tank_trails(
        &mut self,
        entities: &crate::game::entity::EntityStore,
        map: &Map,
        tick: u32,
    ) {
        for pending in self.tank_trails.update(entities, map, tick) {
            let Some(id) = self.tank_trails.next_id() else {
                return;
            };
            let Some(revision) = self.record_revision(GroundDecalRevisionEntry::TrailCreated {
                id,
                tick: self.current_tick,
            }) else {
                return;
            };
            if !self.tank_trails.commit(pending, id, revision, map) {
                return;
            }
        }
    }

    pub(crate) fn create_death(
        &mut self,
        kind: EntityKind,
        x: f32,
        y: f32,
        owner: u32,
        facing: Option<f32>,
        weapon_facing: Option<f32>,
    ) -> Option<u32> {
        let decal_class = GroundDecalClass::from_death_kind(kind)?;
        self.create(decal_class, kind, x, y, owner, facing, weapon_facing, None)
    }

    pub(crate) fn create_mortar_impact(&mut self, map: &Map, x: f32, y: f32) -> Option<u32> {
        let radius_tiles = config::MORTAR_OUTER_RADIUS_TILES;
        if !blast_intersects_map(map, x, y, radius_tiles) {
            return None;
        }
        self.create(
            GroundDecalClass::MortarBlast,
            mortar_ground_decal_source_kind(),
            x,
            y,
            0,
            None,
            None,
            Some(radius_tiles),
        )
    }

    pub(crate) fn create_artillery_impact(&mut self, map: &Map, x: f32, y: f32) -> Option<u32> {
        let radius_tiles = config::ARTILLERY_OUTER_RADIUS_TILES;
        if !blast_intersects_map(map, x, y, radius_tiles) {
            return None;
        }
        self.create(
            GroundDecalClass::ArtilleryBlast,
            artillery_ground_decal_source_kind(),
            x,
            y,
            0,
            None,
            None,
            Some(radius_tiles),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create(
        &mut self,
        decal_class: GroundDecalClass,
        source_kind: EntityKind,
        x: f32,
        y: f32,
        owner: u32,
        facing: Option<f32>,
        weapon_facing: Option<f32>,
        radius_tiles: Option<f32>,
    ) -> Option<u32> {
        if self.next_id == 0
            || !x.is_finite()
            || !y.is_finite()
            || facing.is_some_and(|value| !value.is_finite())
            || weapon_facing.is_some_and(|value| !value.is_finite())
            || radius_tiles.is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return None;
        }
        self.spatial_index.ensure(&self.decals);
        let id = self.next_id;
        let created_revision = self.record_revision(GroundDecalRevisionEntry::Created {
            id,
            tick: self.current_tick,
        })?;
        self.next_id = self.next_id.checked_add(1).unwrap_or(0);
        self.decals.push(GroundDecal {
            id,
            decal_class,
            source_kind,
            x,
            y,
            owner,
            seed: decal_seed(id, x, y),
            facing,
            weapon_facing,
            radius_tiles,
            created_revision,
        });
        if let Some(decal) = self.decals.last() {
            self.spatial_index.add(decal);
        }
        Some(id)
    }

    pub(crate) fn refresh_memory_for_player(&mut self, player: u32, fog: &Fog, map: &Map) {
        self.spatial_index.ensure(&self.decals);
        let mut newly_visible = BTreeSet::new();
        if let Some((width, visible)) = fog.visible_grid_for(player) {
            for (index, is_visible) in visible.iter().copied().enumerate() {
                if !is_visible {
                    continue;
                }
                let Ok(index) = u32::try_from(index) else {
                    continue;
                };
                let tx = index % width;
                let ty = index / width;
                let Some(candidates) = self.spatial_index.candidates(tx, ty) else {
                    continue;
                };
                for &id in candidates {
                    if self
                        .discovered_by_player
                        .get(&player)
                        .is_some_and(|known| known.contains_key(&id))
                    {
                        continue;
                    }
                    let Some(decal) = decal_for_id(&self.decals, id) else {
                        continue;
                    };
                    if decal_visible_to_player(decal, player, fog) {
                        newly_visible.insert(id);
                    }
                }
            }
        }
        for id in newly_visible {
            let Some(revision) = self.record_revision(GroundDecalRevisionEntry::Discovered {
                player,
                id,
                tick: self.current_tick,
            }) else {
                return;
            };
            self.discovered_by_player
                .entry(player)
                .or_default()
                .insert(id, revision);
        }

        let known_trails = self
            .discovered_trails_by_player
            .get(&player)
            .cloned()
            .unwrap_or_default();
        let newly_visible_trails =
            self.tank_trails
                .newly_fully_visible(player, fog, map, &known_trails);
        for id in newly_visible_trails {
            let Some(revision) = self.record_revision(GroundDecalRevisionEntry::TrailDiscovered {
                player,
                id,
                tick: self.current_tick,
            }) else {
                return;
            };
            self.discovered_trails_by_player
                .entry(player)
                .or_default()
                .insert(id, revision);
        }
    }

    pub(in crate::game) fn valid_checkpoint_state(
        &self,
        map: &crate::game::map::Map,
        player_ids: &BTreeSet<u32>,
        tick: u32,
    ) -> bool {
        if self.next_id == 0 || self.current_tick != tick {
            return false;
        }
        let mut ids = BTreeSet::new();
        let mut used_revisions = BTreeSet::new();
        for (index, decal) in self.decals.iter().enumerate() {
            let expected_id = u32::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1));
            let presentation_valid = decal.id != 0
                && Some(decal.id) == expected_id
                && ids.insert(decal.id)
                && decal.created_revision != 0
                && decal.created_revision <= self.revision
                && used_revisions.insert(decal.created_revision)
                && decal_position_valid(decal, map)
                && decal.facing.is_none_or(f32::is_finite)
                && decal.weapon_facing.is_none_or(f32::is_finite)
                && valid_class_and_source(decal)
                && (decal.owner == 0 || player_ids.contains(&decal.owner));
            if !presentation_valid {
                return false;
            }
        }
        let expected_next = self
            .decals
            .iter()
            .map(|decal| decal.id)
            .max()
            .unwrap_or(0)
            .checked_add(1);
        if expected_next != Some(self.next_id) {
            return false;
        }
        for (player, known) in &self.discovered_by_player {
            if !player_ids.contains(player) {
                return false;
            }
            for (id, revision) in known {
                if !ids.contains(id)
                    || *revision == 0
                    || *revision > self.revision
                    || !used_revisions.insert(*revision)
                {
                    return false;
                }
            }
        }
        if !self
            .tank_trails
            .valid_checkpoint_state(map, player_ids, tick)
        {
            return false;
        }
        for revision in self.tank_trails.created_revisions() {
            if revision > self.revision || !used_revisions.insert(revision) {
                return false;
            }
        }
        for (player, known) in &self.discovered_trails_by_player {
            if !player_ids.contains(player) {
                return false;
            }
            for (id, revision) in known {
                if !self.tank_trails.contains(*id)
                    || *revision == 0
                    || *revision > self.revision
                    || !used_revisions.insert(*revision)
                {
                    return false;
                }
            }
        }
        self.revision_log.valid(
            self.revision,
            self.current_tick,
            &self.decals,
            &self.discovered_by_player,
            &self.tank_trails,
            &self.discovered_trails_by_player,
            used_revisions.len(),
        )
    }
}

fn decal_for_id(decals: &[GroundDecal], id: u32) -> Option<&GroundDecal> {
    let index = usize::try_from(id.checked_sub(1)?).ok()?;
    decals.get(index).filter(|decal| decal.id == id)
}

fn decal_position_valid(decal: &GroundDecal, map: &Map) -> bool {
    decal
        .radius_tiles
        .is_some_and(|radius_tiles| blast_intersects_map(map, decal.x, decal.y, radius_tiles))
        || (decal.radius_tiles.is_none() && map.contains_world_point(decal.x, decal.y))
}

fn blast_intersects_map(map: &Map, x: f32, y: f32, radius_tiles: f32) -> bool {
    if !x.is_finite() || !y.is_finite() || !radius_tiles.is_finite() || radius_tiles <= 0.0 {
        return false;
    }
    let closest_x = x.clamp(0.0, map.world_width_px());
    let closest_y = y.clamp(0.0, map.world_height_px());
    let dx = x - closest_x;
    let dy = y - closest_y;
    let radius = radius_tiles * config::TILE_SIZE as f32;
    dx * dx + dy * dy <= radius * radius
}

fn decal_seed(id: u32, x: f32, y: f32) -> u32 {
    let mut hash = 0x811c_9dc5u32;
    for value in [
        id,
        (x * 4.0).round() as i32 as u32,
        (y * 4.0).round() as i32 as u32,
    ] {
        hash ^= value;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

fn decal_visible_to_player(decal: &GroundDecal, player: u32, fog: &Fog) -> bool {
    if fog.is_visible_world(player, decal.x, decal.y) {
        return true;
    }
    let Some(radius_tiles) = decal.radius_tiles else {
        return false;
    };
    let tile_size = config::TILE_SIZE as f32;
    let radius_px = radius_tiles * tile_size;
    let tile_radius = radius_tiles.ceil() as i32;
    let center_tx = (decal.x / tile_size).floor() as i32;
    let center_ty = (decal.y / tile_size).floor() as i32;
    for dy in -tile_radius..=tile_radius {
        for dx in -tile_radius..=tile_radius {
            let tx = center_tx + dx;
            let ty = center_ty + dy;
            if tx < 0 || ty < 0 || !fog.is_visible(player, tx as u32, ty as u32) {
                continue;
            }
            let x = (tx as f32 + 0.5) * tile_size;
            let y = (ty as f32 + 0.5) * tile_size;
            let offset_x = x - decal.x;
            let offset_y = y - decal.y;
            if offset_x * offset_x + offset_y * offset_y <= radius_px * radius_px {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Game, ObserverView, PlayerInit};
    use crate::rules::death_ground_decal_class;

    fn one_player_game() -> Game {
        Game::new(
            &[PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "One".to_string(),
                color: "#fff".to_string(),
                is_ai: false,
            }],
            7,
        )
    }

    fn fog_with_visible_tile(player: u32, visible_index: Option<usize>) -> Fog {
        let mut grid = vec![false; 16];
        if let Some(index) = visible_index {
            grid[index] = true;
        }
        Fog::from_checkpoint_grids(
            4,
            4,
            BTreeMap::from([(player, grid)]),
            BTreeMap::new(),
            BTreeMap::new(),
        )
    }

    #[test]
    fn store_keeps_marks_beyond_the_old_beta_cap() {
        let mut store = GroundDecalStore::new();
        let map = one_player_game().state.map;
        for _ in 0..4_097 {
            assert!(store.create_mortar_impact(&map, 32.0, 32.0).is_some());
        }
        assert_eq!(store.decals.first().map(|decal| decal.id), Some(1));
        assert_eq!(store.decals.last().map(|decal| decal.id), Some(4_097));
    }

    #[test]
    fn stored_mark_has_no_heap_owned_vocabulary() {
        assert!(std::mem::size_of::<GroundDecal>() <= 64);
    }

    #[test]
    fn hidden_mark_is_sent_only_after_first_physical_discovery() {
        let mut store = GroundDecalStore::new();
        let map = one_player_game().state.map;
        store.create_mortar_impact(&map, 48.0, 48.0).unwrap();

        store.refresh_memory_for_player(2, &fog_with_visible_tile(2, None), &map);
        assert_eq!(
            store.views_for_players_after(&[2], 0),
            (0, Vec::new(), Vec::new())
        );
        assert_eq!(
            store.recent_views_for_players(&[2], 64),
            (0, 0, Vec::new(), Vec::new())
        );

        store.refresh_memory_for_player(2, &fog_with_visible_tile(2, Some(5)), &map);
        let (revision, decals, trails) = store.views_for_players_after(&[2], 0);
        assert!(revision > 0);
        assert_eq!(decals.len(), 1);
        assert!(trails.is_empty());
        assert_eq!(decals[0].decal_class, "mortarBlast");
        let (fast_revision, fast_after, fast_decals, fast_trails) =
            store.recent_views_for_players(&[2], 64);
        assert_eq!(fast_revision, revision);
        assert_eq!(fast_after, 0);
        assert_eq!(fast_decals, decals);
        assert!(fast_trails.is_empty());
        assert_eq!(
            decals[0].owner, 0,
            "impact decals must not leak firing owner"
        );
        assert_eq!(
            store.views_for_players_after(&[2], revision),
            (revision, Vec::new(), Vec::new())
        );

        store.refresh_memory_for_player(2, &fog_with_visible_tile(2, Some(5)), &map);
        assert_eq!(store.revision_for_players(&[2]), revision);
        store.begin_tick(1);
        assert_eq!(
            store.recent_views_for_players(&[2], 64),
            (fast_revision, fast_revision, Vec::new(), Vec::new()),
            "a later tick advertises the cursor without repeating old rows"
        );
    }

    #[test]
    fn current_tick_delta_cursor_skips_hidden_global_revision_gaps() {
        assert_eq!(
            revision_log::current_delta_after_for_test(50, &[10, 50], &[50], 64),
            10,
            "the delta must begin at the perspective cursor, not global revision 49"
        );
        assert_eq!(
            revision_log::current_delta_after_for_test(50, &[10, 50], &[], 64),
            50,
            "ticks with no newly entitled rows should carry no delta"
        );
    }

    #[test]
    fn in_place_pivot_finalizes_as_one_sparse_trail_chunk() {
        let mut game = one_player_game();
        let tank = game
            .state
            .entities
            .spawn_unit(1, EntityKind::Tank, 96.0, 96.0)
            .unwrap();

        game.state.ground_decals.begin_tick(1);
        game.state
            .ground_decals
            .update_tank_trails(&game.state.entities, &game.state.map, 1);
        game.state
            .entities
            .get_mut(tank)
            .unwrap()
            .set_facing(std::f32::consts::FRAC_PI_2);
        for tick in 2..=4 {
            game.state.ground_decals.begin_tick(tick);
            game.state.ground_decals.update_tank_trails(
                &game.state.entities,
                &game.state.map,
                tick,
            );
        }

        assert_eq!(game.state.ground_decals.tank_trails.finalized_len(), 1);
        let (_, decals, trails) = game.state.ground_decals.full_world_views_after(0);
        assert!(decals.is_empty());
        assert_eq!(trails.len(), 1);
        assert_eq!(trails[0].poses.len(), 2);
        assert_eq!(trails[0].poses[0][..2], trails[0].poses[1][..2]);
        assert_ne!(trails[0].poses[0][2], trails[0].poses[1][2]);
    }

    #[test]
    fn player_delta_is_fog_safe_while_full_world_gets_created_marks() {
        let mut store = GroundDecalStore::new();
        let map = one_player_game().state.map;
        store.create_artillery_impact(&map, 48.0, 48.0).unwrap();
        store.refresh_memory_for_player(1, &fog_with_visible_tile(1, Some(5)), &map);
        store.refresh_memory_for_player(2, &fog_with_visible_tile(2, None), &map);

        assert_eq!(
            store.views_for_players_after(&[2], 0),
            (0, Vec::new(), Vec::new())
        );
        assert!(store.recent_views_for_players(&[2], 64).2.is_empty());
        assert_eq!(store.views_for_players_after(&[1], 0).1.len(), 1);
        assert_eq!(store.recent_views_for_players(&[1], 64).2.len(), 1);
        assert_eq!(store.full_world_views_after(0).1.len(), 1);
        assert_eq!(store.recent_full_world_views(64).2.len(), 1);
    }

    #[test]
    fn recent_player_delta_is_revision_bounded_and_complete_after_its_cursor() {
        let mut store = GroundDecalStore::new();
        let map = one_player_game().state.map;
        for offset in 0..70 {
            store
                .create_mortar_impact(&map, 48.0 + offset as f32 * 0.01, 48.0)
                .unwrap();
        }
        store.refresh_memory_for_player(1, &fog_with_visible_tile(1, Some(5)), &map);

        let (revision, after_revision, decals, trails) = store.recent_views_for_players(&[1], 64);
        assert_eq!(after_revision, revision - 64);
        assert_eq!(decals.len(), 64);
        assert!(trails.is_empty());
        assert_eq!(
            decals,
            store.views_for_players_after(&[1], after_revision).1,
            "the advertised range must contain every entitled row after its cursor"
        );
        assert!(decals.iter().all(|decal| decal.owner == 0));
    }

    #[test]
    fn game_checkpoint_round_trip_preserves_marks_and_discovery_revisions() {
        let players = [
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "One".to_string(),
                color: "#fff".to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "Two".to_string(),
                color: "#000".to_string(),
                is_ai: false,
            },
        ];
        let mut game = Game::new(&players, 0xdec0_1234);
        let source = game
            .state
            .entities
            .iter()
            .find(|entity| entity.owner == 1 && death_ground_decal_class(entity.kind).is_some())
            .expect("player one worker")
            .clone();
        game.state
            .ground_decals
            .create_death(
                source.kind,
                source.pos_x,
                source.pos_y,
                source.owner,
                Some(source.facing()),
                source.weapon_facing(),
            )
            .unwrap();
        game.refresh_ground_decal_memory(&[1, 2]);
        let before_player = game.ground_decals_for_player(1, 0);
        let before_full = game.ground_decals_for_observer(&ObserverView::Omniscient, 0);
        assert_eq!(before_player.1.len(), 1);

        let payload = game.checkpoint_payload_text_for_test().unwrap();
        let restored = Game::restore_checkpoint_payload_text_for_test(
            &payload,
            game.state.map.clone(),
            game.map_metadata().clone(),
        )
        .unwrap();
        assert_eq!(restored.ground_decals_for_player(1, 0), before_player);
        assert_eq!(
            restored.ground_decals_for_observer(&ObserverView::Omniscient, 0),
            before_full
        );
    }

    #[test]
    fn checkpoint_restore_rebuilds_spatial_discovery_index() {
        let mut game = one_player_game();
        game.state
            .ground_decals
            .create_mortar_impact(&game.state.map, 48.0, 48.0)
            .unwrap();
        let payload = game.checkpoint_payload_text_for_test().unwrap();
        let mut restored = Game::restore_checkpoint_payload_text_for_test(
            &payload,
            game.state.map.clone(),
            game.map_metadata().clone(),
        )
        .unwrap();

        restored.state.ground_decals.refresh_memory_for_player(
            1,
            &fog_with_visible_tile(1, Some(5)),
            &restored.state.map,
        );

        assert_eq!(restored.ground_decals_for_player(1, 0).1.len(), 1);
    }

    #[test]
    fn checkpoint_round_trip_accepts_a_finalized_tank_trail() {
        let mut game = one_player_game();
        let tank = game
            .state
            .entities
            .spawn_unit(1, EntityKind::Tank, 96.0, 96.0)
            .unwrap();
        game.state.ground_decals.begin_tick(1);
        game.state
            .ground_decals
            .update_tank_trails(&game.state.entities, &game.state.map, 1);
        game.state
            .entities
            .get_mut(tank)
            .unwrap()
            .set_facing(std::f32::consts::FRAC_PI_2);
        for tick in 2..=4 {
            game.state.tick = tick;
            game.state.ground_decals.begin_tick(tick);
            game.state.ground_decals.update_tank_trails(
                &game.state.entities,
                &game.state.map,
                tick,
            );
        }
        crate::game::services::supply::recompute_supply(
            &mut game.state.players,
            &game.state.entities,
        );

        let payload = game.checkpoint_payload_text_for_test().unwrap();
        let restored = Game::restore_checkpoint_payload_text_for_test(
            &payload,
            game.state.map.clone(),
            game.map_metadata().clone(),
        )
        .unwrap();
        assert_eq!(
            restored
                .ground_decals_for_observer(&ObserverView::Omniscient, 0)
                .2
                .len(),
            1
        );
    }

    #[test]
    fn hidden_tank_trail_is_discovered_later_and_survives_checkpoint_restore() {
        let players = [
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "One".to_string(),
                color: "#fff".to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "Two".to_string(),
                color: "#000".to_string(),
                is_ai: false,
            },
        ];
        let mut game = Game::new(&players, 7);
        let tank = game
            .state
            .entities
            .spawn_unit(1, EntityKind::Tank, 96.0, 96.0)
            .unwrap();
        game.state.ground_decals.begin_tick(1);
        game.state
            .ground_decals
            .update_tank_trails(&game.state.entities, &game.state.map, 1);
        game.state
            .entities
            .get_mut(tank)
            .unwrap()
            .set_facing(std::f32::consts::FRAC_PI_2);
        for tick in 2..=4 {
            game.state.tick = tick;
            game.state.ground_decals.begin_tick(tick);
            game.state.ground_decals.update_tank_trails(
                &game.state.entities,
                &game.state.map,
                tick,
            );
        }
        crate::game::services::supply::recompute_supply(
            &mut game.state.players,
            &game.state.entities,
        );

        game.state.ground_decals.refresh_memory_for_player(
            2,
            &fog_with_visible_tile(2, None),
            &game.state.map,
        );
        assert!(game.ground_decals_for_player(2, 0).2.is_empty());
        let fully_visible = Fog::from_checkpoint_grids(
            4,
            4,
            BTreeMap::from([(2, vec![true; 16])]),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        game.state
            .ground_decals
            .refresh_memory_for_player(2, &fully_visible, &game.state.map);
        assert_eq!(game.ground_decals_for_player(2, 0).2.len(), 1);

        let payload = game.checkpoint_payload_text_for_test().unwrap();
        let restored = Game::restore_checkpoint_payload_text_for_test(
            &payload,
            game.state.map.clone(),
            game.map_metadata().clone(),
        )
        .unwrap();
        assert_eq!(restored.ground_decals_for_player(2, 0).2.len(), 1);
    }

    #[test]
    fn checkpoint_rejects_noncanonical_blast_radius() {
        let mut game = one_player_game();
        game.state
            .ground_decals
            .create_mortar_impact(&game.state.map, 48.0, 48.0)
            .unwrap();
        let payload = game.checkpoint_payload_text_for_test().unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&payload).unwrap();
        value["groundDecals"]["decals"][0]["radiusTiles"] = serde_json::json!(999.0);
        let malformed = serde_json::to_string(&value).unwrap();
        let result = Game::restore_checkpoint_payload_text_for_test(
            &malformed,
            game.state.map.clone(),
            game.map_metadata().clone(),
        );
        assert!(
            result.is_err(),
            "noncanonical blast radii must reject restore"
        );
    }

    #[test]
    fn checkpoint_rejects_noncanonical_revision_gaps() {
        let mut game = one_player_game();
        game.state
            .ground_decals
            .create_mortar_impact(&game.state.map, 48.0, 48.0)
            .unwrap();
        let payload = game.checkpoint_payload_text_for_test().unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&payload).unwrap();
        value["groundDecals"]["revision"] = serde_json::json!(u32::MAX);
        let malformed = serde_json::to_string(&value).unwrap();
        let result = Game::restore_checkpoint_payload_text_for_test(
            &malformed,
            game.state.map.clone(),
            game.map_metadata().clone(),
        );
        assert!(result.is_err(), "revision gaps must reject restore");
    }

    #[test]
    fn edge_blasts_keep_checkpoint_state_canonical() {
        let mut game = one_player_game();
        game.state
            .ground_decals
            .create_mortar_impact(&game.state.map, -16.0, 48.0)
            .unwrap();
        assert!(
            game.state
                .ground_decals
                .create_mortar_impact(&game.state.map, -200.0, 48.0)
                .is_none(),
            "a fully off-map blast has no visible mark to retain"
        );

        let payload = game.checkpoint_payload_text_for_test().unwrap();
        let restored = Game::restore_checkpoint_payload_text_for_test(
            &payload,
            game.state.map.clone(),
            game.map_metadata().clone(),
        );

        assert!(
            restored.is_ok(),
            "a legal edge impact must not make the server's own checkpoint unrestorable"
        );
    }
}
