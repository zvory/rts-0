use super::fixtures::*;
use super::*;

#[test]
fn scout_car_smoke_defaults_missing_legacy_charge_state_to_full() {
    let (mut game, scout, _target, _) = smoke_command_fixture();
    let target = game.state.map.tile_center(12, 8);
    let scout_entity = game
        .state
        .entities
        .get_mut(scout)
        .expect("scout should exist");
    scout_entity
        .ability_uses_remaining
        .remove(&ability::AbilityKind::Smoke);
    scout_entity
        .ability_charge_recharge_ticks
        .remove(&ability::AbilityKind::Smoke);
    assert_eq!(
        scout_entity.ability_uses_remaining(ability::AbilityKind::Smoke),
        Some(config::SCOUT_CAR_SMOKE_CHARGES),
        "older checkpoints without charge state should treat Smoke as fully charged"
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

    assert_eq!(
        game.state
            .entities
            .get(scout)
            .expect("scout should exist")
            .ability_uses_remaining(ability::AbilityKind::Smoke),
        Some(config::SCOUT_CAR_SMOKE_CHARGES - 1),
        "a legacy-state Scout Car should spend and begin recharging normally"
    );
}

#[test]
fn scout_car_smoke_recharges_two_spent_charges_sequentially() {
    let (mut game, scout, _target, _) = smoke_command_fixture();
    let target = game.state.map.tile_center(12, 8);
    game.state.players[0].set_resources(0, 0);

    for _ in 0..3 {
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
    }
    game.tick();

    let scout_entity = game.state.entities.get(scout).expect("scout should exist");
    assert_eq!(
        scout_entity.ability_uses_remaining(ability::AbilityKind::Smoke),
        Some(0),
        "three same-tick casts should spend only the two stored charges"
    );
    assert_eq!(
        scout_entity.ability_cooldown_ticks(ability::AbilityKind::Smoke),
        0,
        "Smoke should retain its existing cooldown"
    );
    let smoke_status = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|entity| entity.id == scout)
        .expect("owner snapshot should include the Scout Car")
        .abilities
        .into_iter()
        .find(|status| status.ability == ability::AbilityKind::Smoke.to_protocol_str())
        .expect("owner snapshot should include Smoke charge status");
    assert_eq!(
        (
            smoke_status.remaining_uses,
            smoke_status.charge_recharge_left
        ),
        (Some(0), Some(config::SCOUT_CAR_SMOKE_CHARGE_RECHARGE_TICKS)),
        "owner snapshot should drive the HUD recharge clock from the authoritative timer"
    );

    for _ in 0..config::SMOKE_LAUNCH_MAX_DELAY_TICKS {
        game.tick();
    }
    assert_eq!(game.state.smokes.iter().count(), 2);
    assert_eq!(game.state.players[0].steel, 0);
    assert_eq!(game.state.players[0].oil, 0);

    let elapsed_ticks = config::SMOKE_LAUNCH_MAX_DELAY_TICKS as u16;
    for _ in elapsed_ticks..config::SCOUT_CAR_SMOKE_CHARGE_RECHARGE_TICKS - 1 {
        game.tick();
    }
    assert_eq!(
        game.state
            .entities
            .get(scout)
            .expect("scout should exist")
            .ability_uses_remaining(ability::AbilityKind::Smoke),
        Some(0),
        "the first charge should not return before 15 seconds"
    );
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(scout)
            .expect("scout should exist")
            .ability_uses_remaining(ability::AbilityKind::Smoke),
        Some(1),
        "the first missing charge should return after 15 seconds"
    );

    for _ in 0..config::SCOUT_CAR_SMOKE_CHARGE_RECHARGE_TICKS - 1 {
        game.tick();
    }
    assert_eq!(
        game.state
            .entities
            .get(scout)
            .expect("scout should exist")
            .ability_uses_remaining(ability::AbilityKind::Smoke),
        Some(1),
        "the second charge should require another full 15-second interval"
    );
    game.tick();
    let scout_entity = game.state.entities.get(scout).expect("scout should exist");
    assert_eq!(
        scout_entity.ability_uses_remaining(ability::AbilityKind::Smoke),
        Some(config::SCOUT_CAR_SMOKE_CHARGES),
        "the second missing charge should restore the two-charge maximum"
    );
    assert_eq!(
        scout_entity.ability_cooldown_ticks(ability::AbilityKind::Smoke),
        0
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
    assert_eq!(
        game.state
            .entities
            .get(scout)
            .expect("scout should exist")
            .ability_uses_remaining(ability::AbilityKind::Smoke),
        Some(1),
        "recharged Smoke should support unlimited lifetime casts"
    );
}
