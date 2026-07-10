use super::*;

#[test]
fn singleton_team_assignment_matches_legacy_ffa_shuffle() {
    for player_count in 1..=4 {
        for seed in 0..16u32 {
            let legacy = Map::generate(player_count, seed);
            let players: Vec<_> = (1..=player_count)
                .map(|id| (id as u32, id as u32))
                .collect();
            let assigned = Map::generate_for_players(&players, seed);

            assert_eq!(
                assigned.starts, legacy.starts,
                "player_count={player_count} seed={seed}"
            );
            assert_eq!(
                assigned.expansion_sites, legacy.expansion_sites,
                "player_count={player_count} seed={seed}"
            );
        }
    }
}

#[test]
fn two_vs_two_team_starts_are_adjacent_on_default_map() {
    let players = start_players(&[(1, 1), (2, 1), (3, 2), (4, 2)]);
    let map =
        Map::load_for_players("Default", &players, 0x1020_3040).expect("default map should load");

    let team_one_distance = tile_distance_sq(map.starts[0], map.starts[1]);
    let team_two_distance = tile_distance_sq(map.starts[2], map.starts[3]);
    let opposite_corner_baseline = tile_distance_sq((13, 12), (112, 113));

    assert!(
        team_one_distance < opposite_corner_baseline,
        "team 1 assigned starts too far apart: {:?}",
        map.starts
    );
    assert!(
        team_two_distance < opposite_corner_baseline,
        "team 2 assigned starts too far apart: {:?}",
        map.starts
    );
}

#[test]
fn one_vs_two_keeps_larger_team_together_when_layout_supports_it() {
    let players = start_players(&[(1, 1), (2, 2), (3, 2)]);
    let map =
        Map::load_for_players("Default", &players, 0x5566_7788).expect("default map should load");

    let teammate_distance = tile_distance_sq(map.starts[1], map.starts[2]);
    let solo_to_first_teammate = tile_distance_sq(map.starts[0], map.starts[1]);
    let solo_to_second_teammate = tile_distance_sq(map.starts[0], map.starts[2]);

    assert!(
        teammate_distance <= solo_to_first_teammate.max(solo_to_second_teammate),
        "2-player team was not kept together: {:?}",
        map.starts
    );
}

#[test]
fn one_vs_three_is_deterministic_on_four_start_map() {
    let players = start_players(&[(1, 1), (2, 2), (3, 2), (4, 2)]);
    let a =
        Map::load_for_players("Default", &players, 0xfeed_cafe).expect("default map should load");
    let b =
        Map::load_for_players("Default", &players, 0xfeed_cafe).expect("default map should load");

    assert_eq!(a.starts, b.starts);
    assert_eq!(a.expansion_sites, b.expansion_sites);
}

#[test]
fn synthetic_six_start_map_assigns_arbitrary_team_sizes() {
    let json = synthetic_six_start_map_json();
    let players = start_players(&[(1, 1), (2, 1), (3, 1), (4, 2), (5, 2), (6, 3)]);
    let map = Map::from_authored_json_for_players(&players, &json, 0x1234_abcd)
        .expect("synthetic map should load");

    let team_one_spread = tile_distance_sq(map.starts[0], map.starts[1])
        + tile_distance_sq(map.starts[0], map.starts[2])
        + tile_distance_sq(map.starts[1], map.starts[2]);
    let split_cluster_baseline =
        tile_distance_sq((16, 16), (84, 84)) + tile_distance_sq((16, 36), (36, 16));

    assert_eq!(map.starts.len(), 6);
    assert_eq!(map.expansion_sites.len(), 6);
    assert!(
        team_one_spread < split_cluster_baseline,
        "team-aware assignment should not split the 3-player team across clusters: {:?}",
        map.starts
    );
}

#[test]
fn start_payload_reports_team_id_with_assigned_start_tile() {
    let players = vec![
        crate::game::PlayerInit {
            id: 10,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Alpha".to_string(),
            color: "#f00".to_string(),
            is_ai: false,
        },
        crate::game::PlayerInit {
            id: 20,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Bravo".to_string(),
            color: "#0f0".to_string(),
            is_ai: false,
        },
        crate::game::PlayerInit {
            id: 30,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Charlie".to_string(),
            color: "#00f".to_string(),
            is_ai: true,
        },
    ];
    let game = crate::game::Game::new(&players, 0x2468_ace0);
    let start = game.start_payload();

    for (index, player) in players.iter().enumerate() {
        let payload = &start.players[index];
        assert_eq!(payload.id, player.id);
        assert_eq!(payload.team_id, player.team_id);
        assert_eq!(
            (payload.start_tile_x, payload.start_tile_y),
            game.state.map.starts[index]
        );
    }
}

#[test]
fn replay_reconstruction_preserves_team_aware_start_assignment() {
    let players = vec![
        crate::game::PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Alpha".to_string(),
            color: "#f00".to_string(),
            is_ai: false,
        },
        crate::game::PlayerInit {
            id: 2,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Bravo".to_string(),
            color: "#0f0".to_string(),
            is_ai: false,
        },
        crate::game::PlayerInit {
            id: 3,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Charlie".to_string(),
            color: "#00f".to_string(),
            is_ai: true,
        },
        crate::game::PlayerInit {
            id: 4,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Delta".to_string(),
            color: "#ff0".to_string(),
            is_ai: true,
        },
    ];
    let live = crate::game::Game::new(&players, 0x1357_9bdf);
    let replay = crate::game::Game::new_for_replay_with_starting_resources(
        &live.player_inits(),
        live.starting_steel(),
        live.starting_oil(),
        live.seed(),
    );

    assert_eq!(replay.state.map.starts, live.state.map.starts);
    assert_eq!(
        replay.state.map.expansion_sites,
        live.state.map.expansion_sites
    );
}

fn start_players(players: &[(u32, u32)]) -> Vec<(u32, u32)> {
    players.to_vec()
}

fn tile_distance_sq(a: Tile, b: Tile) -> u64 {
    let dx = i64::from(a.0) - i64::from(b.0);
    let dy = i64::from(a.1) - i64::from(b.1);
    (dx * dx + dy * dy) as u64
}

fn synthetic_six_start_map_json() -> String {
    let rows = vec![".".repeat(100); 100];
    format!(
        r#"{{
          "version": 2,
          "name": "six-start",
          "description": "synthetic six start map",
          "_design": "test-only six-start authored layout",
          "terrain": {},
          "sites": [
            {{"id": "main_a", "kind": "main", "x": 16, "y": 16}},
            {{"id": "nat_a", "kind": "natural", "x": 16, "y": 28}},
            {{"id": "main_b", "kind": "main", "x": 16, "y": 36}},
            {{"id": "nat_b", "kind": "natural", "x": 16, "y": 48}},
            {{"id": "main_c", "kind": "main", "x": 36, "y": 16}},
            {{"id": "nat_c", "kind": "natural", "x": 48, "y": 16}},
            {{"id": "main_d", "kind": "main", "x": 84, "y": 84}},
            {{"id": "nat_d", "kind": "natural", "x": 84, "y": 72}},
            {{"id": "main_e", "kind": "main", "x": 84, "y": 64}},
            {{"id": "nat_e", "kind": "natural", "x": 84, "y": 52}},
            {{"id": "main_f", "kind": "main", "x": 64, "y": 84}},
            {{"id": "nat_f", "kind": "natural", "x": 52, "y": 84}}
          ],
          "layouts": [
            {{
              "id": "six",
              "playerCount": 6,
              "slots": [
                {{"main": "main_a", "natural": "nat_a"}},
                {{"main": "main_b", "natural": "nat_b"}},
                {{"main": "main_c", "natural": "nat_c"}},
                {{"main": "main_d", "natural": "nat_d"}},
                {{"main": "main_e", "natural": "nat_e"}},
                {{"main": "main_f", "natural": "nat_f"}}
              ]
            }}
          ]
        }}"#,
        serde_json::to_string(&rows).unwrap()
    )
}
