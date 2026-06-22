use super::fixtures::*;
use super::*;

#[test]
fn scout_car_smoke_requires_no_steelworks() {
    let (mut game, scout, _target, _) = smoke_command_fixture();
    let target = game.map.tile_center(12, 8);
    assert!(
        !game
            .entities
            .iter()
            .any(|e| e.owner == 1 && e.kind == EntityKind::Steelworks),
        "fixture should not contain Steelworks"
    );

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::Smoke,
            units: vec![scout],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
    game.tick();

    assert!(
        game.entities
            .get(scout)
            .expect("scout should exist")
            .ability_cooldown_ticks(ability::AbilityKind::Smoke)
            > 0,
        "Scout Car smoke should be available before Steelworks and start cooldown"
    );
}

#[test]
fn command_car_requires_rd_unlock_then_trains_at_factory() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game =
        Game::new_for_replay_with_starting_resources(&players, 5_000, 5_000, 0x5150_0701);
    for id in game.entities.ids() {
        game.entities.remove(id);
    }
    let city_centre_pos = game.map.tile_center(8, 12);
    let research_pos = game.map.tile_center(8, 8);
    let factory_pos = game.map.tile_center(12, 8);
    game.entities
        .spawn_building(
            1,
            EntityKind::CityCentre,
            city_centre_pos.0,
            city_centre_pos.1,
            true,
        )
        .expect("city centre should spawn");
    let research_complex = game
        .entities
        .spawn_building(
            1,
            EntityKind::ResearchComplex,
            research_pos.0,
            research_pos.1,
            true,
        )
        .expect("research complex should spawn");
    let factory = game
        .entities
        .spawn_building(1, EntityKind::Factory, factory_pos.0, factory_pos.1, true)
        .expect("factory should spawn");

    game.enqueue(
        1,
        Command::Research {
            building: research_complex,
            upgrade: crate::game::upgrade::UpgradeKind::CommandCarUnlock,
        },
    );
    game.tick();
    assert!(
        game.entities
            .get(research_complex)
            .expect("research complex")
            .research_queue()
            .is_empty(),
        "Command Car research should require Tank Production first"
    );

    game.players[0]
        .upgrades
        .insert(crate::game::upgrade::UpgradeKind::TankUnlock);
    game.enqueue(
        1,
        Command::Research {
            building: research_complex,
            upgrade: crate::game::upgrade::UpgradeKind::CommandCarUnlock,
        },
    );
    for _ in 0..=crate::config::COMMAND_CAR_UNLOCK_RESEARCH_TICKS {
        game.tick();
    }
    assert!(game.players[0]
        .upgrades
        .contains(&crate::game::upgrade::UpgradeKind::CommandCarUnlock));

    game.enqueue(
        1,
        Command::Train {
            building: factory,
            unit: EntityKind::CommandCar,
        },
    );
    for _ in 0..=crate::config::TICK_HZ * 15 {
        game.tick();
    }

    assert!(
        game.entities
            .iter()
            .any(|e| e.owner == 1 && e.kind == EntityKind::CommandCar),
        "Vehicle Works should train Command Cars after R&D unlock"
    );
}

#[test]
fn breakthrough_applies_owned_nonstacking_speed_status_and_cooldown() {
    let (mut game, _scout, _target, _) = smoke_command_fixture();
    for id in game.entities.ids() {
        game.entities.remove(id);
    }
    let car_pos = game.map.tile_center(8, 8);
    let nearby_pos = game.map.tile_center(10, 8);
    let far_pos = game.map.tile_center(20, 8);
    let command_car = game
        .entities
        .spawn_unit(1, EntityKind::CommandCar, car_pos.0, car_pos.1)
        .expect("command car should spawn");
    let nearby = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, nearby_pos.0, nearby_pos.1)
        .expect("nearby rifle should spawn");
    let nearby_command_car = game
        .entities
        .spawn_unit(1, EntityKind::CommandCar, nearby_pos.0 + 16.0, nearby_pos.1)
        .expect("nearby command car should spawn");
    let far = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, far_pos.0, far_pos.1)
        .expect("far rifle should spawn");
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::Breakthrough,
            units: vec![command_car],
            x: None,
            y: None,
            queued: false,
        },
    );
    game.tick();

    let car = game.entities.get(command_car).expect("command car");
    assert!(car.ability_cooldown_ticks(ability::AbilityKind::Breakthrough) > 0);
    assert!(car.breakthrough_ticks() > 0);
    assert!(car.breakthrough_aura_ticks() > 0);
    assert!(
        game.entities
            .get(nearby)
            .expect("nearby unit")
            .breakthrough_ticks()
            > 0
    );
    let nearby_car = game
        .entities
        .get(nearby_command_car)
        .expect("nearby command car");
    assert!(nearby_car.breakthrough_ticks() > 0);
    assert_eq!(
        nearby_car.breakthrough_aura_ticks(),
        0,
        "nearby buffed Command Cars should not become aura origins"
    );
    assert_eq!(
        game.entities
            .get(far)
            .expect("far unit")
            .breakthrough_ticks(),
        0
    );

    let owner_snapshot = game.snapshot_for(1);
    let caster_view = owner_snapshot
        .entities
        .iter()
        .find(|entity| entity.id == command_car)
        .expect("owner should see caster");
    let breakthrough_affordance = caster_view
        .abilities
        .iter()
        .find(|ability| ability.ability == crate::protocol::abilities::BREAKTHROUGH)
        .expect("caster should project Breakthrough affordance");
    assert_eq!(
        breakthrough_affordance.expires_in,
        Some(car.breakthrough_aura_ticks()),
        "caster affordance should expose active aura duration"
    );
    let nearby_car_view = owner_snapshot
        .entities
        .iter()
        .find(|entity| entity.id == nearby_command_car)
        .expect("owner should see nearby command car");
    assert!(
        nearby_car_view
            .abilities
            .iter()
            .find(|ability| ability.ability == crate::protocol::abilities::BREAKTHROUGH)
            .is_none_or(|ability| ability.expires_in.is_none()),
        "buffed non-caster Command Cars should not project aura duration"
    );

    let remaining = game
        .entities
        .get(nearby)
        .expect("nearby unit")
        .breakthrough_ticks();
    if let Some(e) = game.entities.get_mut(nearby) {
        e.start_breakthrough(1);
    }
    assert_eq!(
        game.entities
            .get(nearby)
            .expect("nearby unit")
            .breakthrough_ticks(),
        remaining,
        "shorter overlapping Breakthrough should not reduce an active buff"
    );
}

#[test]
fn breakthrough_smoke_synergy_speeds_units_more() {
    let (mut game, _scout, _target, _) = smoke_command_fixture();
    for id in game.entities.ids() {
        game.entities.remove(id);
    }
    let plain_pos = game.map.tile_center(8, 5);
    let smoke_pos = game.map.tile_center(8, 10);
    let goal_plain = game.map.tile_center(20, 5);
    let goal_smoke = game.map.tile_center(20, 10);
    let plain = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, plain_pos.0, plain_pos.1)
        .expect("plain rifle should spawn");
    let smoked = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, smoke_pos.0, smoke_pos.1)
        .expect("smoked rifle should spawn");
    game.smokes.schedule(
        smoke_pos.0,
        smoke_pos.1,
        crate::config::SMOKE_CLOUD_RADIUS_TILES,
        crate::config::SMOKE_CLOUD_DURATION_TICKS,
        game.tick,
    );
    game.smokes.spawn_due(game.tick);
    for id in [plain, smoked] {
        if let Some(e) = game.entities.get_mut(id) {
            e.start_breakthrough(crate::config::BREAKTHROUGH_DURATION_TICKS);
        }
    }
    if let Some(e) = game.entities.get_mut(smoked) {
        e.mark_in_smoke_for_breakthrough(crate::config::BREAKTHROUGH_RECENT_SMOKE_TICKS);
    }
    game.enqueue(
        1,
        Command::Move {
            units: vec![plain],
            x: goal_plain.0,
            y: goal_plain.1,
            queued: false,
        },
    );
    game.enqueue(
        1,
        Command::Move {
            units: vec![smoked],
            x: goal_smoke.0,
            y: goal_smoke.1,
            queued: false,
        },
    );
    for _ in 0..10 {
        game.tick();
    }
    let plain_dx = game.entities.get(plain).expect("plain").pos_x - plain_pos.0;
    let smoke_dx = game.entities.get(smoked).expect("smoked").pos_x - smoke_pos.0;
    assert!(
        smoke_dx > plain_dx,
        "unit in smoke should receive the stronger Breakthrough multiplier ({smoke_dx} <= {plain_dx})"
    );
}
