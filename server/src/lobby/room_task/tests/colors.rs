use super::support::*;

fn room() -> RoomTask {
    RoomTask::new(
        "preferred-colors".into(),
        RoomMode::Normal,
        None,
        false,
        DrainHandle::default(),
    )
}

#[test]
fn preferred_color_reserves_winners_before_ordered_fallbacks_and_ai() {
    let mut task = room();
    for id in 1..=4 {
        add_test_room_player(&mut task, id, false);
    }
    task.color_preferences
        .insert(1, (Some(PLAYER_PALETTE[2].into()), 10));
    task.color_preferences
        .insert(2, (Some(PLAYER_PALETTE[2].into()), 20));
    task.color_preferences
        .insert(3, (Some(PLAYER_PALETTE[0].into()), 30));
    task.resolve_preferred_colors();
    assert_eq!(task.players[&2].color, PLAYER_PALETTE[2]);
    assert_eq!(task.players[&3].color, PLAYER_PALETTE[0]);
    assert_eq!(task.players[&1].color, PLAYER_PALETTE[1]);
    assert_eq!(task.players[&4].color, PLAYER_PALETTE[3]);
    assert_eq!(task.ai_color(0), PLAYER_PALETTE[4]);
    // Repeated broadcasts cannot reroll the conflict.
    for _ in 0..5 {
        task.broadcast_lobby();
    }
    assert_eq!(task.players[&2].color, PLAYER_PALETTE[2]);
}

#[test]
fn preferred_color_validation_spectators_and_priority_are_stable() {
    let mut task = room();
    add_test_room_player(&mut task, 1, false);
    add_test_room_player(&mut task, 2, false);
    add_test_room_spectator(&mut task, 3);
    task.on_set_preferred_color(1, Some(PLAYER_PALETTE[3].into()));
    let priority = task.color_preferences[&1].1;
    task.on_set_preferred_color(1, Some("#123456".into()));
    assert_eq!(task.players[&1].color, PLAYER_PALETTE[3]);
    task.on_set_preferred_color(1, Some(PLAYER_PALETTE[4].into()));
    assert_eq!(task.color_preferences[&1].1, priority);
    task.on_set_preferred_color(3, Some(PLAYER_PALETTE[0].into()));
    assert_eq!(task.players[&2].color, PLAYER_PALETTE[0]);
    task.match_countdown_deadline = Some(TokioInstant::now());
    task.on_set_preferred_color(1, Some(PLAYER_PALETTE[5].into()));
    assert_eq!(task.players[&1].color, PLAYER_PALETTE[4]);
    task.match_countdown_deadline = None;
    task.on_leave(1);
    assert!(!task.color_preferences.contains_key(&1));
}

#[test]
fn preferred_color_conflict_can_be_won_by_either_join_position() {
    for winner in [1, 2] {
        let mut task = room();
        for id in 1..=2 {
            add_test_room_player(&mut task, id, false);
            task.color_preferences.insert(
                id,
                (
                    Some(PLAYER_PALETTE[7].into()),
                    if id == winner { 100 } else { 1 },
                ),
            );
        }
        task.resolve_preferred_colors();
        assert_eq!(task.players[&winner].color, PLAYER_PALETTE[7]);
        assert_eq!(task.players[&(3 - winner)].color, PLAYER_PALETTE[0]);
    }
}

#[test]
fn preferred_color_saved_during_match_applies_only_in_next_lobby() {
    let mut task = room();
    add_test_room_player(&mut task, 1, false);
    add_test_room_player(&mut task, 2, false);
    task.phase = Phase::InGame(Box::new(replay_test_game(&replay_test_players(2), 1)));
    let original = task.players[&1].color.clone();
    task.on_set_preferred_color(1, Some(PLAYER_PALETTE[7].into()));
    assert_eq!(task.players[&1].color, original);
    task.phase = Phase::Lobby;
    task.broadcast_lobby();
    assert_eq!(task.players[&1].color, PLAYER_PALETTE[7]);
}
