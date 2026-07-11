use super::*;

fn ai_player(id: u32, faction_id: &str) -> PlayerInit {
    PlayerInit {
        id,
        team_id: id,
        faction_id: faction_id.to_string(),
        name: format!("AI {id}"),
        color: "#fff".to_string(),
        is_ai: true,
    }
}

fn starting_base_id(game: &Game, player_id: u32, kind: EntityKind) -> u32 {
    let player = game
        .state
        .players
        .iter()
        .find(|player| player.id == player_id)
        .expect("player should exist");
    let (start_x, start_y) = game
        .state
        .map
        .tile_center(player.start_tile.0, player.start_tile.1);
    let max_dist_sq = (config::TILE_SIZE as f32 * 0.5).powi(2);
    game.state
        .entities
        .iter()
        .find(|entity| {
            entity.owner == player_id && entity.kind == kind && {
                let dx = entity.pos_x - start_x;
                let dy = entity.pos_y - start_y;
                dx * dx + dy * dy <= max_dist_sq
            }
        })
        .map(|entity| entity.id)
        .expect("starting base should exist")
}

fn expansion_center(game: &Game, player_id: u32) -> (f32, f32) {
    let start_tile = game
        .state
        .players
        .iter()
        .find(|player| player.id == player_id)
        .expect("player should exist")
        .start_tile;
    let expansion_tile = game
        .state
        .map
        .base_sites
        .iter()
        .copied()
        .find(|tile| *tile != start_tile)
        .unwrap_or_else(|| {
            (
                start_tile.0.saturating_add(12),
                start_tile.1.saturating_add(12),
            )
        });
    game.state
        .map
        .tile_center(expansion_tile.0, expansion_tile.1)
}

#[test]
fn primary_base_alive_players_do_not_count_expansion_city_centres() {
    let players = [ai_player(1, "kriegsia"), ai_player(2, "kriegsia")];
    let mut game = Game::new_without_ai_controllers(&players, 0x5150_0B45);
    assert!(game.primary_base_alive_players().contains(&2));

    let starting_city_centre = starting_base_id(&game, 2, EntityKind::CityCentre);
    game.state.entities.remove(starting_city_centre);
    let expansion = expansion_center(&game, 2);
    game.state
        .entities
        .spawn_building(2, EntityKind::CityCentre, expansion.0, expansion.1, true)
        .expect("expansion City Centre should spawn");

    assert!(
        game.alive_players().contains(&2),
        "generic elimination still counts the surviving expansion base"
    );
    assert!(
        !game.primary_base_alive_players().contains(&2),
        "primary-base objective should end when the starting City Centre is gone"
    );
}

#[test]
fn primary_base_alive_players_track_ekat_zamok_starts() {
    let players = [ai_player(1, "ekat"), ai_player(2, "ekat")];
    let mut game = Game::new_without_ai_controllers(&players, 0xE1A7_0B45);
    assert_eq!(game.primary_base_alive_players(), vec![1, 2]);

    let starting_zamok = starting_base_id(&game, 2, EntityKind::Zamok);
    game.state.entities.remove(starting_zamok);

    assert_eq!(game.primary_base_alive_players(), vec![1]);
}
