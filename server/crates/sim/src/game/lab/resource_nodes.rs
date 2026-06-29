use crate::game::entity::{EntityKind, EntityStore};
use crate::game::map::Map;
use crate::game::resource_placement;

use super::{validate_resource_node_position, validate_world_position, LabError};

// Restore can snap legacy oil coordinates, but invalid authoring should not be relocated
// across the map.
const OIL_RESTORE_SEARCH_RADIUS_TILES: u32 = 7;

pub(super) fn restore_resource_node_position(
    map: &Map,
    entities: &EntityStore,
    kind: EntityKind,
    x: f32,
    y: f32,
) -> Result<(f32, f32), LabError> {
    if kind != EntityKind::Oil {
        validate_resource_node_position(map, entities, x, y)?;
        return Ok((x, y));
    }

    validate_world_position(map, x, y)?;
    let source_tile = map.tile_of(x, y);
    let occupied_tiles = resource_placement::occupied_oil_tiles(map, entities);
    let Some((center_x, center_y, _tile)) =
        resource_placement::nearest_oil_tile_center(map, x, y, |tile, center_x, center_y| {
            tile.0.abs_diff(source_tile.0) <= OIL_RESTORE_SEARCH_RADIUS_TILES
                && tile.1.abs_diff(source_tile.1) <= OIL_RESTORE_SEARCH_RADIUS_TILES
                && resource_placement::tile_has_one_tile_oil_gap(tile, &occupied_tiles)
                && validate_resource_node_position(map, entities, center_x, center_y).is_ok()
        })
    else {
        return Err(LabError::InvalidPosition {
            x,
            y,
            reason: "oil node must have a nearby passable tile center with one tile of spacing",
        });
    };

    Ok((center_x, center_y))
}

#[cfg(test)]
mod tests {
    use crate::config;
    use crate::game::entity::{EntityKind, NEUTRAL};
    use crate::game::lab::{LabOp, LabScenarioEntity};
    use crate::game::map::Map;
    use crate::game::{Game, PlayerInit};

    fn lab_players() -> [PlayerInit; 2] {
        [
            PlayerInit {
                id: 1,
                team_id: 1,
                faction_id: "kriegsia".to_string(),
                name: "Alpha".to_string(),
                color: "#4878c8".to_string(),
                is_ai: false,
            },
            PlayerInit {
                id: 2,
                team_id: 2,
                faction_id: "kriegsia".to_string(),
                name: "Bravo".to_string(),
                color: "#c84848".to_string(),
                is_ai: false,
            },
        ]
    }

    fn default_map_game() -> Game {
        let players = lab_players();
        let start_players: Vec<_> = players
            .iter()
            .map(|player| (player.id, player.team_id))
            .collect();
        let map =
            Map::load_for_players("Default", &start_players, 0xABCD).expect("default lab map");
        let metadata = Map::metadata_for_name("Default").expect("default map metadata");
        Game::new_lab(&players, 0xABCD, map, metadata)
    }

    fn first_passable_tile(game: &Game) -> (u32, u32) {
        for ty in 8..game.map.size.saturating_sub(8) {
            for tx in 8..game.map.size.saturating_sub(8) {
                if game.map.is_passable(tx as i32, ty as i32) {
                    return (tx, ty);
                }
            }
        }
        panic!("no passable tile found");
    }

    fn lab_oil_entity(id: u32, x: f32, y: f32) -> LabScenarioEntity {
        LabScenarioEntity {
            id,
            owner: NEUTRAL,
            kind: EntityKind::Oil.to_string(),
            x,
            y,
            hp: 1,
            completed: true,
            construction_progress: None,
            construction_total: None,
            resource_remaining: Some(config::OIL_GEYSER_AMOUNT),
            facing: None,
            weapon_facing: None,
            set_up: false,
            setup_facing: None,
            setup_target: None,
        }
    }

    #[test]
    fn lab_scenario_restore_centers_and_spaces_oil_nodes() {
        let source = default_map_game();
        let mut scenario = source.export_lab_scenario();
        let (tile_x, tile_y) = first_passable_tile(&source);
        let (center_x, center_y) = source.map.tile_center(tile_x, tile_y);
        scenario.entities = vec![
            lab_oil_entity(101, center_x - 12.0, center_y - 12.0),
            lab_oil_entity(102, center_x + 10.0, center_y + 10.0),
            lab_oil_entity(103, center_x + 12.0, center_y - 8.0),
        ];

        let mut restored = default_map_game();
        restored
            .apply_lab_op(LabOp::RestoreScenario(Box::new(scenario)))
            .expect("scenario restore should normalize oil nodes");

        let oils: Vec<_> = restored
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Oil)
            .collect();
        assert_eq!(oils.len(), 3);
        let mut oil_tiles = Vec::new();
        for oil in oils {
            let (oil_tile_x, oil_tile_y) = restored.map.tile_of(oil.pos_x, oil.pos_y);
            let (expected_x, expected_y) = restored.map.tile_center(oil_tile_x, oil_tile_y);
            assert!(
                (oil.pos_x - expected_x).abs() < 0.001
                    && (oil.pos_y - expected_y).abs() < 0.001,
                "restored oil node {} should be centered on tile ({oil_tile_x}, {oil_tile_y})",
                oil.id
            );
            oil_tiles.push((oil.id, oil_tile_x, oil_tile_y));
        }
        for (index, &(a_id, a_x, a_y)) in oil_tiles.iter().enumerate() {
            for &(b_id, b_x, b_y) in oil_tiles.iter().skip(index + 1) {
                assert!(
                    a_x.abs_diff(b_x) > 1 || a_y.abs_diff(b_y) > 1,
                    "restored oil nodes {a_id} and {b_id} should have one free tile between them, got tiles ({a_x}, {a_y}) and ({b_x}, {b_y})"
                );
            }
        }
    }
}
