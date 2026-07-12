use super::fixtures::*;
use super::*;

#[test]
fn artillery_blanket_fire_queue_is_terminal() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(38, 10);
    let artillery = game.state.entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    deploy_artillery_toward(&mut game, artillery, target);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::BlanketFire,
            units: vec![artillery],
            x: Some(target.0),
            y: Some(target.1),
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Move {
            units: vec![artillery],
            x: target.0 + 64.0,
            y: target.1,
            queued: true,
        },
    );
    game.tick();

    let entity = game.state.entities.get(artillery).expect("artillery exists");
    assert!(matches!(entity.order(), Order::ArtilleryBlanketFire(_)));
    assert!(
        entity.queued_orders().is_empty(),
        "later queued move should not be accepted behind terminal Blanket Fire"
    );
}

#[test]
fn packed_artillery_blanket_fire_auto_sets_up_and_samples_inside_radius() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.state.players[0].steel;
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(38, 10);
    let artillery = game.state.entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::BlanketFire,
            units: vec![artillery],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
    let events = game.tick();

    let entity = game.state.entities.get(artillery).expect("artillery exists");
    assert!(matches!(
        entity.weapon_setup(),
        WeaponSetup::Packed | WeaponSetup::SettingUp { .. }
    ));
    let Order::ArtilleryBlanketFire(order) = entity.order() else {
        panic!("packed blanket fire should store a Blanket Fire order");
    };
    let center = (order.intent.x, order.intent.y);
    assert_eq!(game.state.players[0].steel, initial_steel);
    assert!(
        events
            .iter()
            .flat_map(|(_, events)| events)
            .all(|event| !matches!(event, Event::ArtilleryTarget { .. })),
        "packed blanket fire should not emit a target marker before deployment"
    );

    let mut sampled_target = None;
    for _ in 0..=(config::ARTILLERY_SETUP_TICKS as u32 + 4) {
        for (pid, events) in game.tick() {
            if pid == 1 {
                sampled_target = events.iter().find_map(|event| match event {
                    Event::ArtilleryTarget { from, x, y, .. } if *from == artillery => {
                        Some((*x, *y))
                    }
                    _ => None,
                });
            }
        }
        if sampled_target.is_some() {
            break;
        }
    }
    let sampled = sampled_target.expect("auto-setup blanket fire should eventually fire");
    let radius_px = config::ARTILLERY_BLANKET_RADIUS_TILES * config::TILE_SIZE as f32;
    let dx = sampled.0 - center.0;
    let dy = sampled.1 - center.1;
    assert!(
        dx * dx + dy * dy <= (radius_px + 0.5) * (radius_px + 0.5),
        "sampled target should stay inside the blanket radius"
    );
    assert!(
        game.state.players[0].steel <= initial_steel - config::ARTILLERY_AMMO_COST_STEEL,
        "auto-setup blanket fire should spend ammo only once the gun is deployed"
    );
}

#[test]
fn blanket_fire_sampling_is_replay_stable_and_ignores_ballistic_tables() {
    let baseline = collect_blanket_fire_targets(false);
    let with_ballistic_tables = collect_blanket_fire_targets(true);

    assert_eq!(
        baseline, with_ballistic_tables,
        "Blanket Fire sampling should not tighten or otherwise change with Ballistic Tables"
    );
    assert!(
        baseline.windows(2).any(|pair| pair[0] != pair[1]),
        "deterministic blanket sequence should still vary between shots"
    );
}

#[test]
fn queued_blanket_fire_mixed_selection_locks_each_artillery_and_keeps_rifle_queueable() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let min_px = config::ARTILLERY_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let first_pos = game.state.map.tile_center(10, 10);
    let second_pos = game.state.map.tile_center(10, 12);
    let raw_click = (first_pos.0 + min_px - 8.0, first_pos.1);
    let move_target = game.state.map.tile_center(18, 18);
    let first_artillery = game.state.entities
        .spawn_unit(1, EntityKind::Artillery, first_pos.0, first_pos.1)
        .expect("first artillery should spawn");
    let second_artillery = game.state.entities
        .spawn_unit(1, EntityKind::Artillery, second_pos.0, second_pos.1)
        .expect("second artillery should spawn");
    let rifle = game.state.entities
        .spawn_unit(1, EntityKind::Rifleman, first_pos.0, first_pos.1 + 192.0)
        .expect("rifleman should spawn");
    deploy_artillery_toward(&mut game, first_artillery, raw_click);
    deploy_artillery_toward(&mut game, second_artillery, raw_click);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::BlanketFire,
            units: vec![first_artillery, rifle, second_artillery],
            x: Some(raw_click.0),
            y: Some(raw_click.1),
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Move {
            units: vec![first_artillery, rifle, second_artillery],
            x: move_target.0,
            y: move_target.1,
            queued: true,
        },
    );
    game.tick();

    let first_entity = game.state.entities
        .get(first_artillery)
        .expect("first artillery should exist");
    let second_entity = game.state.entities
        .get(second_artillery)
        .expect("second artillery should exist");
    let Order::ArtilleryBlanketFire(first_order) = first_entity.order() else {
        panic!("first artillery should promote queued Blanket Fire");
    };
    let Order::ArtilleryBlanketFire(second_order) = second_entity.order() else {
        panic!("second artillery should promote queued Blanket Fire");
    };

    assert!(
        first_entity.queued_orders().is_empty() && second_entity.queued_orders().is_empty(),
        "later queued movement must not append behind terminal Blanket Fire for either gun"
    );
    assert!(
        (first_order.intent.x - second_order.intent.x).abs() > 0.5
            || (first_order.intent.y - second_order.intent.y).abs() > 0.5,
        "different artillery origins should store different locked Blanket Fire centers"
    );
    let first_distance = ((first_order.intent.x - first_pos.0).powi(2)
        + (first_order.intent.y - first_pos.1).powi(2))
    .sqrt();
    let second_distance = ((second_order.intent.x - second_pos.0).powi(2)
        + (second_order.intent.y - second_pos.1).powi(2))
    .sqrt();
    assert!(
        (first_distance - min_px).abs() < 0.001 && (second_distance - min_px).abs() < 0.001,
        "inside-minimum mixed Blanket Fire clicks should lock each gun to its own range floor"
    );

    let rifle_entity = game.state.entities.get(rifle).expect("rifleman should exist");
    assert!(
        matches!(rifle_entity.order(), Order::Move(_)),
        "non-artillery in the mixed selection should still accept the later queued move"
    );
}

fn collect_blanket_fire_targets(ballistic_tables: bool) -> Vec<(u32, u32)> {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    if ballistic_tables {
        game.state.players[0]
            .upgrades
            .insert(crate::game::upgrade::UpgradeKind::BallisticTables);
    }
    let pos = game.state.map.tile_center(10, 10);
    let target = game.state.map.tile_center(38, 10);
    let artillery = game.state.entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    deploy_artillery_toward(&mut game, artillery, target);
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::BlanketFire,
            units: vec![artillery],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );

    let mut targets = Vec::new();
    for _ in 0..=(config::ARTILLERY_RELOAD_TICKS * 3 + 8) {
        for (pid, events) in game.tick() {
            if pid != 1 {
                continue;
            }
            for event in events {
                if let Event::ArtilleryTarget { from, x, y, .. } = event {
                    if from == artillery {
                        targets.push((x.to_bits(), y.to_bits()));
                    }
                }
            }
        }
        if targets.len() >= 3 {
            return targets;
        }
    }
    panic!("expected three deterministic Blanket Fire targets, got {targets:?}");
}

fn deploy_artillery_toward(game: &mut Game, artillery: u32, target: (f32, f32)) {
    let entity = game.state.entities
        .get_mut(artillery)
        .expect("artillery should exist");
    let facing = (target.1 - entity.pos_y).atan2(target.0 - entity.pos_x);
    entity.set_weapon_setup(WeaponSetup::Deployed);
    entity.set_emplacement_facing(Some(facing));
    entity.set_desired_weapon_facing(facing);
}
