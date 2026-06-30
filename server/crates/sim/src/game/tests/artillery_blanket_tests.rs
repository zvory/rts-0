use super::fixtures::*;
use super::*;

#[test]
fn artillery_blanket_fire_queue_is_terminal() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = game.map.tile_center(38, 10);
    let artillery = game
        .entities
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

    let entity = game.entities.get(artillery).expect("artillery exists");
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
    let initial_steel = game.players[0].steel;
    let pos = game.map.tile_center(10, 10);
    let target = game.map.tile_center(38, 10);
    let artillery = game
        .entities
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

    let entity = game.entities.get(artillery).expect("artillery exists");
    assert!(matches!(
        entity.weapon_setup(),
        WeaponSetup::Packed | WeaponSetup::SettingUp { .. }
    ));
    let Order::ArtilleryBlanketFire(order) = entity.order() else {
        panic!("packed blanket fire should store a Blanket Fire order");
    };
    let center = (order.intent.x, order.intent.y);
    assert_eq!(game.players[0].steel, initial_steel);
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
        game.players[0].steel <= initial_steel - config::ARTILLERY_AMMO_COST_STEEL,
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

fn collect_blanket_fire_targets(ballistic_tables: bool) -> Vec<(u32, u32)> {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    if ballistic_tables {
        game.players[0]
            .upgrades
            .insert(crate::game::upgrade::UpgradeKind::BallisticTables);
    }
    let pos = game.map.tile_center(10, 10);
    let target = game.map.tile_center(38, 10);
    let artillery = game
        .entities
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
    let entity = game
        .entities
        .get_mut(artillery)
        .expect("artillery should exist");
    let facing = (target.1 - entity.pos_y).atan2(target.0 - entity.pos_x);
    entity.set_weapon_setup(WeaponSetup::Deployed);
    entity.set_emplacement_facing(Some(facing));
    entity.set_desired_weapon_facing(facing);
}
