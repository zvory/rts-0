use super::*;

fn meth_movement_fixture() -> (Game, u32, (f32, f32)) {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x5150_0702);
    for tile in &mut game.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }

    let start = game.map.tile_center(20, 20);
    let goal = (start.0 + 500.0, start.1);
    let rifleman = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("rifleman should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    game.assert_invariants();

    game.enqueue(
        1,
        Command::Move {
            units: vec![rifleman],
            x: goal.0,
            y: goal.1,
            queued: false,
        },
    );

    (game, rifleman, goal)
}

fn entity_pos(game: &Game, id: u32) -> (f32, f32) {
    let entity = game.entities.get(id).expect("entity should exist");
    (entity.pos_x, entity.pos_y)
}

fn moved_distance(from: (f32, f32), to: (f32, f32)) -> f32 {
    let dx = to.0 - from.0;
    let dy = to.1 - from.1;
    (dx * dx + dy * dy).sqrt()
}

fn next_moving_step(game: &mut Game, id: u32) -> f32 {
    for _ in 0..3 {
        let before = entity_pos(game, id);
        game.tick();
        let moved = moved_distance(before, entity_pos(game, id));
        if moved > 0.0 {
            return moved;
        }
    }
    0.0
}

#[test]
fn removed_methamphetamines_clears_persistent_rifleman_speed() {
    let (mut game, rifleman, _goal) = meth_movement_fixture();
    let base_speed = config::unit_stats(EntityKind::Rifleman)
        .expect("rifleman stats")
        .speed;

    game.players[0]
        .upgrades
        .insert(crate::game::upgrade::UpgradeKind::Methamphetamines);
    let boosted_step = next_moving_step(&mut game, rifleman);
    assert!(
        (boosted_step - base_speed * config::RIFLEMAN_CHARGE_SPEED_MULTIPLIER).abs() < 0.01,
        "researched Methamphetamines should boost rifleman speed, moved {boosted_step:.3}px"
    );
    assert!(
        game.entities
            .get(rifleman)
            .expect("rifleman should exist")
            .charge_ticks()
            > config::RIFLEMAN_CHARGE_TICKS,
        "Methamphetamines should seed the persistent charge state"
    );

    game.players[0]
        .upgrades
        .remove(&crate::game::upgrade::UpgradeKind::Methamphetamines);
    let normal_step = next_moving_step(&mut game, rifleman);
    assert!(
        (normal_step - base_speed).abs() < 0.01,
        "removed Methamphetamines should immediately return riflemen to base speed, moved {normal_step:.3}px"
    );
    assert_eq!(
        game.entities
            .get(rifleman)
            .expect("rifleman should exist")
            .charge_ticks(),
        0,
        "removed Methamphetamines should clear the persistent charge state"
    );
}
