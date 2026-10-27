use super::*;
use rts_sim::game::PlayerInit;

#[test]
fn composition_selects_once_and_keeps_the_requested_identity() {
    for member in [PRESSURE_AI_ID, RUSH_AI_ID, JEFFS_AI_ID] {
        let composition = composition_by_id(AI_2_3_ID).unwrap();
        let seed = (0..128)
            .find(|seed| composition.select(*seed, 1) == Some(member))
            .unwrap();
        let mut controller = AiController::new(1);
        assert_eq!(controller.profile_id(), AI_2_3_ID);
        assert_eq!(controller.selected_profile_id(), None);
        controller.select_for_match(seed);
        assert_eq!(controller.selected_profile_id(), Some(member));
        for other_seed in 0..128 {
            controller.select_for_match(other_seed);
            assert_eq!(controller.selected_profile_id(), Some(member));
        }
        assert_eq!(controller.profile_id(), AI_2_3_ID);
        assert!(controller.custom_strategy.is_none());
    }
    assert!(is_player_live_profile_id(AI_2_3_ID));
    assert!(!is_player_live_profile_id(AI_2_1_ID));
    assert_eq!(live_profile_label("default"), "AI 2.3");
}

#[test]
fn all_composition_members_emit_the_same_commands_as_explicit_profiles() {
    let composition = composition_by_id(AI_2_3_ID).unwrap();
    for member in [PRESSURE_AI_ID, RUSH_AI_ID, JEFFS_AI_ID] {
        let seed = (0..128)
            .find(|seed| composition.select(*seed, 1) == Some(member))
            .unwrap();
        let players = [
            PlayerInit {
                id: 1,
                name: "Candidate".into(),
                color: "#ffffff".into(),
                team_id: 1,
                is_ai: true,
                faction_id: "kriegsia".into(),
            },
            PlayerInit {
                id: 2,
                name: "Enemy".into(),
                color: "#000000".into(),
                team_id: 2,
                is_ai: false,
                faction_id: "kriegsia".into(),
            },
        ];
        let mut mixed_game = Game::new(&players, seed);
        let mut explicit_game = Game::new(&players, seed);
        let mut mixed = [AiController::with_profile_id(1, AI_2_3_ID)];
        let mut explicit = [AiController::with_profile_id(1, member)];
        for _ in 0..900 {
            CanonicalAiTickDriver::run(&mut mixed_game, &mut mixed, AiAlivePolicy::Normal);
            CanonicalAiTickDriver::run(&mut explicit_game, &mut explicit, AiAlivePolicy::Normal);
            mixed_game.tick();
            explicit_game.tick();
        }
        assert_eq!(mixed[0].selected_profile_id(), Some(member));
        assert!(!mixed_game.command_log().is_empty());
        let trace = mixed[0]
            .latest_decision_trace()
            .expect("composition decision trace");
        assert_eq!(trace.profile_id, AI_2_3_ID);
        assert!(trace
            .lines
            .iter()
            .any(|line| line == &format!("composition=ai_2_3 selected={member}")));
        assert_eq!(
            mixed_game.command_log(),
            explicit_game.command_log(),
            "selected {member}"
        );
        assert_eq!(mixed_game.scores(), explicit_game.scores());
    }
}
