use super::*;

/// Adding an AI identity must not perturb a human-only game's construction.
#[test]
fn human_only_match_has_no_ai_players() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let game = Game::new(&players, 0x1234_5678);
    assert!(game.state.players.iter().all(|player| !player.is_ai));
}

#[test]
fn replay_games_preserve_ai_identity_without_controllers() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Computer".into(),
        color: "#fff".into(),
        is_ai: true,
    }];
    let game = Game::new_without_ai_controllers(&players, 0x1234_5678);

    assert!(
        game.state.players
            .iter()
            .any(|player| player.id == 1 && player.is_ai),
        "replays must preserve AI identity for deterministic simulation rules"
    );
    assert!(
        game.player_inits()
            .iter()
            .any(|player| player.id == 1 && player.is_ai),
        "replay artifacts must serialize the original AI identity"
    );
}
