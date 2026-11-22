use super::fixtures::*;
use super::*;
use crate::game::entity::{Order, RallyIntent, RallyKind};
use crate::game::upgrade::UpgradeKind;

fn player(id: u32) -> PlayerInit {
    PlayerInit {
        id,
        team_id: id,
        faction_id: "cultivators".into(),
        name: format!("P{id}"),
        color: "#fff".into(),
        is_ai: false,
    }
}
fn setup() -> (Game, u32) {
    let mut game = empty_flat_game(&[player(1), player(2)]);
    let grounds = game
        .state
        .entities
        .spawn_building(1, EntityKind::ProvenGrounds, 624.0, 624.0, true)
        .unwrap();
    game.state.players[0].set_resources(0, 300);
    (game, grounds)
}
fn queue(game: &mut Game, owner: u32, building: u32, choice: UpgradeKind) {
    game.enqueue(
        owner,
        Command::Research {
            building,
            upgrade: choice,
        },
    );
}
fn send(
    game: &mut Game,
    building: u32,
    owner: u32,
    kind: EntityKind,
    dao: Option<UpgradeKind>,
) -> u32 {
    let unit = game
        .state
        .entities
        .spawn_unit(owner, kind, 560.0, 624.0)
        .unwrap();
    if let Some(dao) = dao {
        game.state
            .entities
            .get_mut(unit)
            .unwrap()
            .set_disciple_kind_for_test(EntityKind::evolved_disciple(dao).unwrap());
    }
    let b = game.state.entities.get(building).unwrap();
    game.enqueue(
        owner,
        Command::Move {
            units: vec![unit, unit, u32::MAX],
            x: b.pos_x(),
            y: b.pos_y(),
            queued: false,
        },
    );
    unit
}
fn plans(game: &Game, building: u32) -> &[UpgradeKind] {
    &game
        .state
        .entities
        .get(building)
        .unwrap()
        .production
        .as_ref()
        .unwrap()
        .dao_training_queue
}
fn restore(game: &Game) -> Game {
    Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap()
}

#[test]
fn paid_dao_training_requires_research_owned_base_disciple_and_oil() {
    let (mut game, grounds) = setup();
    queue(&mut game, 1, grounds, UpgradeKind::DaoOfMagic);
    game.tick();
    assert!(
        plans(&game, grounds).is_empty(),
        "unresearched training is rejected"
    );
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfMagic);
    queue(&mut game, 2, grounds, UpgradeKind::DaoOfMagic);
    queue(&mut game, 1, grounds, UpgradeKind::TankPlus);
    game.tick();
    assert!(plans(&game, grounds).is_empty());
    queue(&mut game, 1, grounds, UpgradeKind::DaoOfMagic);
    game.state.players[0].set_resources(0, 99);
    let base = send(&mut game, grounds, 1, EntityKind::Warrior, None);
    game.tick();
    assert!(game
        .state
        .entities
        .get(grounds)
        .unwrap()
        .prod_queue()
        .is_empty());
    assert!(game.state.entities.get(base).is_some());
    assert_eq!(game.state.players[0].oil, 99);
    game.state.players[0].set_resources(0, 100);
    game.tick();
    assert!(game.state.entities.get(base).is_none());
    let foreign = send(&mut game, grounds, 2, EntityKind::Warrior, None);
    let evolved = send(
        &mut game,
        grounds,
        1,
        EntityKind::Warrior,
        Some(UpgradeKind::DaoOfMagic),
    );
    let worker = send(&mut game, grounds, 1, EntityKind::Worker, None);
    game.tick();
    for id in [foreign, evolved, worker] {
        assert!(game.state.entities.get(id).is_some());
    }
    assert_eq!(game.state.players[0].oil, 0);
    assert_eq!(
        game.state.entities.get(grounds).unwrap().prod_queue().len(),
        1
    );
}

#[test]
fn paid_dao_training_mixed_queue_exact_duration_supply_rally_checkpoint_and_cancel() {
    let (mut game, grounds) = setup();
    let choices = [
        UpgradeKind::DaoOfLightness,
        UpgradeKind::DaoOfHeaviness,
        UpgradeKind::DaoOfMagic,
    ];
    for choice in choices {
        game.state.players[0].upgrades.insert(choice);
        queue(&mut game, 1, grounds, choice);
    }
    game.state
        .entities
        .get_mut(grounds)
        .unwrap()
        .set_rally_point(Some(RallyIntent::new(RallyKind::Move, 950.0, 624.0)));
    game.tick();
    for _ in 0..5 {
        game.tick();
    }
    assert!(
        game.state
            .entities
            .get(grounds)
            .unwrap()
            .prod_queue()
            .is_empty(),
        "plans alone never produce Disciples"
    );
    assert_eq!(game.state.players[0].oil, 300);
    let input = send(&mut game, grounds, 1, EntityKind::Warrior, None);
    game.tick();
    assert!(game.state.entities.get(input).is_none());
    assert_eq!(game.state.players[0].supply_used, 2);
    assert_eq!(game.state.players[0].oil, 200);
    // Tail cancellation is free and cannot cancel the active trainee.
    game.enqueue(
        1,
        Command::Cancel {
            building: grounds,
            construction: false,
        },
    );
    game.tick();
    assert_eq!(plans(&game, grounds), &choices[..2]);
    let mut restored = restore(&game);
    let progress = game.state.entities.get(grounds).unwrap().prod_queue()[0].progress;
    for _ in progress..299 {
        game.tick();
        restored.tick();
    }
    assert!(!game.state.entities.iter().any(|e| e.kind.is_disciple()));
    game.tick();
    restored.tick();
    let output = game
        .state
        .entities
        .iter()
        .find(|e| e.kind.is_disciple())
        .unwrap();
    assert_eq!(output.dao(), Some(UpgradeKind::DaoOfLightness));
    assert_eq!(output.insight, 0);
    assert_eq!(output.insight_cap(), 120);
    assert_eq!(output.hp, output.max_hp);
    assert!(matches!(output.order(), Order::AttackMove(_)));
    assert_eq!(game.state.players[0].supply_used, 2);
    assert_eq!(
        game.replay_end_state_hash().unwrap(),
        restored.replay_end_state_hash().unwrap()
    );
    let second = send(&mut game, grounds, 1, EntityKind::Warrior, None);
    game.tick();
    for _ in 0..120 {
        if game.state.entities.get(second).is_none() {
            break;
        }
        game.tick();
    }
    assert!(game.state.entities.get(second).is_none());
    game.enqueue(
        1,
        Command::Cancel {
            building: grounds,
            construction: false,
        },
    );
    game.tick();
    assert_eq!(plans(&game, grounds), &[UpgradeKind::DaoOfHeaviness]);
    for _ in 0..300 {
        game.tick();
    }
    let war_hp = config::unit_stats(EntityKind::Warrior).unwrap().hp + config::DAO_WAR_HP_BONUS;
    assert!(
        game.state
            .entities
            .iter()
            .any(|e| e.dao() == Some(UpgradeKind::DaoOfHeaviness) && e.max_hp == war_hp),
        "expected War output; production={:?}; units={:?}",
        game.state.entities.get(grounds).unwrap().prod_queue(),
        game.state
            .entities
            .iter()
            .filter(|e| e.kind.is_disciple())
            .map(|e| (e.id, e.dao(), e.hp, e.max_hp, e.pos_x(), e.pos_y()))
            .collect::<Vec<_>>()
    );
    assert!(plans(&game, grounds).is_empty());
    assert_eq!(game.state.players[0].oil, 100);
}

#[test]
fn paid_dao_training_destroyed_building_loses_trainee_without_refund_and_hides_plans() {
    let (mut game, grounds) = setup();
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfMagic);
    queue(&mut game, 1, grounds, UpgradeKind::DaoOfMagic);
    send(&mut game, grounds, 1, EntityKind::Warrior, None);
    game.state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 800.0, 624.0)
        .unwrap();
    for _ in 0..12 {
        game.tick();
    }
    let own = game.snapshot_for(1);
    assert_eq!(
        own.entities
            .iter()
            .find(|e| e.id == grounds)
            .unwrap()
            .dao_training_queue
            .as_ref()
            .unwrap(),
        &vec!["dao_of_magic".to_string()]
    );
    let enemy = game.snapshot_for(2);
    let visible = enemy.entities.iter().find(|e| e.id == grounds).unwrap();
    assert!(visible.dao_training_queue.is_none());
    assert!(visible.prod_progress.is_some());
    game.state
        .entities
        .get_mut(grounds)
        .unwrap()
        .apply_damage(u32::MAX, None);
    game.tick();
    assert!(game.state.entities.get(grounds).is_none());
    assert_eq!(game.state.players[0].oil, 200);
    assert_eq!(game.state.players[0].supply_used, 0);
    assert!(!game
        .state
        .entities
        .iter()
        .any(|e| e.owner == 1 && e.kind.is_disciple()));
}

#[test]
fn paid_dao_training_blocked_exit_holds_completed_trainee_and_reservation() {
    let (mut game, grounds) = setup();
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfMagic);
    queue(&mut game, 1, grounds, UpgradeKind::DaoOfMagic);
    send(&mut game, grounds, 1, EntityKind::Warrior, None);
    game.tick();
    game.state.map.terrain.fill(crate::protocol::terrain::WATER);
    for _ in 0..310 {
        game.tick();
    }
    assert_eq!(
        game.state.entities.get(grounds).unwrap().prod_queue()[0].progress,
        300
    );
    assert_eq!(game.state.players[0].supply_used, 2);
    assert_eq!(game.state.players[0].oil, 200);
    let mut game = restore(&game);
    game.state.map.terrain.fill(crate::protocol::terrain::GRASS);
    game.tick();
    assert!(game
        .state
        .entities
        .get(grounds)
        .unwrap()
        .prod_queue()
        .is_empty());
    assert!(plans(&game, grounds).is_empty());
    assert_eq!(
        game.state
            .entities
            .iter()
            .filter(|e| e.dao() == Some(UpgradeKind::DaoOfMagic))
            .count(),
        1
    );
}

#[test]
fn paid_dao_training_queue_is_bounded_and_portal_rally_supplies_the_input() {
    let (mut game, grounds) = setup();
    game.state.players[0]
        .upgrades
        .insert(UpgradeKind::DaoOfMagic);
    for _ in 0..100 {
        queue(&mut game, 1, grounds, UpgradeKind::DaoOfMagic);
    }
    game.tick();
    assert_eq!(plans(&game, grounds).len(), config::MAX_DAO_TRAINING_QUEUE);
    let portal = game
        .state
        .entities
        .spawn_building(1, EntityKind::Portal, 368.0, 624.0, true)
        .unwrap();
    game.state
        .entities
        .get_mut(portal)
        .unwrap()
        .set_rally_point(Some(RallyIntent::new(RallyKind::Move, 624.0, 624.0)));
    game.state.players[0].set_resources(200, 300);
    game.enqueue(
        1,
        Command::Train {
            building: portal,
            unit: EntityKind::Warrior,
        },
    );
    for _ in 0..800 {
        game.tick();
    }
    let output = game
        .state
        .entities
        .iter()
        .find(|e| e.dao().is_some())
        .unwrap();
    assert_eq!(output.dao(), Some(UpgradeKind::DaoOfMagic));
    assert_eq!(game.state.players[0].oil, 200);
    assert_eq!(
        plans(&game, grounds).len(),
        config::MAX_DAO_TRAINING_QUEUE - 1
    );
    assert_eq!(game.state.players[0].supply_used, 2);
    assert!(!game
        .state
        .entities
        .iter()
        .any(|e| e.kind.is_disciple() && e.dao().is_none()));
}
