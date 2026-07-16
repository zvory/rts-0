use super::*;

#[test]
fn lab_delete_prunes_firing_reveal_sources_before_checkpointing() {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let hidden_pos = game.state.map.tile_center(30, 30);
    let shooter = game
        .state
        .entities
        .spawn_unit(2, EntityKind::MachineGunner, hidden_pos.0, hidden_pos.1)
        .expect("hidden shooter");
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

    game.apply_lab_op(crate::game::lab::LabOp::DeleteEntity { entity_id: shooter })
        .expect("lab delete");
    assert!(game.state.firing_reveals.is_empty());
    game.checkpoint_payload_text_for_test()
        .expect("removed reveal sources must not leave checkpoint-invalid provenance");
}
