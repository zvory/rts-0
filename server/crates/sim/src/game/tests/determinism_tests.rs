use super::*;

/// A one-player sandbox with no commands must still be deterministic: fog, supply, and the
/// spatial index rebuild identically every tick, and replaying the empty command log
/// reproduces the same final snapshot.
#[test]
fn no_commands_one_player_is_deterministic() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new(&players, 0x1234_5678);

    let mut event_log = Vec::new();
    for tick in 1..=300 {
        for (player_id, events) in game.tick() {
            for event in events {
                event_log.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert!(
        event_log.is_empty(),
        "a one-player sandbox with no commands should emit no events"
    );

    let replay = super::replay::replay_commands(
        &players,
        game.command_log(),
        game.tick_count(),
        game.seed(),
        game.starting_loadouts(),
    )
    .expect("one-player no-commands replay should succeed");
    assert_eq!(replay.events, event_log);
    assert_eq!(replay.final_snapshots[0].snapshot, game.snapshot_for(1));
}
