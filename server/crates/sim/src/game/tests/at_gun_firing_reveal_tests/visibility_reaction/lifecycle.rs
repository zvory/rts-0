use super::*;

fn game_with_active_reveal_gate() -> (Game, u32, u32) {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let hidden_pos = game.state.map.tile_center(30, 30);
    let counter_pos = game.state.map.tile_center(5, 5);
    let counter = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Tank, counter_pos.0, counter_pos.1)
        .expect("counter");
    let shooter = game
        .state
        .entities
        .spawn_unit(2, EntityKind::MachineGunner, hidden_pos.0, hidden_pos.1)
        .expect("hidden shooter");
    let target = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Worker, hidden_pos.0, hidden_pos.1)
        .expect("colocated target");
    let teams = game.team_relations();
    firing_reveal::record_global_firing_reveals_for_enemy_players(
        &mut game.state.firing_reveals,
        &[1, 2],
        &teams,
        2,
        shooter,
        0,
        config::TICK_HZ,
    );
    refresh_visibility_for_test(&mut game);
    assert!(!game.state.firing_reveals.is_empty());
    let episode = game
        .state
        .fog
        .firing_reveal_only_source_at_world(1, hidden_pos.0, hidden_pos.1)
        .expect("reveal episode");
    let weapon = crate::rules::combat::WeaponKind::TankCannon;
    assert!(!game
        .state
        .entities
        .get_mut(counter)
        .expect("counter")
        .weapon_firing_reveal_reaction_ready(weapon, target, episode, 0, config::TICK_HZ));
    (game, shooter, target)
}

#[test]
fn lab_delete_prunes_firing_reveal_sources_and_reaction_gates_before_checkpointing() {
    let (mut game, shooter, _target) = game_with_active_reveal_gate();

    game.apply_lab_op(crate::game::lab::LabOp::DeleteEntity { entity_id: shooter })
        .expect("lab delete");
    assert!(game.state.firing_reveals.is_empty());
    game.checkpoint_payload_text_for_test()
        .expect("removed reveal sources must not leave checkpoint-invalid provenance");
}

#[test]
fn lab_delete_prunes_reaction_gates_with_removed_targets_before_checkpointing() {
    let (mut game, _shooter, target) = game_with_active_reveal_gate();

    game.apply_lab_op(crate::game::lab::LabOp::DeleteEntity { entity_id: target })
        .expect("lab delete");
    assert!(!game.state.firing_reveals.is_empty());
    game.checkpoint_payload_text_for_test()
        .expect("removed targets must not leave checkpoint-invalid reaction gates");
}

#[test]
fn elimination_prunes_firing_reveal_sources_and_reaction_gates_before_checkpointing() {
    let (mut game, _shooter, _target) = game_with_active_reveal_gate();

    game.eliminate(2);
    assert!(game.state.firing_reveals.is_empty());
    game.checkpoint_payload_text_for_test()
        .expect("eliminated reveal sources must not leave checkpoint-invalid provenance");
}
