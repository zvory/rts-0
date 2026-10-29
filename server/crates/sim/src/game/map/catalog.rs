//! Discovery, metadata, and loading for maps bundled with the server.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::{
    authored, fnv_bytes, AvailableMap, Map, DEFAULT_MAP_JSON, DEFAULT_MAP_NAME, FNV_OFFSET_BASIS,
};

const MAPS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/maps");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MapMetadata {
    pub name: String,
    pub schema_version: u32,
    pub content_hash: String,
}

impl Map {
    /// Return all available maps in `assets/maps/`. Only maps with the current schema version are
    /// included. Unreadable or invalid assets are skipped so a bad asset cannot crash the lobby.
    pub fn list_available() -> Vec<AvailableMap> {
        let Some(dir) = bundled_maps_dir() else {
            return vec![default_available_map()];
        };
        let Ok(entries) = std::fs::read_dir(dir) else {
            return vec![default_available_map()];
        };
        let mut paths: Vec<_> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
            .map(|entry| entry.path())
            .collect();
        paths.sort();

        let mut available = Vec::new();
        for path in paths {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("")
                .to_string();
            let Some(json) = std::fs::read_to_string(&path).ok() else {
                continue;
            };
            if let Some(entry) = available_map_from_json(&stem, &json) {
                available.push(entry);
            }
        }
        if available.is_empty() {
            available.push(default_available_map());
        }
        available
    }

    /// Load a map by display name (the `name` field in the JSON) for `player_count` players.
    /// Returns an error string if the map cannot be found, read, or parsed.
    pub fn load(map_name: &str, player_count: usize, seed: u32) -> Result<Map, String> {
        let (name, json) = authored_json_for_name(map_name)?;
        Self::from_authored_json_with_name(player_count, &name, &json, seed)
    }

    /// Load a map by display name and assign starts to the ordered players.
    pub fn load_for_players(
        map_name: &str,
        players: &[(u32, u32)],
        seed: u32,
    ) -> Result<Map, String> {
        let (name, json) = authored_json_for_name(map_name)?;
        Self::from_authored_json_with_name_for_players(players, &name, &json, seed)
    }

    pub fn metadata_for_name(map_name: &str) -> Result<MapMetadata, String> {
        let (name, json) = authored_json_for_name(map_name)?;
        Ok(MapMetadata {
            name,
            schema_version: authored::schema_version(&json)?,
            content_hash: stable_content_hash(&json),
        })
    }
}

fn authored_json_for_name(map_name: &str) -> Result<(String, String), String> {
    // First try to match by `name` field, then by filename stem.
    if let Some(dir) = bundled_maps_dir() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return Err(format!("cannot read maps directory: {}", dir.display()));
        };
        let mut paths: Vec<_> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("json"))
            .map(|entry| entry.path())
            .collect();
        paths.sort();

        for path in paths {
            let stem = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .unwrap_or("")
                .to_string();
            let json = std::fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let json_name = serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|value| {
                    value
                        .get("name")
                        .and_then(|name| name.as_str())
                        .map(str::to_string)
                });
            if json_name.as_deref() == Some(map_name) || stem == map_name {
                return Ok((json_name.unwrap_or(stem), json));
            }
        }
    }
    if map_name == DEFAULT_MAP_NAME {
        return Ok((DEFAULT_MAP_NAME.to_string(), DEFAULT_MAP_JSON.to_string()));
    }
    Err(format!("map not found: {map_name:?}"))
}

fn stable_content_hash(content: &str) -> String {
    format!("{:016x}", fnv_bytes(FNV_OFFSET_BASIS, content.as_bytes()))
}

fn default_available_map() -> AvailableMap {
    available_map_from_json(DEFAULT_MAP_NAME, DEFAULT_MAP_JSON).unwrap_or_else(|| AvailableMap {
        name: DEFAULT_MAP_NAME.to_string(),
        description: DEFAULT_MAP_NAME.to_string(),
        min_players: 1,
        max_players: 4,
    })
}

fn available_map_from_json(stem: &str, json: &str) -> Option<AvailableMap> {
    let value = serde_json::from_str::<serde_json::Value>(json).ok()?;
    let version = value
        .get("version")
        .and_then(|version| version.as_u64())
        .unwrap_or(0);
    if !u32::try_from(version).is_ok_and(super::supported_map_version) {
        return None;
    }
    let name = value
        .get("name")
        .and_then(|name| name.as_str())
        .unwrap_or(stem)
        .to_string();
    let description = value
        .get("description")
        .and_then(|description| description.as_str())
        .unwrap_or(&name)
        .to_string();
    let (min_players, max_players) = authored::player_count_bounds(json).ok()?;
    (!name.is_empty()).then_some(AvailableMap {
        name,
        description,
        min_players,
        max_players,
    })
}

fn bundled_maps_dir() -> Option<PathBuf> {
    maps_dir_candidates().into_iter().find(|path| path.is_dir())
}

fn maps_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from(MAPS_DIR)];
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("server/assets/maps"));
        candidates.push(cwd.join("assets/maps"));
    }
    if let Ok(executable) = std::env::current_exe() {
        for ancestor in executable.ancestors() {
            candidates.push(ancestor.join("server/assets/maps"));
            candidates.push(ancestor.join("assets/maps"));
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_map_catalog_loads_available_maps_by_name() {
        let available = Map::list_available();
        assert!(
            !available.is_empty(),
            "lobby map catalog must expose at least one selectable map"
        );
        let names: Vec<&str> = available.iter().map(|entry| entry.name.as_str()).collect();
        assert!(names.contains(&"Chokes"), "got: {names:?}");
        assert!(names.contains(&"Classic"), "got: {names:?}");
        assert!(names.contains(&"1v1 No Terrain"), "got: {names:?}");
        assert!(names.contains(&"4 Player Map"), "got: {names:?}");
        for entry in &available {
            assert!(
                entry.min_players >= 1 && entry.min_players <= entry.max_players,
                "bad player bounds on {}: {}..={}",
                entry.name,
                entry.min_players,
                entry.max_players
            );
        }
        assert_eq!(
            available
                .iter()
                .find(|entry| entry.name == "Fastest Map Possible")
                .map(|entry| entry.description.as_str()),
            Some(""),
            "the explicitly blank map description must remain blank"
        );

        let map = Map::load("Chokes", 2, 0x1234_5678)
            .expect("default handcrafted map should load from bundled assets");
        assert_eq!(map.width, 126);
        assert_eq!(map.starts.len(), 2);

        let classic_authored = available
            .iter()
            .find(|entry| entry.name == "Classic")
            .expect("Classic map should be listed");
        assert_eq!(classic_authored.min_players, 1);
        assert_eq!(classic_authored.max_players, 2);
        let classic_map = Map::load("Classic", 2, 0x1234_5678)
            .expect("Classic should load for two active players");
        assert_eq!(
            classic_map.base_sites.len(),
            10,
            "Classic must retain all ten permanent resource bases"
        );
        assert!(
            Map::load("Classic", 3, 0x1234_5678).is_err(),
            "Classic should not expose a third start location"
        );
        for seed in 0..32 {
            let mut starts = Map::load("Classic", 2, seed)
                .expect("Classic should load for two active players")
                .starts;
            starts.sort_unstable();
            assert_eq!(
                starts,
                vec![(9, 9), (116, 116)],
                "Classic must only use its two authored start locations for seed {seed}"
            );
        }

        let classic_v2 = available
            .iter()
            .find(|entry| entry.name == "Classic V2")
            .expect("Classic V2 should be listed separately from Classic");
        assert_eq!(classic_v2.min_players, 1);
        assert_eq!(classic_v2.max_players, 2);
        let classic_v2_map = Map::load("Classic V2", 2, 0x1234_5678)
            .expect("Classic V2 should load for two active players");
        assert_eq!((classic_v2_map.width, classic_v2_map.height), (189, 126));
        assert_eq!(classic_v2_map.base_sites.len(), 12);
        assert_ne!(
            Map::metadata_for_name("Classic").unwrap().content_hash,
            Map::metadata_for_name("Classic V2").unwrap().content_hash,
        );

        let one_v_one = available
            .iter()
            .find(|entry| entry.name == "1v1 No Terrain")
            .expect("1v1 no-terrain scaffold should be listed");
        assert_eq!(one_v_one.min_players, 1);
        assert_eq!(one_v_one.max_players, 2);
        assert!(
            Map::load("1v1 No Terrain", 2, 0x1234_5678).is_ok(),
            "1v1 No Terrain should load for two active players"
        );
        assert!(
            Map::load("1v1 No Terrain", 3, 0x1234_5678).is_err(),
            "1v1 No Terrain should not expose a third start location"
        );
        for seed in 0..32 {
            let mut starts = Map::load("1v1 No Terrain", 2, seed)
                .expect("1v1 No Terrain should load for two active players")
                .starts;
            starts.sort_unstable();
            assert_eq!(
                starts,
                vec![(25, 25), (100, 100)],
                "1v1 No Terrain must only use its two opposing start locations for seed {seed}"
            );
        }

        let four_player = available
            .iter()
            .find(|entry| entry.name == "4 Player Map")
            .expect("four-player map should be listed");
        assert_eq!(four_player.min_players, 1);
        assert_eq!(four_player.max_players, 4);
        for player_count in 1..=4 {
            let map = Map::load("4 Player Map", player_count, 0x1234_5678)
                .expect("four-player map should load for every supported player count");
            assert_eq!(map.width, 166);
            assert_eq!(map.starts.len(), player_count);
            assert_eq!(map.base_sites.len(), 16);
        }
    }

    #[test]
    fn afrika_two_vs_two_uses_authored_team_sides() {
        let available = Map::list_available();
        let entry = available
            .iter()
            .find(|entry| entry.name == "Afrika (2v2)")
            .expect("Afrika (2v2) must be selectable in the lobby");
        assert_eq!((entry.min_players, entry.max_players), (1, 4));
        let side_one = [(26, 117), (12, 104)]; // Authored S1 and S3.
        let side_two = [(173, 8), (187, 21)]; // Authored S2 and S4.
        for roster in [
            [(1, 1), (2, 1), (3, 2), (4, 2)],
            [(7, 9), (2, 4), (5, 9), (6, 4)],
        ] {
            for seed in 0..32 {
                let map = Map::load_for_players("Afrika (2v2)", &roster, seed).unwrap();
                assert_eq!(map.starts.len(), roster.len());
                let mut unique_starts = map.starts.clone();
                unique_starts.sort_unstable();
                unique_starts.dedup();
                assert_eq!(unique_starts.len(), roster.len());
                assert_eq!((map.width, map.height), (200, 126));
                assert_eq!(map.base_sites.len(), 20);
                for (i, start) in map.starts.iter().enumerate() {
                    assert!(side_one.contains(start) || side_two.contains(start));
                    for j in 0..i {
                        assert_eq!(
                            roster[i].1 == roster[j].1,
                            side_one.contains(start) == side_one.contains(&map.starts[j])
                        );
                    }
                }
            }
        }
        assert!(
            Map::load_for_players("Afrika (2v2)", &[(1, 1), (2, 2), (3, 3), (4, 4)], 1).is_err()
        );
    }

    #[test]
    fn every_catalog_map_materializes_for_its_advertised_player_counts() {
        for entry in Map::list_available() {
            for player_count in entry.min_players..=entry.max_players {
                let (_, json) = authored_json_for_name(&entry.name).unwrap();
                // Player counts do not imply a roster: grouped maps can reject FFA.
                let map = Map::materialize_authored_json(&json, player_count as usize)
                    .unwrap_or_else(|error| {
                        panic!(
                            "catalog map {:?} failed to load for {player_count} player(s): {error}",
                            entry.name
                        )
                    });
                assert_eq!(map.starts.len(), entry.max_players as usize);
                // Use the authored groups to construct a compatible roster; ungrouped maps
                // must still support an FFA roster for every advertised count.
                let players: Vec<_> = (0..player_count as usize)
                    .map(|index| {
                        let id = index as u32 + 1;
                        let team = map.start_team_groups.get(index).copied().unwrap_or(id);
                        (id, team)
                    })
                    .collect();
                let assigned = Map::load_for_players(&entry.name, &players, 0x1234_5678)
                    .unwrap_or_else(|error| {
                        panic!(
                            "catalog map {:?} failed to assign {player_count} player(s): {error}",
                            entry.name
                        )
                    });
                assert_eq!(assigned.starts.len(), player_count as usize);
                assert_eq!(
                    map.terrain.len(),
                    (map.width * map.height) as usize,
                    "catalog map {:?} materialized an invalid terrain grid",
                    entry.name
                );
            }
        }
    }
}
