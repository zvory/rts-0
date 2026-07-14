use super::fixtures::*;
use super::*;

#[test]
fn scout_car_smoke_requires_no_steelworks() {
    let (mut game, scout, _target, _) = smoke_command_fixture();
    let target = game.state.map.tile_center(12, 8);
    assert!(
        !game
            .state
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
        matches!(
            game.state
                .entities
                .get(scout)
                .expect("scout should exist")
                .ability_uses_remaining(ability::AbilityKind::Smoke),
            Some(1)
        ),
        "Scout Car smoke should be available before Steelworks and spend one use"
    );
    assert_eq!(
        game.state
            .entities
            .get(scout)
            .expect("scout should exist")
            .ability_cooldown_ticks(ability::AbilityKind::Smoke),
        0,
        "Scout Car smoke should not start a cooldown"
    );
}

#[test]
fn smoke_plus_increases_scout_car_smoke_radius_by_half_and_doubles_duration() {
    let (mut game, scout, _target, _) = smoke_command_fixture();
    game.state.players[0]
        .upgrades
        .insert(crate::game::upgrade::UpgradeKind::SmokePlus);
    let target = game
        .state
        .entities
        .get(scout)
        .map(|entity| (entity.pos_x, entity.pos_y))
        .expect("scout should exist");

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

    let cloud = game
        .state
        .smokes
        .iter()
        .next()
        .expect("upgraded smoke should spawn immediately at zero launch distance");
    assert_eq!(
        config::SMOKE_PLUS_CLOUD_RADIUS_TILES,
        config::SMOKE_CLOUD_RADIUS_TILES * 1.5,
        "Smoke Plus should increase smoke radius by 50%"
    );
    assert_eq!(cloud.radius_tiles, config::SMOKE_PLUS_CLOUD_RADIUS_TILES);
    assert_eq!(
        cloud.expires_in(game.state.tick),
        config::SMOKE_PLUS_CLOUD_DURATION_TICKS as u16
    );
    let snapshot = game.snapshot_full_for(1);
    assert_eq!(
        snapshot.smokes[0].radius_tiles,
        config::SMOKE_PLUS_CLOUD_RADIUS_TILES
    );
    assert_eq!(
        snapshot.smokes[0].expires_in,
        config::SMOKE_PLUS_CLOUD_DURATION_TICKS as u16
    );
}

#[test]
fn command_car_requires_tank_production_then_trains_at_factory() {
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
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }
    let city_centre_pos = game.state.map.tile_center(8, 12);
    let factory_pos = game.state.map.tile_center(12, 8);
    game.state
        .entities
        .spawn_building(
            1,
            EntityKind::CityCentre,
            city_centre_pos.0,
            city_centre_pos.1,
            true,
        )
        .expect("city centre should spawn");
    let factory = game
        .state
        .entities
        .spawn_building(1, EntityKind::Factory, factory_pos.0, factory_pos.1, true)
        .expect("factory should spawn");

    game.enqueue(
        1,
        Command::Train {
            building: factory,
            unit: EntityKind::CommandCar,
        },
    );
    game.tick();
    assert!(
        game.state
            .entities
            .get(factory)
            .expect("factory")
            .prod_queue()
            .is_empty(),
        "Command Cars should require Tank Production first"
    );

    game.state.players[0]
        .upgrades
        .insert(crate::game::upgrade::UpgradeKind::TankUnlock);
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
        game.state
            .entities
            .iter()
            .any(|e| e.owner == 1 && e.kind == EntityKind::CommandCar),
        "Vehicle Works should train Command Cars after Tank Production"
    );
}

#[test]
fn breakthrough_applies_owned_nonstacking_speed_status_and_cooldown() {
    let (mut game, _scout, _target, _) = smoke_command_fixture();
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }
    let car_pos = game.state.map.tile_center(8, 8);
    let nearby_pos = game.state.map.tile_center(10, 8);
    let far_pos = game.state.map.tile_center(20, 8);
    let command_car = game
        .state
        .entities
        .spawn_unit(1, EntityKind::CommandCar, car_pos.0, car_pos.1)
        .expect("command car should spawn");
    let nearby = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, nearby_pos.0, nearby_pos.1)
        .expect("nearby rifle should spawn");
    let nearby_command_car = game
        .state
        .entities
        .spawn_unit(1, EntityKind::CommandCar, nearby_pos.0 + 16.0, nearby_pos.1)
        .expect("nearby command car should spawn");
    let far = game
        .state
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

    let car = game.state.entities.get(command_car).expect("command car");
    assert!(car.ability_cooldown_ticks(ability::AbilityKind::Breakthrough) > 0);
    assert!(car.breakthrough_ticks() > 0);
    assert!(car.breakthrough_aura_ticks() > 0);
    assert!(
        game.state
            .entities
            .get(nearby)
            .expect("nearby unit")
            .breakthrough_ticks()
            > 0
    );
    let nearby_car = game
        .state
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
        game.state
            .entities
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
    assert_eq!(
        caster_view.breakthrough_aura_ticks,
        Some(car.breakthrough_aura_ticks()),
        "visible caster view should expose active aura duration for rendering"
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
    assert_eq!(
        nearby_car_view.breakthrough_aura_ticks, None,
        "buffed non-caster Command Cars should not project caster aura status"
    );

    let remaining = game
        .state
        .entities
        .get(nearby)
        .expect("nearby unit")
        .breakthrough_ticks();
    if let Some(e) = game.state.entities.get_mut(nearby) {
        e.start_breakthrough(1);
    }
    assert_eq!(
        game.state
            .entities
            .get(nearby)
            .expect("nearby unit")
            .breakthrough_ticks(),
        remaining,
        "shorter overlapping Breakthrough should not reduce an active buff"
    );
}

#[test]
fn breakthrough_uses_configured_plain_and_smoke_speed_multipliers() {
    let (mut game, _scout, _target, _) = smoke_command_fixture();
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }
    let plain_pos = game.state.map.tile_center(8, 5);
    let smoke_pos = game.state.map.tile_center(8, 10);
    let baseline_pos = game.state.map.tile_center(8, 15);
    let goal_plain = game.state.map.tile_center(20, 5);
    let goal_smoke = game.state.map.tile_center(20, 10);
    let goal_baseline = game.state.map.tile_center(20, 15);
    let plain = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, plain_pos.0, plain_pos.1)
        .expect("plain rifle should spawn");
    let smoked = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, smoke_pos.0, smoke_pos.1)
        .expect("smoked rifle should spawn");
    let baseline = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, baseline_pos.0, baseline_pos.1)
        .expect("baseline rifle should spawn");
    game.state.smokes.schedule(
        smoke_pos.0,
        smoke_pos.1,
        crate::config::SMOKE_CLOUD_RADIUS_TILES,
        crate::config::SMOKE_CLOUD_DURATION_TICKS,
        game.state.tick,
    );
    game.state.smokes.spawn_due(game.state.tick);
    for id in [plain, smoked] {
        if let Some(e) = game.state.entities.get_mut(id) {
            e.start_breakthrough(crate::config::BREAKTHROUGH_DURATION_TICKS);
        }
    }
    if let Some(e) = game.state.entities.get_mut(smoked) {
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
            units: vec![baseline],
            x: goal_baseline.0,
            y: goal_baseline.1,
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
    let plain_dx = game.state.entities.get(plain).expect("plain").pos_x - plain_pos.0;
    let smoke_dx = game.state.entities.get(smoked).expect("smoked").pos_x - smoke_pos.0;
    let baseline_dx = game.state.entities.get(baseline).expect("baseline").pos_x - baseline_pos.0;
    assert!(
        (plain_dx / baseline_dx - crate::config::BREAKTHROUGH_BASE_SPEED_MULTIPLIER).abs() < 0.001,
        "plain Breakthrough should use its configured multiplier ({plain_dx} / {baseline_dx})"
    );
    assert!(
        (smoke_dx / baseline_dx - crate::config::BREAKTHROUGH_SMOKE_SPEED_MULTIPLIER).abs() < 0.001,
        "smoke Breakthrough should use its configured multiplier ({smoke_dx} / {baseline_dx})"
    );
}

#[test]
fn command_car_passive_aura_uses_configured_speed_and_has_no_smoke_bonus() {
    let (mut game, _scout, _target, _) = smoke_command_fixture();
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    let car_pos = game.state.map.tile_center(8, 8);
    let near_pos = game.state.map.tile_center(10, 6);
    let smoky_near_pos = game.state.map.tile_center(10, 10);
    let far_pos = game.state.map.tile_center(20, 8);
    let goal_near = game.state.map.tile_center(30, 6);
    let goal_smoky_near = game.state.map.tile_center(30, 10);
    let goal_far = game.state.map.tile_center(40, 8);
    game.state
        .entities
        .spawn_unit(1, EntityKind::CommandCar, car_pos.0, car_pos.1)
        .expect("command car should spawn");
    let near = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, near_pos.0, near_pos.1)
        .expect("nearby rifle should spawn");
    let smoky_near = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, smoky_near_pos.0, smoky_near_pos.1)
        .expect("nearby smoky rifle should spawn");
    let far = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, far_pos.0, far_pos.1)
        .expect("far rifle should spawn");
    if let Some(entity) = game.state.entities.get_mut(smoky_near) {
        entity.mark_in_smoke_for_breakthrough(crate::config::BREAKTHROUGH_RECENT_SMOKE_TICKS);
    }

    for (unit, goal) in [
        (near, goal_near),
        (smoky_near, goal_smoky_near),
        (far, goal_far),
    ] {
        game.enqueue(
            1,
            Command::Move {
                units: vec![unit],
                x: goal.0,
                y: goal.1,
                queued: false,
            },
        );
    }
    for _ in 0..10 {
        game.tick();
    }

    let near_dx = game.state.entities.get(near).expect("nearby rifle").pos_x - near_pos.0;
    let smoky_near_dx = game
        .state
        .entities
        .get(smoky_near)
        .expect("nearby smoky rifle")
        .pos_x
        - smoky_near_pos.0;
    let far_dx = game.state.entities.get(far).expect("far rifle").pos_x - far_pos.0;
    assert!(
        near_dx > far_dx,
        "the nearby permanent aura should increase movement ({near_dx} <= {far_dx})"
    );
    assert!(
        (near_dx - smoky_near_dx).abs() < 0.001,
        "the passive aura must not gain Breakthrough's smoke speed bonus ({near_dx} != {smoky_near_dx})"
    );
    assert!(
        (near_dx / far_dx - crate::config::COMMAND_CAR_AURA_SPEED_MULTIPLIER).abs() < 0.001,
        "the permanent aura should use the configured multiplier"
    );
}
