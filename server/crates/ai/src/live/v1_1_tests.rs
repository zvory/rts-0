//! Jeff's AI V1.1 in the live adapter.

use super::*;

#[test]
fn jeffs_ai_v1_1_sends_defenders_home_once_their_fight_is_quiet() {
    use crate::ai_core::observation::{AiEconomy, AiEntityState, AiEntitySummary, AiMapSummary};
    let rifleman = |id, x, owner| AiEntitySummary {
        id,
        owner,
        kind: EntityKind::Rifleman,
        x,
        y: 320.0,
        hp: 45,
        state: AiEntityState::Idle,
        is_complete: true,
        production_queue_len: None,
        production_kind: None,
        latched_node: None,
        target_id: None,
        free_for_combat: true,
    };
    let fighting = AiObservation {
        player_id: 1,
        tick: 0,
        map: AiMapSummary {
            width: 64,
            height: 64,
            tile_size: 32,
        },
        economy: AiEconomy {
            steel: 0,
            oil: 0,
            supply_used: 0,
            supply_cap: 100,
        },
        own_start_tile: (8, 8),
        players: Vec::new(),
        owned: vec![rifleman(43, 320.0, 1)],
        resources: Vec::new(),
        visible_allies: Vec::new(),
        visible_enemies: vec![rifleman(90, 480.0, 2)],
        ability_states: Vec::new(),
        smokes: Vec::new(),
        visible_tank_traps: Vec::new(),
        pending_builds: Vec::new(),
        upgrades: Vec::new(),
    };
    let quiet = AiObservation {
        visible_enemies: Vec::new(),
        ..fighting.clone()
    };
    for profile in [JEFFS_AI_ID, JEFFS_AI_V1_1_ID] {
        let mut ai = AiController::with_profile_id(1, profile);
        ai.active_attack_units.insert(43, 0);
        // An enemy five tiles away: the fight goes on.
        ai.prune_combat_memory(&fighting, 200);
        assert!(ai.active_attack_units.contains_key(&43));
        // Quiet from tick 300: V1.1 sends the unit back after four seconds; V1.0 holds it.
        ai.prune_combat_memory(&quiet, 300);
        ai.prune_combat_memory(&quiet, 419);
        assert!(ai.active_attack_units.contains_key(&43));
        ai.prune_combat_memory(&quiet, 420);
        assert_eq!(
            ai.active_attack_units.contains_key(&43),
            profile == JEFFS_AI_ID,
            "{profile}"
        );
    }
}
