use std::collections::{BTreeMap, BTreeSet};

use crate::config;
use crate::game::entity::EntityKind;
use crate::game::fog::Fog;
use crate::protocol::{self, GroundDecalView};
use crate::rules::{
    artillery_ground_decal_source_kind, death_ground_decal_class, mortar_ground_decal_source_kind,
};
use serde::{Deserialize, Serialize};

pub(crate) const MAX_GROUND_DECALS: usize = 4_096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GroundDecal {
    id: u32,
    decal_class: String,
    source_kind: String,
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
            decal_class: self.decal_class.clone(),
            source_kind: self.source_kind.clone(),
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
///
/// The beta cap is intentionally simple: once full, the match stops creating new marks. Existing
/// marks and revision cursors never shift underneath a client.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct GroundDecalStore {
    next_id: u32,
    revision: u32,
    decals: Vec<GroundDecal>,
    discovered_by_player: BTreeMap<u32, BTreeMap<u32, u32>>,
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
            decals: Vec::new(),
            discovered_by_player: BTreeMap::new(),
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
        let decal_class = death_ground_decal_class(kind)?;
        self.create(
            decal_class,
            protocol::kind_to_wire(kind),
            x,
            y,
            owner,
            facing,
            weapon_facing,
            None,
        )
    }

    pub(crate) fn create_mortar_impact(&mut self, _owner: u32, x: f32, y: f32) -> Option<u32> {
        self.create(
            "mortarBlast",
            protocol::kind_to_wire(mortar_ground_decal_source_kind()),
            x,
            y,
            0,
            None,
            None,
            Some(config::MORTAR_OUTER_RADIUS_TILES),
        )
    }

    pub(crate) fn create_artillery_impact(&mut self, _owner: u32, x: f32, y: f32) -> Option<u32> {
        self.create(
            "artilleryBlast",
            protocol::kind_to_wire(artillery_ground_decal_source_kind()),
            x,
            y,
            0,
            None,
            None,
            Some(config::ARTILLERY_OUTER_RADIUS_TILES),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create(
        &mut self,
        decal_class: &str,
        source_kind: &str,
        x: f32,
        y: f32,
        owner: u32,
        facing: Option<f32>,
        weapon_facing: Option<f32>,
        radius_tiles: Option<f32>,
    ) -> Option<u32> {
        if self.decals.len() >= MAX_GROUND_DECALS
            || self.next_id == 0
            || !x.is_finite()
            || !y.is_finite()
            || facing.is_some_and(|value| !value.is_finite())
            || weapon_facing.is_some_and(|value| !value.is_finite())
            || radius_tiles.is_some_and(|value| !value.is_finite() || value <= 0.0)
        {
            return None;
        }
        let created_revision = self.next_revision()?;
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).unwrap_or(0);
        self.decals.push(GroundDecal {
            id,
            decal_class: decal_class.to_string(),
            source_kind: source_kind.to_string(),
            x,
            y,
            owner,
            seed: decal_seed(id, x, y),
            facing,
            weapon_facing,
            radius_tiles,
            created_revision,
        });
        Some(id)
    }

    pub(crate) fn refresh_memory_for_player(&mut self, player: u32, fog: &Fog) {
        let newly_visible = self
            .decals
            .iter()
            .filter(|decal| {
                !self
                    .discovered_by_player
                    .get(&player)
                    .is_some_and(|known| known.contains_key(&decal.id))
                    && decal_visible_to_player(decal, player, fog)
            })
            .map(|decal| decal.id)
            .collect::<Vec<_>>();
        for id in newly_visible {
            let Some(revision) = self.next_revision() else {
                return;
            };
            self.discovered_by_player
                .entry(player)
                .or_default()
                .insert(id, revision);
        }
    }

    pub(crate) fn revision_for_players(&self, players: &[u32]) -> u32 {
        players
            .iter()
            .filter_map(|player| self.discovered_by_player.get(player))
            .flat_map(|known| known.values().copied())
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn views_for_players_after(
        &self,
        players: &[u32],
        after_revision: u32,
    ) -> (u32, Vec<GroundDecalView>) {
        let revision = self.revision_for_players(players);
        let decals = self
            .decals
            .iter()
            .filter(|decal| {
                players.iter().any(|player| {
                    self.discovered_by_player
                        .get(player)
                        .and_then(|known| known.get(&decal.id))
                        .is_some_and(|revision| *revision > after_revision)
                })
            })
            .map(GroundDecal::to_view)
            .collect();
        (revision, decals)
    }

    pub(crate) fn full_world_revision(&self) -> u32 {
        self.revision
    }

    pub(crate) fn full_world_views_after(
        &self,
        after_revision: u32,
    ) -> (u32, Vec<GroundDecalView>) {
        (
            self.revision,
            self.decals
                .iter()
                .filter(|decal| decal.created_revision > after_revision)
                .map(GroundDecal::to_view)
                .collect(),
        )
    }

    pub(in crate::game) fn checkpoint_len(&self) -> usize {
        self.decals.len()
    }

    pub(in crate::game) fn valid_checkpoint_state(
        &self,
        map: &crate::game::map::Map,
        player_ids: &BTreeSet<u32>,
    ) -> bool {
        if self.next_id == 0 || self.decals.len() > MAX_GROUND_DECALS {
            return false;
        }
        let mut ids = BTreeSet::new();
        let mut used_revisions = BTreeSet::new();
        for decal in &self.decals {
            let presentation_valid = decal.id != 0
                && ids.insert(decal.id)
                && decal.created_revision != 0
                && decal.created_revision <= self.revision
                && used_revisions.insert(decal.created_revision)
                && map.contains_world_point(decal.x, decal.y)
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
        usize::try_from(self.revision).ok() == Some(used_revisions.len())
    }

    fn next_revision(&mut self) -> Option<u32> {
        let next = self.revision.checked_add(1)?;
        self.revision = next;
        Some(next)
    }
}

fn valid_class_and_source(decal: &GroundDecal) -> bool {
    let kind = EntityKind::ALL
        .into_iter()
        .find(|kind| protocol::kind_to_wire(*kind) == decal.source_kind);
    match decal.decal_class.as_str() {
        "mortarBlast" => {
            kind == Some(mortar_ground_decal_source_kind())
                && decal
                    .radius_tiles
                    .is_some_and(|radius| radius == config::MORTAR_OUTER_RADIUS_TILES)
                && decal.owner == 0
        }
        "artilleryBlast" => {
            kind == Some(artillery_ground_decal_source_kind())
                && decal
                    .radius_tiles
                    .is_some_and(|radius| radius == config::ARTILLERY_OUTER_RADIUS_TILES)
                && decal.owner == 0
        }
        class => {
            decal.radius_tiles.is_none()
                && kind
                    .and_then(death_ground_decal_class)
                    .is_some_and(|expected| expected == class)
        }
    }
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
    fn store_stops_at_cap_without_evicting_or_wrapping_ids() {
        let mut store = GroundDecalStore::new();
        for _ in 0..MAX_GROUND_DECALS {
            assert!(store.create_mortar_impact(1, 32.0, 32.0).is_some());
        }
        assert!(store.create_mortar_impact(1, 64.0, 64.0).is_none());
        assert_eq!(store.decals.first().map(|decal| decal.id), Some(1));
        assert_eq!(store.decals.last().map(|decal| decal.id), Some(4_096));
    }

    #[test]
    fn hidden_mark_is_sent_only_after_first_physical_discovery() {
        let mut store = GroundDecalStore::new();
        store.create_mortar_impact(1, 48.0, 48.0).unwrap();

        store.refresh_memory_for_player(2, &fog_with_visible_tile(2, None));
        assert_eq!(store.views_for_players_after(&[2], 0), (0, Vec::new()));

        store.refresh_memory_for_player(2, &fog_with_visible_tile(2, Some(5)));
        let (revision, decals) = store.views_for_players_after(&[2], 0);
        assert!(revision > 0);
        assert_eq!(decals.len(), 1);
        assert_eq!(decals[0].decal_class, "mortarBlast");
        assert_eq!(
            decals[0].owner, 0,
            "impact decals must not leak firing owner"
        );
        assert_eq!(
            store.views_for_players_after(&[2], revision),
            (revision, Vec::new())
        );

        store.refresh_memory_for_player(2, &fog_with_visible_tile(2, Some(5)));
        assert_eq!(store.revision_for_players(&[2]), revision);
    }

    #[test]
    fn player_delta_is_fog_safe_while_full_world_gets_created_marks() {
        let mut store = GroundDecalStore::new();
        store.create_artillery_impact(1, 48.0, 48.0).unwrap();
        store.refresh_memory_for_player(1, &fog_with_visible_tile(1, Some(5)));
        store.refresh_memory_for_player(2, &fog_with_visible_tile(2, None));

        assert_eq!(store.views_for_players_after(&[2], 0), (0, Vec::new()));
        assert_eq!(store.views_for_players_after(&[1], 0).1.len(), 1);
        assert_eq!(store.full_world_views_after(0).1.len(), 1);
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
    fn checkpoint_rejects_noncanonical_blast_radius() {
        let players = [PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".to_string(),
            color: "#fff".to_string(),
            is_ai: false,
        }];
        let mut game = Game::new(&players, 7);
        game.state
            .ground_decals
            .create_mortar_impact(1, 48.0, 48.0)
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
        let players = [PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".to_string(),
            color: "#fff".to_string(),
            is_ai: false,
        }];
        let mut game = Game::new(&players, 7);
        game.state
            .ground_decals
            .create_mortar_impact(1, 48.0, 48.0)
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
}
