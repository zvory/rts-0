use std::collections::HashMap;

use super::scoring::entity_score_value;
use super::*;
use crate::game::ai_core::profiles::{RIFLE_FLOOD_FAST_ID, RIFLE_FLOOD_FULL_SATURATION_ID};
use crate::game::command::SimCommand as Command;
use crate::game::entity::{Entity, EntityKind, GatherPhase, Order};
use crate::protocol::{kinds, EntityView};

fn human_vs_ai_players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            name: "Human".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            name: "Computer".into(),
            color: "#000".into(),
            is_ai: true,
        },
    ]
}

fn count_ai_pending_depot_builders(game: &Game, player_id: u32) -> usize {
    game.entities
        .iter()
        .filter(|e| e.owner == player_id && e.kind == EntityKind::Worker)
        .filter(|e| {
            matches!(
                e.order().build_intent_tile(),
                Some((EntityKind::Depot, _, _))
            )
        })
        .count()
}

fn count_ai_gathering_workers(game: &Game, player_id: u32) -> usize {
    game.entities
        .iter()
        .filter(|e| e.owner == player_id && e.kind == EntityKind::Worker)
        .filter(|e| matches!(e.order(), Order::Gather(_)))
        .count()
}

#[test]
fn live_ai_profiles_are_selected_from_requested_pool_at_match_start() {
    let players = human_vs_ai_players();

    for seed in 0..64 {
        let game = Game::new_with_random_ai_profiles(&players, seed);
        let profiles = game.ai_profile_ids();

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0], RIFLE_FLOOD_FULL_SATURATION_ID);
    }
}

#[test]
fn ordinary_game_new_uses_deterministic_ai_profile_for_tests() {
    let players = human_vs_ai_players();

    for seed in 0..16 {
        let game = Game::new(&players, seed);
        assert_eq!(game.ai_profile_ids(), vec![RIFLE_FLOOD_FULL_SATURATION_ID]);
    }
}

fn legacy_snapshot_entities(game: &Game, player: u32, fogged: bool) -> Vec<EntityView> {
    let mut entities = Vec::new();
    for id in game.spatial.all_ids() {
        let Some(e) = game.entities.get(id) else {
            continue;
        };
        let own = e.owner == player;
        if fogged
            && !own
            && !e.kind.is_node()
            && !game.fog.is_visible_world(player, e.pos_x, e.pos_y)
        {
            continue;
        }
        entities.push(legacy_view_of(game, e, player, fogged));
    }
    entities.sort_by_key(|v| v.id);
    entities
}

fn legacy_view_of(game: &Game, e: &Entity, viewer: u32, fogged: bool) -> EntityView {
    let mut v = EntityView::new(
        e.id,
        e.owner,
        e.kind.to_protocol_str(),
        e.pos_x,
        e.pos_y,
        e.hp,
        e.max_hp,
        e.state_str(),
    );

    if e.is_unit() {
        v.facing = Some(e.facing());
    }
    let active_combat_target = matches!(e.order(), Order::Attack(_) | Order::AttackMove(_))
        || (e.is_building() && e.can_attack());
    let target_visible = if let Some(t) = e.target_id() {
        game.entities
            .get(t)
            .map(|target| {
                e.owner == viewer
                    || !fogged
                    || game
                        .fog
                        .is_visible_world(viewer, target.pos_x, target.pos_y)
            })
            .unwrap_or(false)
    } else {
        false
    };
    let weapon_facing_useful = e.kind == EntityKind::Tank || active_combat_target;
    if weapon_facing_useful {
        if let Some(weapon_facing) = e.weapon_facing() {
            let weapon_facing_is_safe = e.owner == viewer
                || !fogged
                || e.target_id().is_none()
                || !active_combat_target
                || target_visible;
            if weapon_facing_is_safe {
                v.weapon_facing = Some(weapon_facing);
            }
        }
    }
    if e.kind == EntityKind::MachineGunner {
        v.setup_state = Some(e.weapon_setup().to_protocol_str().to_string());
    }
    if e.kind == EntityKind::AtTeam {
        v.setup_state = Some(e.weapon_setup().to_protocol_str().to_string());
        if e.owner == viewer {
            v.setup_facing = e.emplacement_facing();
        }
    }
    if e.is_building() && !e.prod_queue().is_empty() {
        if let Some(front) = e.prod_queue().first() {
            v.prod_kind = Some(front.unit.to_protocol_str().to_string());
            v.prod_progress = Some(if front.total == 0 {
                0.0
            } else {
                front.progress as f32 / front.total as f32
            });
        }
        if e.owner == viewer {
            v.prod_queue = Some(e.prod_queue().len() as u32);
        }
    }
    if let Some(progress) = e.build_progress_fraction() {
        v.build_progress = Some(progress);
    }
    if e.is_node() {
        v.remaining = e.remaining();
    }
    if e.kind == EntityKind::Worker && e.gather_phase() == Some(GatherPhase::Harvesting) {
        if let Some(node) = e.order().gather_node() {
            v.latched_node = Some(node);
        }
    }
    if let Some(t) = e.target_id() {
        if active_combat_target {
            if game.entities.get(t).is_some() {
                if target_visible {
                    v.target_id = Some(t);
                }
            }
        }
    }
    v
}

fn flat_tank_move_fixture() -> (Game, u32, (f32, f32)) {
    let players = [PlayerInit {
        id: 1,
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    for tile in &mut game.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }

    let start = game.map.tile_center(4, 4);
    let goal = game.map.tile_center(28, 17);
    let tank = game
        .entities
        .spawn_unit(1, EntityKind::Tank, start.0, start.1)
        .expect("tank should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    game.assert_invariants();

    (game, tank, goal)
}

fn queued_move_fixture() -> (Game, u32, (f32, f32), (f32, f32), (f32, f32)) {
    let players = [PlayerInit {
        id: 1,
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x5150_0001);
    for tile in &mut game.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }

    let start = game.map.tile_center(8, 8);
    let first = game.map.tile_center(10, 8);
    let second = game.map.tile_center(12, 8);
    let replacement = game.map.tile_center(8, 10);
    let unit = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, start.0, start.1)
        .expect("rifleman should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    game.assert_invariants();

    (game, unit, first, second, replacement)
}

fn entity_distance_to(game: &Game, id: u32, point: (f32, f32)) -> f32 {
    let entity = game.entities.get(id).expect("entity should exist");
    let dx = entity.pos_x - point.0;
    let dy = entity.pos_y - point.1;
    (dx * dx + dy * dy).sqrt()
}

#[test]
fn tank_move_command_preserves_exact_goal_and_repeats_deterministically() {
    let (mut live, tank, goal) = flat_tank_move_fixture();

    live.enqueue(
        1,
        Command::Move {
            units: vec![tank],
            x: goal.0,
            y: goal.1,
            queued: false,
        },
    );
    live.tick();

    assert_eq!(
        live.command_log(),
        &[super::replay::CommandLogEntry {
            tick: 1,
            player_id: 1,
            command: crate::protocol::Command::Move {
                units: vec![tank],
                x: goal.0,
                y: goal.1,
                queued: false,
            },
        }]
    );
    let moved_tank = live.entities.get(tank).expect("tank should exist");
    assert_eq!(moved_tank.path_goal(), Some(goal));
    assert_eq!(
        moved_tank
            .movement
            .as_ref()
            .map(|movement| movement.path.as_slice()),
        Some(&[goal][..]),
        "flat tank move should smooth to the exact command goal only"
    );

    let (mut repeat_a, tank_a, goal_a) = flat_tank_move_fixture();
    let (mut repeat_b, tank_b, goal_b) = flat_tank_move_fixture();
    assert_eq!(tank_a, tank_b, "fixture entity ids should be reproducible");
    assert_eq!(goal_a, goal_b, "fixture goals should be reproducible");
    for game in [&mut repeat_a, &mut repeat_b] {
        game.enqueue(
            1,
            Command::Move {
                units: vec![tank_a],
                x: goal_a.0,
                y: goal_a.1,
                queued: false,
            },
        );
    }

    for _ in 0..120 {
        repeat_a.tick();
        repeat_b.tick();
    }

    let a = repeat_a.entities.get(tank_a).expect("tank A should exist");
    let b = repeat_b.entities.get(tank_b).expect("tank B should exist");
    assert_eq!(
        (a.pos_x, a.pos_y, a.facing()),
        (b.pos_x, b.pos_y, b.facing())
    );
    assert_eq!(a.path_goal(), b.path_goal());
    assert_eq!(
        a.movement.as_ref().map(|movement| movement.path.clone()),
        b.movement.as_ref().map(|movement| movement.path.clone())
    );
    assert_eq!(repeat_a.command_log(), repeat_b.command_log());
}

#[test]
fn queued_move_commands_follow_waypoints_in_order() {
    let (mut game, unit, first, second, _) = queued_move_fixture();

    game.enqueue(
        1,
        Command::Move {
            units: vec![unit],
            x: first.0,
            y: first.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Move {
            units: vec![unit],
            x: second.0,
            y: second.1,
            queued: true,
        },
    );
    game.tick();

    let entity = game.entities.get(unit).expect("unit should exist");
    assert_eq!(
        entity.move_intent(),
        Some(first),
        "idle unit should immediately promote the first queued move"
    );
    assert_eq!(entity.queued_orders().len(), 1);

    for _ in 0..120 {
        game.tick();
    }

    let entity = game.entities.get(unit).expect("unit should exist");
    assert!(
        entity_distance_to(&game, unit, second) <= 3.0,
        "unit should end at the second queued waypoint"
    );
    assert!(entity.queued_orders().is_empty());
    assert!(matches!(entity.order(), Order::Idle));
    assert_eq!(game.command_log().len(), 2);
    assert!(game.command_log().iter().all(|entry| {
        matches!(
            &entry.command,
            crate::protocol::Command::Move { queued: true, .. }
        )
    }));
}

#[test]
fn normal_move_then_queued_move_snapshot_shows_active_and_future_waypoints() {
    let (mut game, unit, first, second, _) = queued_move_fixture();

    game.enqueue(
        1,
        Command::Move {
            units: vec![unit],
            x: first.0,
            y: first.1,
            queued: false,
        },
    );
    game.tick();
    game.enqueue(
        1,
        Command::Move {
            units: vec![unit],
            x: second.0,
            y: second.1,
            queued: true,
        },
    );
    game.tick();

    let view = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|entity| entity.id == unit)
        .expect("selected unit should be visible to owner");
    assert_eq!(
        view.active_marker,
        Some(crate::protocol::QueuedOrderMarker {
            x: first.0,
            y: first.1,
            attack_move: false,
        })
    );
    assert_eq!(
        view.queued_markers,
        vec![crate::protocol::QueuedOrderMarker {
            x: second.0,
            y: second.1,
            attack_move: false,
        }]
    );
}

#[test]
fn replacement_move_and_stop_clear_queued_movement() {
    let (mut game, unit, first, second, replacement) = queued_move_fixture();

    for goal in [first, second] {
        game.enqueue(
            1,
            Command::Move {
                units: vec![unit],
                x: goal.0,
                y: goal.1,
                queued: true,
            },
        );
    }
    game.tick();
    game.enqueue(
        1,
        Command::Move {
            units: vec![unit],
            x: replacement.0,
            y: replacement.1,
            queued: false,
        },
    );
    game.tick();

    let entity = game.entities.get(unit).expect("unit should exist");
    assert_eq!(entity.move_intent(), Some(replacement));
    assert!(entity.queued_orders().is_empty());

    game.enqueue(1, Command::Stop { units: vec![unit] });
    game.tick();

    let entity = game.entities.get(unit).expect("unit should exist");
    assert!(matches!(entity.order(), Order::Idle));
    assert!(entity.queued_orders().is_empty());
    assert!(entity.path_is_empty());
}

#[test]
fn scores_count_starting_entities() {
    let players = human_vs_ai_players();
    let game = Game::new(&players, 0x515C_0DE);
    let scores = game.scores();
    let human = scores
        .iter()
        .find(|score| score.id == 1)
        .expect("human score should exist");

    assert_eq!(
        human.unit_score,
        config::STARTING_WORKERS * entity_score_value(EntityKind::Worker)
    );
    assert_eq!(
        human.structure_score,
        entity_score_value(EntityKind::CityCentre)
    );
    assert_eq!(human.units_killed, 0);
    assert_eq!(human.units_lost, 0);
    assert_eq!(human.buildings_killed, 0);
    assert_eq!(human.buildings_lost, 0);
}

#[test]
fn scores_record_kills_and_losses_on_death() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0x515C_0DE);
    let victim_unit = game
        .entities
        .iter()
        .find(|e| e.owner == 2 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .expect("victim unit should exist");
    let victim_building = game
        .entities
        .iter()
        .find(|e| e.owner == 2 && e.kind == EntityKind::CityCentre)
        .map(|e| e.id)
        .expect("victim building should exist");
    for id in [victim_unit, victim_building] {
        let entity = game.entities.get_mut(id).expect("victim should exist");
        entity.hp = 0;
        entity.set_last_damage_owner(Some(1));
    }

    let mut events: HashMap<u32, Vec<Event>> =
        game.players.iter().map(|p| (p.id, Vec::new())).collect();
    let mut lingering_sight = Vec::new();
    let tick = game.tick_count();
    services::death::death_system(
        &mut game.entities,
        &game.fog,
        &mut game.players,
        &mut lingering_sight,
        &mut events,
        tick,
    );

    let scores = game.scores();
    let attacker = scores
        .iter()
        .find(|score| score.id == 1)
        .expect("attacker score should exist");
    let victim = scores
        .iter()
        .find(|score| score.id == 2)
        .expect("victim score should exist");

    assert_eq!(attacker.units_killed, 1);
    assert_eq!(attacker.buildings_killed, 1);
    assert_eq!(victim.units_lost, 1);
    assert_eq!(victim.buildings_lost, 1);
}

#[test]
fn phase4_projection_matches_legacy_snapshot_entities() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0xCAFE_BABE);
    let (sx, sy) = game
        .map
        .tile_center(game.players[0].start_tile.0, game.players[0].start_tile.1);
    let attacker = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, sx + 64.0, sy)
        .expect("attacker should spawn");
    let target = game
        .entities
        .spawn_unit(2, EntityKind::Rifleman, sx + 96.0, sy)
        .expect("target should spawn");
    if let Some(e) = game.entities.get_mut(attacker) {
        e.set_order(Order::attack(target));
        e.set_target_id(Some(target));
    }
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);

    assert_eq!(
        game.snapshot_for(1).entities,
        legacy_snapshot_entities(&game, 1, true)
    );
    assert_eq!(
        game.snapshot_full_for(1).entities,
        legacy_snapshot_entities(&game, 1, false)
    );
}

#[test]
fn spectator_snapshot_uses_union_fog_not_full_world() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0xCAFE_BABE);
    let active_players = [1, 2];
    game.fog
        .recompute(&active_players, &game.entities, &game.map);

    let hidden_pos = (0..game.map.size)
        .flat_map(|ty| (0..game.map.size).map(move |tx| (tx, ty)))
        .find_map(|(tx, ty)| {
            let (x, y) = game.map.tile_center(tx, ty);
            let hidden_from_all = active_players
                .iter()
                .all(|player| !game.fog.is_visible_world(*player, x, y));
            hidden_from_all.then_some((x, y))
        })
        .expect("map should contain a tile outside both players' opening fog");
    let hidden = game
        .entities
        .spawn_unit(99, EntityKind::Rifleman, hidden_pos.0, hidden_pos.1)
        .expect("hidden unit should spawn");
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    game.fog
        .recompute(&active_players, &game.entities, &game.map);

    let snapshot = game.snapshot_for_spectator(&active_players);

    assert!(snapshot.entities.iter().any(|e| e.owner == 1));
    assert!(snapshot.entities.iter().any(|e| e.owner == 2));
    assert!(!snapshot.entities.iter().any(|e| e.id == hidden));
    assert_eq!(snapshot.player_resources.len(), 2);
}

#[test]
fn death_vision_lingers_for_one_second_as_visual_only_intel() {
    let players = [
        PlayerInit {
            id: 1,
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = Game::new_for_replay(&players, 0xD3AD_5151);
    for tile in &mut game.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }

    let rifle_pos = game.map.tile_center(2, 2);
    let rifle = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, rifle_pos.0, rifle_pos.1)
        .expect("rifleman should spawn");
    let spotter_pos = game.map.tile_center(20, 20);
    let spotter = game
        .entities
        .spawn_unit(1, EntityKind::Worker, spotter_pos.0, spotter_pos.1)
        .expect("spotter should spawn");
    let enemy_pos = game.map.tile_center(22, 20);
    let enemy = game
        .entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    assert!(game.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1));

    game.entities
        .get_mut(spotter)
        .expect("spotter should exist")
        .hp = 0;
    game.tick();

    assert!(!game.entities.contains(spotter));
    assert!(
        !game.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1),
        "live fog should no longer see through the dead spotter"
    );
    let first_linger = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|e| e.id == enemy)
        .expect("enemy should remain visible through lingering death vision");
    assert!(first_linger.vision_only);

    let enemy_goal = game.map.tile_center(24, 20);
    game.enqueue(
        1,
        Command::Attack {
            units: vec![rifle],
            target: enemy,
            queued: false,
        },
    );
    game.enqueue(
        2,
        Command::Move {
            units: vec![enemy],
            x: enemy_goal.0,
            y: enemy_goal.1,
            queued: false,
        },
    );
    game.tick();

    let rifle_entity = game.entities.get(rifle).expect("rifle should remain alive");
    assert_eq!(
        rifle_entity.order().attack_target(),
        None,
        "vision-only enemies should not be accepted as direct attack targets"
    );
    let moved_enemy = game.entities.get(enemy).expect("enemy should remain alive");
    let moving_linger = game
        .snapshot_for(1)
        .entities
        .into_iter()
        .find(|e| e.id == enemy)
        .expect("moving enemy should still be visible during lingering death vision");
    assert!(moving_linger.vision_only);
    assert!((moving_linger.x - moved_enemy.pos_x).abs() < 0.001);
    assert!((moving_linger.y - moved_enemy.pos_y).abs() < 0.001);

    while game.tick_count() <= config::TICK_HZ {
        game.tick();
    }
    assert!(
        game.snapshot_for(1).entities.iter().all(|e| e.id != enemy),
        "lingering death vision should expire after one second"
    );
}

#[test]
fn live_ai_rifle_raid_attacks_visible_scout_car() {
    let players = human_vs_ai_players();
    let mut game = Game::new_for_replay(&players, 0xCAFE_BABE);
    for tile in &mut game.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }
    game.ai
        .push(ai::AiController::with_profile_id(2, RIFLE_FLOOD_FAST_ID));

    let ai_base = game.map.tile_center(42, 42);
    game.entities
        .spawn_building(2, EntityKind::CityCentre, ai_base.0, ai_base.1, true)
        .expect("AI city centre should spawn");
    let human_base = game.map.tile_center(8, 8);
    game.entities
        .spawn_building(1, EntityKind::CityCentre, human_base.0, human_base.1, true)
        .expect("human city centre should spawn");

    let raider_pos = game.map.tile_center(24, 24);
    let raider = game
        .entities
        .spawn_unit(2, EntityKind::Rifleman, raider_pos.0, raider_pos.1)
        .expect("AI rifleman should spawn");
    let scout_pos = game.map.tile_center(27, 24);
    let scout_car = game
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, scout_pos.0, scout_pos.1)
        .expect("human scout car should spawn");
    if let Some(e) = game.entities.get_mut(raider) {
        e.set_order(Order::move_to(human_base.0, human_base.1));
    }

    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);

    while (game.tick_count().wrapping_add(1).wrapping_add(2)) % 9 != 0 {
        game.tick();
    }
    game.tick();

    let raider = game
        .entities
        .get(raider)
        .expect("raider should remain alive");
    assert_eq!(
        raider.order().attack_target(),
        Some(scout_car),
        "visible scout car should interrupt the AI rifle raid move"
    );
}

/// Drive a passive human vs. one AI and confirm the deterministic default AI actually plays:
/// it grows its economy, expands supply, builds a barracks, produces riflemen, and marches
/// them into the human base to deal damage. This exercises the full command path the AI shares
/// with human clients.
#[test]
fn ai_builds_economy_and_attacks() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0x1234_5678);

    let mut max_workers = 0usize;
    let mut max_riflemen = 0usize;
    let mut ever_had_barracks = false;
    let mut ai_supply_cap = 0u32;
    let mut human_damaged = false;
    let mut max_pending_depot_builders = 0usize;
    let mut depot_completed_tick = None;
    let mut gathering_workers_after_depot = 0usize;
    let mut event_log = Vec::new();
    let target_workers = config::STEEL_PATCHES_PER_BASE as usize;

    // ~200s of simulation. The human issues no commands (passive target).
    for tick in 1..=6000 {
        for (player_id, events) in game.tick() {
            for event in events {
                if player_id == 2
                    && matches!(
                        event,
                        Event::Build { ref kind, .. } if kind == kinds::DEPOT
                    )
                {
                    depot_completed_tick.get_or_insert(tick);
                }
                event_log.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }

        max_pending_depot_builders =
            max_pending_depot_builders.max(count_ai_pending_depot_builders(&game, 2));
        if depot_completed_tick.is_some() {
            gathering_workers_after_depot =
                gathering_workers_after_depot.max(count_ai_gathering_workers(&game, 2));
        }

        let ai = game.snapshot_for(2);
        ai_supply_cap = ai.supply_cap.max(ai_supply_cap);
        let workers = ai
            .entities
            .iter()
            .filter(|e| e.owner == 2 && e.kind == kinds::WORKER)
            .count();
        let riflemen = ai
            .entities
            .iter()
            .filter(|e| e.owner == 2 && e.kind == kinds::RIFLEMAN)
            .count();
        max_workers = max_workers.max(workers);
        max_riflemen = max_riflemen.max(riflemen);
        if ai
            .entities
            .iter()
            .any(|e| e.owner == 2 && e.kind == kinds::BARRACKS)
        {
            ever_had_barracks = true;
        }

        // Any human entity below full hp means an AI attack landed.
        let human = game.snapshot_for(1);
        if human
            .entities
            .iter()
            .any(|e| e.owner == 1 && e.hp < e.max_hp)
        {
            human_damaged = true;
        }

        if max_workers >= target_workers
            && ai_supply_cap > config::CITY_CENTRE_SUPPLY
            && max_pending_depot_builders <= 1
            && gathering_workers_after_depot > 0
            && ever_had_barracks
            && max_riflemen > 0
            && human_damaged
        {
            break;
        }
    }

    assert!(
        max_workers > config::STARTING_WORKERS as usize,
        "AI should train workers beyond the {} it starts with (saw {max_workers})",
        config::STARTING_WORKERS
    );
    assert!(
            max_workers >= target_workers,
            "AI should train enough workers to saturate its starting steel patches (target {}, saw {max_workers})",
            target_workers
        );
    assert!(
        ai_supply_cap > config::CITY_CENTRE_SUPPLY,
        "AI should build a depot to raise supply above the City Centre's {} (saw {ai_supply_cap})",
        config::CITY_CENTRE_SUPPLY
    );
    assert!(
            max_pending_depot_builders <= 1,
            "AI should never have more than one depot builder pending simultaneously (saw {max_pending_depot_builders})"
        );
    assert!(
        gathering_workers_after_depot > 0,
        "AI should have workers mining again after the depot completes"
    );
    assert!(ever_had_barracks, "AI should build a barracks");
    assert!(max_riflemen > 0, "AI should produce riflemen");
    assert!(
        human_damaged,
        "AI riflemen should reach and damage the human base"
    );

    // Replay determinism: the same command log fed into a fresh game must reproduce
    // the exact events and final snapshots.
    selfplay::assert_replay_matches_live(&game, &players, &event_log).unwrap_or_else(|failure| {
        panic!("AI replay determinism failed: {}", failure.reason());
    });
}

#[test]
fn base_ai_tracks_pending_depot_build_order() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0x1234_5678);
    let mut saw_pending_without_scaffold = false;
    let mut max_pending_depot_builders = 0usize;
    let mut gathering_workers_while_pending = 0usize;

    for _ in 0..2000 {
        game.tick();

        let pending_depot_builders: Vec<_> = game
            .entities
            .iter()
            .filter(|e| e.owner == 2 && e.kind == EntityKind::Worker)
            .filter(|e| {
                matches!(
                    e.order().build_intent_tile(),
                    Some((EntityKind::Depot, _, _))
                )
            })
            .collect();
        let scaffold_exists = game
            .entities
            .iter()
            .any(|e| e.owner == 2 && e.kind == EntityKind::Depot && e.under_construction());

        if !pending_depot_builders.is_empty() && !scaffold_exists {
            saw_pending_without_scaffold = true;
            max_pending_depot_builders =
                max_pending_depot_builders.max(pending_depot_builders.len());
            gathering_workers_while_pending =
                gathering_workers_while_pending.max(count_ai_gathering_workers(&game, 2));
        }
    }

    assert!(
        saw_pending_without_scaffold,
        "test should observe the window where a depot order is pending before the scaffold spawns"
    );
    assert!(
            max_pending_depot_builders <= 1,
            "AI should track pending depot build intents and keep them to one worker (saw {max_pending_depot_builders})"
        );
    assert!(
            gathering_workers_while_pending >= (config::STARTING_WORKERS as usize).saturating_sub(1),
            "AI should keep nearly all starting workers gathering while one depot order is pending (saw {gathering_workers_while_pending})"
        );
}

#[test]
fn base_ai_reassigns_idle_workers_to_steel() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0x1234_5678);

    // Advance to a point where the AI has active gathering assignments.
    for _ in 0..30 {
        game.tick();
    }

    let idle_worker = game
        .entities
        .iter()
        .find(|e| {
            e.owner == 2 && e.kind == EntityKind::Worker && matches!(e.order(), Order::Gather(_))
        })
        .map(|e| e.id)
        .expect("AI should have a gathering worker to perturb");
    game.entities.release_miner(idle_worker);
    if let Some(worker) = game.entities.get_mut(idle_worker) {
        worker.clear_orders();
    }

    let mut reassigned_to = None;
    for _ in 0..20 {
        game.tick();
        if let Some(worker) = game.entities.get(idle_worker) {
            if let Some(node) = worker.order().gather_node() {
                reassigned_to = Some(node);
                break;
            }
        }
    }

    assert!(
        reassigned_to.is_some(),
        "AI should send an idle worker back to gather on a later decision tick"
    );
}

/// Adding an AI must not perturb a human-only game's construction: an all-human match has no
/// controllers and behaves exactly as before.
#[test]
fn no_ai_controllers_without_ai_players() {
    let players = [PlayerInit {
        id: 1,
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let game = Game::new(&players, 0x1234_5678);
    assert!(
        game.ai.is_empty(),
        "a human-only match has no AI controllers"
    );
}

#[test]
fn replay_games_preserve_ai_identity_without_controllers() {
    let players = [PlayerInit {
        id: 1,
        name: "Computer".into(),
        color: "#fff".into(),
        is_ai: true,
    }];
    let game = Game::new_without_ai_controllers(&players, 0x1234_5678);

    assert!(
        game.ai.is_empty(),
        "replays should not run live AI controllers"
    );
    assert!(
        game.players
            .iter()
            .any(|player| player.id == 1 && player.is_ai),
        "replays must preserve AI identity for deterministic simulation rules"
    );
    assert!(
        game.player_inits()
            .iter()
            .any(|player| player.id == 1 && player.is_ai),
        "replay artifacts must serialize the original AI identity"
    );
}

#[test]
fn gather_command_ignores_nodes_without_nearby_completed_cc() {
    let players = [PlayerInit {
        id: 1,
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    let worker = game
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .expect("starting worker");
    let cc = game
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::CityCentre)
        .expect("starting City Centre");
    let world = game.map.world_size_px();
    let far_x = if cc.pos_x < world * 0.5 {
        world - config::TILE_SIZE as f32 * 0.5
    } else {
        config::TILE_SIZE as f32 * 0.5
    };
    let far_y = if cc.pos_y < world * 0.5 {
        world - config::TILE_SIZE as f32 * 0.5
    } else {
        config::TILE_SIZE as f32 * 0.5
    };
    let far_node = game
        .entities
        .spawn_node(EntityKind::Steel, far_x, far_y)
        .expect("far resource node");

    game.enqueue(
        1,
        Command::Gather {
            units: vec![worker],
            node: far_node,
            queued: false,
        },
    );
    game.tick();

    let worker_order = game.entities.get(worker).expect("worker survives").order();
    assert!(
        !matches!(worker_order, Order::Gather(_)),
        "worker should ignore gather commands for patches outside City Centre mining range"
    );
}

#[test]
fn gather_command_to_occupied_patch_is_ignored() {
    let players = [PlayerInit {
        id: 1,
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    let mut workers: Vec<u32> = game
        .entities
        .iter()
        .filter(|e| e.owner == 1 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .collect();
    workers.sort_unstable();
    let holder = workers[0];
    let ordered = workers[1];
    let node = game
        .entities
        .iter()
        .find(|e| e.is_node())
        .map(|e| e.id)
        .expect("starting resource node");
    let (node_x, node_y) = game
        .entities
        .get(node)
        .map(|e| (e.pos_x, e.pos_y))
        .expect("node position");

    {
        let holder_entity = game.entities.get_mut(holder).expect("holder worker");
        holder_entity.pos_x = node_x;
        holder_entity.pos_y = node_y;
        holder_entity.set_order(Order::gather(node));
        holder_entity.mark_gather_phase(GatherPhase::Harvesting);
    }
    assert!(game.entities.claim_miner(node, holder));
    {
        let ordered_entity = game.entities.get_mut(ordered).expect("ordered worker");
        ordered_entity.pos_x = node_x + 4.0;
        ordered_entity.pos_y = node_y;
    }

    game.enqueue(
        1,
        Command::Gather {
            units: vec![ordered],
            node,
            queued: false,
        },
    );
    game.tick();

    let ordered_worker = game.entities.get(ordered).expect("worker survives");
    assert!(
        !matches!(ordered_worker.order(), Order::Gather(_)),
        "occupied patches should reject gather orders so extra workers do not move onto them"
    );
    assert_eq!(
        game.entities.node_slot_holder(node),
        Some(holder),
        "the original worker should remain the single active miner"
    );
}

#[test]
fn worker_already_touching_resource_body_starts_harvesting() {
    let players = [PlayerInit {
        id: 1,
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    let worker = game
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .expect("starting worker");
    let node = game
        .entities
        .iter()
        .find(|e| e.is_node())
        .map(|e| e.id)
        .expect("starting resource node");
    let (node_x, node_y) = game
        .entities
        .get(node)
        .map(|e| (e.pos_x, e.pos_y))
        .expect("node position");
    let worker_radius = game.entities.get(worker).expect("worker").radius();
    let node_radius = game.entities.get(node).expect("node").radius();
    {
        let worker_entity = game.entities.get_mut(worker).expect("worker");
        worker_entity.pos_x = node_x + worker_radius + node_radius - 1.0;
        worker_entity.pos_y = node_y;
    }

    game.enqueue(
        1,
        Command::Gather {
            units: vec![worker],
            node,
            queued: false,
        },
    );
    game.tick();

    assert_eq!(
        game.entities.get(worker).and_then(|e| e.gather_phase()),
        Some(GatherPhase::Harvesting),
        "worker already touching the resource body should not need to reach the exact node center"
    );
}

#[test]
fn active_mining_stops_when_nearby_cc_is_removed() {
    let players = [PlayerInit {
        id: 1,
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    let worker = game
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .expect("starting worker");
    let (worker_x, worker_y) = game
        .entities
        .get(worker)
        .map(|e| (e.pos_x, e.pos_y))
        .expect("worker position");
    let node = game
        .entities
        .iter()
        .filter(|e| e.is_node())
        .min_by(|a, b| {
            let da = (a.pos_x - worker_x).powi(2) + (a.pos_y - worker_y).powi(2);
            let db = (b.pos_x - worker_x).powi(2) + (b.pos_y - worker_y).powi(2);
            da.total_cmp(&db).then_with(|| a.id.cmp(&b.id))
        })
        .map(|e| e.id)
        .expect("starting resource node");

    game.enqueue(
        1,
        Command::Gather {
            units: vec![worker],
            node,
            queued: false,
        },
    );
    for _ in 0..600 {
        game.tick();
        if matches!(
            game.entities.get(worker).and_then(|e| e.gather_phase()),
            Some(GatherPhase::Harvesting)
        ) {
            break;
        }
    }
    assert_eq!(
        game.entities.get(worker).and_then(|e| e.gather_phase()),
        Some(GatherPhase::Harvesting),
        "worker should reach and latch the starting patch before the City Centre is removed"
    );

    let cc = game
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::CityCentre)
        .map(|e| e.id)
        .expect("starting City Centre");
    game.entities.remove(cc);
    let steel_before = game.players.iter().find(|p| p.id == 1).unwrap().steel;

    for _ in 0..(config::HARVEST_TICKS + 5) {
        game.tick();
    }

    let steel_after = game.players.iter().find(|p| p.id == 1).unwrap().steel;
    assert_eq!(
        steel_after, steel_before,
        "mining should not continue without a City Centre"
    );
    assert!(
        !matches!(
            game.entities.get(worker).map(|e| e.order()),
            Some(Order::Gather(_))
        ),
        "worker should go idle when its mining City Centre disappears"
    );
}

#[test]
fn ai_with_building_but_no_units_is_eliminated() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0x1234_5678);
    let ai_units: Vec<u32> = game
        .entities
        .iter()
        .filter(|e| e.owner == 2 && e.is_unit())
        .map(|e| e.id)
        .collect();
    for id in ai_units {
        game.entities.remove(id);
    }

    assert!(
        !game.alive_players().contains(&2),
        "AI players have special elimination: no units means defeated"
    );
}

#[test]
fn resource_snapshots_include_remaining_even_through_fog() {
    let players = [
        PlayerInit {
            id: 1,
            name: "A".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            name: "B".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let game = Game::new_for_replay(&players, 0x1234_5678);
    let snapshot = game.snapshot_for(1);
    let resources: Vec<_> = snapshot
        .entities
        .iter()
        .filter(|e| e.owner == 0 && (e.kind == kinds::STEEL || e.kind == kinds::OIL))
        .collect();

    assert!(
        resources.iter().all(|e| e.remaining.is_some()),
        "current resource snapshots expose remaining for all static resource nodes"
    );
}

/// A one-player sandbox with no commands must still be deterministic: fog, supply, and the
/// spatial index rebuild identically every tick, and replaying the empty command log
/// reproduces the same final snapshot.
#[test]
fn no_commands_one_player_is_deterministic() {
    let players = [PlayerInit {
        id: 1,
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new(&players, 0x1234_5678);

    let mut event_log = Vec::new();
    for tick in 1..=300 {
        for (player_id, events) in game.tick() {
            for event in events {
                event_log.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert!(
        event_log.is_empty(),
        "a one-player sandbox with no commands should emit no events"
    );

    selfplay::assert_replay_matches_live(&game, &players, &event_log).unwrap_or_else(|failure| {
        panic!(
            "one-player no-commands replay determinism failed: {}",
            failure.reason()
        );
    });
}

/// Every player must receive the same relative resource layout, and all starting resources
/// must fall within the configured min/max distance from the City Centre.
#[test]
fn spawn_resource_distances_are_fair_and_symmetric() {
    let counts = [1, 2, 3, 4];
    for &pc in &counts {
        let players: Vec<PlayerInit> = (1..=pc)
            .map(|id| PlayerInit {
                id,
                name: format!("P{id}"),
                color: "#fff".into(),
                is_ai: false,
            })
            .collect();
        let game = Game::new_for_replay(&players, 0x1234_5678);

        let mut all_player_dists: Vec<Vec<(EntityKind, f32)>> = Vec::new();
        for p in &game.players {
            let cc = game
                .entities
                .iter()
                .find(|e| e.owner == p.id && e.kind == EntityKind::CityCentre)
                .expect("City Centre exists for every player");

            let mut dists = Vec::new();
            for e in game.entities.iter() {
                if e.owner != 0 || (!e.is_node()) {
                    continue;
                }
                let d_x = e.pos_x - cc.pos_x;
                let d_y = e.pos_y - cc.pos_y;
                let dist_tiles = (d_x * d_x + d_y * d_y).sqrt() / config::TILE_SIZE as f32;

                // Only consider nodes that belong to this player's start cluster.
                if dist_tiles <= config::CC_RESOURCE_MAX_DIST_TILES + 1.0 {
                    dists.push((e.kind, dist_tiles));
                    assert!(
                        dist_tiles >= config::CC_RESOURCE_MIN_DIST_TILES,
                        "player {} has a {:?} node too close ({:.2} tiles) to their City Centre",
                        p.id,
                        e.kind,
                        dist_tiles
                    );
                    assert!(
                        dist_tiles <= config::CC_RESOURCE_MAX_DIST_TILES,
                        "player {} has a {:?} node too far ({:.2} tiles) from their City Centre",
                        p.id,
                        e.kind,
                        dist_tiles
                    );
                }
            }
            // Sort for deterministic comparison.
            dists.sort_by(|a, b| {
                let kind_ord = a.0.to_protocol_str().cmp(b.0.to_protocol_str());
                if kind_ord != std::cmp::Ordering::Equal {
                    return kind_ord;
                }
                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
            });
            all_player_dists.push(dists);
        }

        // Every player in the same match must have identical distance sets.
        if let Some(first) = all_player_dists.first() {
            for (i, other) in all_player_dists.iter().enumerate().skip(1) {
                assert_eq!(
                    first.len(),
                    other.len(),
                    "player count {}: player {} has a different number of nearby resources",
                    pc,
                    i + 1
                );
                for (j, ((ek_a, da), (ek_b, db))) in first.iter().zip(other.iter()).enumerate() {
                    assert_eq!(*ek_a, *ek_b, "mismatched resource kind at index {j}");
                    assert!(
                            (da - db).abs() < 0.01,
                            "player count {pc}: resource {j} distance mismatch — player 1 has {:.3} tiles, player {} has {:.3} tiles",
                            da,
                            i + 1,
                            db
                        );
                }
            }
        }
    }
}
