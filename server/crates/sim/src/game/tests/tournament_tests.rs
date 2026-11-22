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

fn arena(game: &mut Game) -> u32 {
    game.state
        .entities
        .spawn_building(1, EntityKind::TournamentGrounds, 640.0, 640.0, true)
        .unwrap()
}

fn send(game: &mut Game, grounds: u32, count: usize) -> Vec<u32> {
    let units: Vec<_> = (0..count)
        .map(|i| {
            game.state
                .entities
                .spawn_unit(1, EntityKind::Warrior, 560.0, 600.0 + i as f32 * 28.0)
                .unwrap()
        })
        .collect();
    let b = game.state.entities.get(grounds).unwrap();
    game.enqueue(
        1,
        Command::Move {
            units: units.clone(),
            x: b.pos_x(),
            y: b.pos_y(),
            queued: false,
        },
    );
    units
}

fn entrants(game: &Game, grounds: u32) -> u8 {
    game.state
        .entities
        .get(grounds)
        .unwrap()
        .production
        .as_ref()
        .unwrap()
        .tournament_entrants
}

fn wait_running(game: &mut Game, grounds: u32) {
    for _ in 0..600 {
        game.tick();
        if !game
            .state
            .entities
            .get(grounds)
            .unwrap()
            .prod_queue()
            .is_empty()
        {
            return;
        }
    }
    panic!(
        "Disciples failed to enter arena: {}",
        entrants(game, grounds)
    );
}

#[test]
fn tournament_admits_at_each_side_and_corner_of_the_four_tile_footprint() {
    // The arena spans [576, 704] on each axis. Tile centers just outside
    // its sides and corners are legal approach points within admission range.
    for (x, y) in [
        (560.0, 640.0),
        (720.0, 640.0),
        (640.0, 560.0),
        (640.0, 720.0),
        (560.0, 560.0),
        (560.0, 720.0),
        (720.0, 560.0),
        (720.0, 720.0),
    ] {
        let mut game = empty_flat_game(&[player(1)]);
        let grounds = arena(&mut game);
        let unit = game
            .state
            .entities
            .spawn_unit(1, EntityKind::Warrior, x, y)
            .unwrap();
        game.enqueue(
            1,
            Command::Move {
                units: vec![unit],
                x: 640.0,
                y: 640.0,
                queued: false,
            },
        );
        game.tick();
        assert!(
            game.state.entities.get(unit).is_none(),
            "entrant at ({x}, {y}) must be admitted at the perimeter"
        );
        assert_eq!(entrants(&game, grounds), 1);
    }
}

#[test]
fn tournament_four_enter_five_seconds_one_random_upgrade_rally_and_checkpoint() {
    let mut game = empty_flat_game(&[player(1)]);
    let grounds = arena(&mut game);
    game.state.players[0].set_resources(0, 0);
    game.state
        .entities
        .get_mut(grounds)
        .unwrap()
        .set_rally_point(Some(RallyIntent::new(RallyKind::Move, 950.0, 640.0)));
    let sent = send(&mut game, grounds, 4);
    wait_running(&mut game, grounds);
    assert!(sent.iter().all(|id| game.state.entities.get(*id).is_none()));
    assert_eq!(
        game.state.players[0].supply_used, 2,
        "the winner reserves supply"
    );
    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    let progress = game.state.entities.get(grounds).unwrap().prod_queue()[0].progress;
    for _ in progress..149 {
        game.tick();
        restored.tick();
    }
    assert!(!game
        .state
        .entities
        .iter()
        .any(|e| e.kind == EntityKind::Warrior));
    game.tick();
    restored.tick();
    let winners: Vec<_> = game
        .state
        .entities
        .iter()
        .filter(|e| e.kind == EntityKind::Warrior)
        .collect();
    assert_eq!(winners.len(), 1, "one winner emerges at five seconds");
    for e in winners {
        assert!(matches!(
            e.dao,
            Some(
                UpgradeKind::DaoOfLightness | UpgradeKind::DaoOfHeaviness | UpgradeKind::DaoOfMagic
            )
        ));
        assert_eq!(e.hp, e.max_hp);
        assert_eq!(e.insight, 0);
        assert_eq!(e.insight_cap(), 120);
        assert!(
            matches!(e.order(), Order::AttackMove(_)),
            "combat winners inherit rally orders"
        );
    }
    let completed_payload = game.checkpoint_payload_text_for_test().unwrap();
    Game::restore_checkpoint_payload_text_for_test(
        &completed_payload,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    assert_eq!(entrants(&game, grounds), 0);
    assert_eq!(
        (game.state.players[0].steel, game.state.players[0].oil),
        (0, 0)
    );
    assert_eq!(
        game.replay_end_state_hash().unwrap(),
        restored.replay_end_state_hash().unwrap()
    );
}

#[test]
fn tournament_partial_capacity_rejects_specialists_enemies_and_cancels_entry() {
    let mut game = empty_flat_game(&[player(1), player(2)]);
    game.state
        .entities
        .spawn_building(2, EntityKind::Portal, 1808.0, 1808.0, true)
        .unwrap();
    let grounds = arena(&mut game);
    send(&mut game, grounds, 3);
    for _ in 0..400 {
        game.tick();
    }
    assert_eq!(entrants(&game, grounds), 3);
    assert!(game
        .state
        .entities
        .get(grounds)
        .unwrap()
        .prod_queue()
        .is_empty());
    let specialist = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 560.0, 650.0)
        .unwrap();
    game.state.entities.get_mut(specialist).unwrap().dao = Some(UpgradeKind::DaoOfMagic);
    let enemy = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 560.0, 690.0)
        .unwrap();
    for (owner, id) in [(1, specialist), (2, enemy)] {
        game.enqueue(
            owner,
            Command::Move {
                units: vec![id],
                x: 640.0,
                y: 640.0,
                queued: false,
            },
        );
    }
    let cancelled = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 400.0)
        .unwrap();
    game.enqueue(
        1,
        Command::Move {
            units: vec![cancelled],
            x: 640.0,
            y: 640.0,
            queued: false,
        },
    );
    game.enqueue(
        1,
        Command::Stop {
            units: vec![cancelled],
        },
    );
    game.tick();
    assert_eq!(entrants(&game, grounds), 3);
    assert!(game.state.entities.get(specialist).is_some());
    assert!(game.state.entities.get(enemy).is_some());
    assert_eq!(
        game.state
            .entities
            .get(cancelled)
            .unwrap()
            .movement
            .as_ref()
            .unwrap()
            .tournament_target,
        None
    );
}

#[test]
fn tournament_destroyed_batch_never_mints_refunds_and_cannot_be_cancelled() {
    let mut game = empty_flat_game(&[player(1)]);
    let grounds = arena(&mut game);
    send(&mut game, grounds, 4);
    wait_running(&mut game, grounds);
    game.state.players[0].set_resources(0, 0);
    game.enqueue(
        1,
        Command::Cancel {
            building: grounds,
            construction: false,
        },
    );
    game.tick();
    assert_eq!(
        game.state.entities.get(grounds).unwrap().prod_queue().len(),
        1
    );
    game.state
        .entities
        .get_mut(grounds)
        .unwrap()
        .apply_damage(u32::MAX, None);
    game.tick();
    assert!(game.state.entities.get(grounds).is_none());
    assert_eq!(game.state.players[0].supply_used, 0);
    assert_eq!(game.state.players[0].steel, 0);
}

#[test]
fn tournament_counts_are_team_private_and_hidden_buildings_stay_hidden() {
    let mut game = empty_flat_game(&[player(1), player(2)]);
    game.state
        .entities
        .spawn_building(2, EntityKind::Portal, 1808.0, 1808.0, true)
        .unwrap();
    let grounds = arena(&mut game);
    send(&mut game, grounds, 1);
    for _ in 0..120 {
        game.tick();
    }
    let owner = game.snapshot_for(1);
    assert_eq!(
        owner
            .entities
            .iter()
            .find(|e| e.id == grounds)
            .unwrap()
            .tournament_entrants,
        Some(1)
    );
    assert!(!game
        .snapshot_for(2)
        .entities
        .iter()
        .any(|e| e.id == grounds));
    game.state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 760.0, 640.0)
        .unwrap();
    for _ in 0..12 {
        game.tick();
    }
    assert_eq!(
        game.snapshot_for(2)
            .entities
            .iter()
            .find(|e| e.id == grounds)
            .unwrap()
            .tournament_entrants,
        None
    );
}

#[test]
fn tournament_blocked_exits_keep_winners_until_legal_spawn_is_available() {
    let mut game = empty_flat_game(&[player(1)]);
    let grounds = arena(&mut game);
    send(&mut game, grounds, 4);
    wait_running(&mut game, grounds);
    game.state.map.terrain.fill(crate::protocol::terrain::WATER);
    for _ in 0..310 {
        game.tick();
    }
    assert_eq!(
        game.state.entities.get(grounds).unwrap().prod_queue().len(),
        1
    );
    assert_eq!(game.state.players[0].supply_used, 2);
    game.state.map.terrain.fill(crate::protocol::terrain::GRASS);
    game.tick();
    assert_eq!(
        game.state
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Warrior)
            .count(),
        1
    );
    assert_eq!(entrants(&game, grounds), 0);
}

#[test]
fn tournament_rules_require_portal_and_only_cultivator_engineers_can_build() {
    use crate::rules::{economy, faction};
    let kind = EntityKind::TournamentGrounds;
    assert!(economy::can_build_for_faction(
        "cultivators",
        EntityKind::Worker,
        kind
    ));
    assert!(!economy::can_build_for_faction(
        faction::DEFAULT_FACTION_ID,
        EntityKind::Worker,
        kind
    ));
    assert!(!economy::build_requirement_met(kind, &[]));
    assert!(!economy::build_requirement_met(
        kind,
        &[EntityKind::ResourceDepot]
    ));
    assert!(economy::build_requirement_met(kind, &[EntityKind::Portal]));
    let stats = config::building_stats(kind).unwrap();
    assert_eq!(
        (
            stats.cost_steel,
            stats.cost_oil,
            stats.build_ticks,
            stats.foot_w,
            stats.foot_h
        ),
        (100, 0, 200, 4, 4)
    );
}

#[test]
fn tournament_portal_rally_trains_and_feeds_four_disciples_automatically() {
    let mut game = empty_flat_game(&[player(1)]);
    let grounds = arena(&mut game);
    let portal = game
        .state
        .entities
        .spawn_building(1, EntityKind::Portal, 400.0, 656.0, true)
        .unwrap();
    game.state.players[0].set_resources(1000, 0);
    game.enqueue(
        1,
        Command::SetRally {
            building: portal,
            x: 640.0,
            y: 640.0,
            kind: RallyKind::Move,
            queued: false,
        },
    );
    for _ in 0..4 {
        game.enqueue(
            1,
            Command::Train {
                building: portal,
                unit: EntityKind::Warrior,
            },
        );
    }
    for _ in 0..1500 {
        game.tick();
        if !game
            .state
            .entities
            .get(grounds)
            .unwrap()
            .prod_queue()
            .is_empty()
        {
            break;
        }
    }
    assert_eq!(
        entrants(&game, grounds),
        4,
        "Portal graduates enter automatically without individual commands"
    );
    assert!(!game
        .state
        .entities
        .iter()
        .any(|e| e.kind == EntityKind::Warrior));
    assert_eq!(
        game.state.players[0].steel, 600,
        "only the four Portal training costs are paid"
    );
    for _ in 0..300 {
        game.tick();
    }
    let winners: Vec<_> = game
        .state
        .entities
        .iter()
        .filter(|e| e.kind == EntityKind::Warrior)
        .collect();
    assert_eq!(winners.len(), 1);
    assert!(winners[0].dao.is_some());
}

#[test]
fn tournament_busy_arrivals_wait_and_enter_the_next_intake() {
    let mut game = empty_flat_game(&[player(1)]);
    let grounds = arena(&mut game);
    send(&mut game, grounds, 4);
    wait_running(&mut game, grounds);
    let waiting = send(&mut game, grounds, 1)[0];
    for _ in 0..100 {
        game.tick();
    }
    assert!(game.state.entities.get(waiting).is_some());
    assert_eq!(
        game.state
            .entities
            .get(waiting)
            .unwrap()
            .movement
            .as_ref()
            .unwrap()
            .tournament_target,
        Some(grounds)
    );
    for _ in 0..300 {
        game.tick();
    }
    assert!(game.state.entities.get(waiting).is_none());
    assert_eq!(entrants(&game, grounds), 1);
    assert_eq!(
        game.state
            .entities
            .iter()
            .filter(|e| e.kind == EntityKind::Warrior && e.dao.is_some())
            .count(),
        1
    );
}

#[test]
fn tournament_staged_disciple_retries_when_the_perimeter_clears() {
    let mut game = empty_flat_game(&[player(1)]);
    let grounds = arena(&mut game);
    let mut blockers = Vec::new();
    for ty in 0..6 {
        for tx in 0..6 {
            if tx != 0 && tx != 5 && ty != 0 && ty != 5 {
                continue;
            }
            blockers.push(
                game.state
                    .entities
                    .spawn_unit(
                        1,
                        EntityKind::Worker,
                        560.0 + tx as f32 * 32.0,
                        560.0 + ty as f32 * 32.0,
                    )
                    .unwrap(),
            );
        }
    }
    let disciple = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 400.0, 640.0)
        .unwrap();
    game.enqueue(
        1,
        Command::Move {
            units: vec![disciple],
            x: 640.0,
            y: 640.0,
            queued: false,
        },
    );
    for _ in 0..180 {
        game.tick();
    }
    assert!(game.state.entities.get(disciple).is_some());
    assert_eq!(entrants(&game, grounds), 0);
    for id in blockers {
        game.state.entities.remove(id);
    }
    for _ in 0..300 {
        game.tick();
    }
    assert!(
        game.state.entities.get(disciple).is_none(),
        "staged entrants retry after temporary crowding clears"
    );
    assert_eq!(entrants(&game, grounds), 1);
}

#[test]
fn tournament_entry_intent_survives_a_fully_blocked_approach() {
    let mut game = empty_flat_game(&[player(1)]);
    let grounds = arena(&mut game);
    game.state.map.terrain.fill(crate::protocol::terrain::WATER);
    // Leave the entrant on legal ground, beyond the arena's six-ring spawn search.
    game.state.map.terrain[(20 * game.state.map.width + 9) as usize] =
        crate::protocol::terrain::GRASS;
    let disciple = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 304.0, 656.0)
        .unwrap();
    game.enqueue(
        1,
        Command::Move {
            units: vec![disciple],
            x: 640.0,
            y: 640.0,
            queued: false,
        },
    );
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(disciple)
            .unwrap()
            .movement
            .as_ref()
            .unwrap()
            .tournament_target,
        Some(grounds)
    );
    let payload = game.checkpoint_payload_text_for_test().unwrap();
    Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    game.state.map.terrain.fill(crate::protocol::terrain::GRASS);
    for _ in 0..300 {
        game.tick();
    }
    assert!(game.state.entities.get(disciple).is_none());
    assert_eq!(entrants(&game, grounds), 1);
}

#[test]
fn tournament_evolution_during_approach_clears_entry_without_stopping_movement() {
    use crate::game::ability::AbilityKind;

    let mut game = empty_flat_game(&[player(1)]);
    let grounds = arena(&mut game);
    let disciple = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Warrior, 304.0, 640.0)
        .unwrap();
    let unit = game.state.entities.get_mut(disciple).unwrap();
    unit.insight = config::DISCIPLE_INSIGHT_REQUIRED - 1;
    unit.choose_dao(AbilityKind::EvolveLightness);
    game.enqueue(
        1,
        Command::Move {
            units: vec![disciple],
            x: 640.0,
            y: 640.0,
            queued: false,
        },
    );
    game.tick();
    assert_eq!(
        game.state
            .entities
            .get(disciple)
            .unwrap()
            .movement
            .as_ref()
            .unwrap()
            .tournament_target,
        Some(grounds)
    );

    // Combat insight can evolve a unit still on its approach after the admission pass.
    game.state.entities.get_mut(disciple).unwrap().insight = config::DISCIPLE_INSIGHT_REQUIRED;
    game.tick();
    let payload = game.checkpoint_payload_text_for_test().unwrap();
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &payload,
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .expect("an evolved entrant must remain checkpointable at the end of the tick");
    let unit = game.state.entities.get(disciple).unwrap();
    assert_eq!(unit.dao, Some(UpgradeKind::DaoOfLightness));
    assert_eq!(unit.movement.as_ref().unwrap().tournament_target, None);
    assert!(matches!(unit.order(), Order::Move(_)));
    game.tick();
    restored.tick();
    assert!(matches!(
        game.state.entities.get(disciple).unwrap().order(),
        Order::Move(_)
    ));
    assert_eq!(entrants(&game, grounds), 0);
    assert_eq!(
        game.replay_end_state_hash().unwrap(),
        restored.replay_end_state_hash().unwrap()
    );
}
