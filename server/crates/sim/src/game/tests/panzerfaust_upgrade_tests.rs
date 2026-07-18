use super::fixtures::empty_flat_game;
use super::*;
use crate::game::upgrade::UpgradeKind;

fn players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ]
}

fn spawn_on_tile(game: &mut Game, owner: u32, kind: EntityKind, tile_x: u32, tile_y: u32) -> u32 {
    let (x, y) = game.state.map.tile_center(tile_x, tile_y);
    game.state
        .entities
        .spawn_unit(owner, kind, x, y)
        .expect("unit should spawn")
}

fn refresh_world(game: &mut Game) {
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let player_ids: Vec<u32> = game.state.players.iter().map(|player| player.id).collect();
    game.state
        .fog
        .recompute(&player_ids, &game.state.entities, &game.state.map);
}

pub(super) fn panzerfaust_fixture() -> (Game, u32, u32) {
    let mut game = empty_flat_game(&players());
    let rifleman = spawn_on_tile(&mut game, 1, EntityKind::Rifleman, 8, 8);
    let tank = spawn_on_tile(&mut game, 2, EntityKind::Tank, 11, 8);
    game.state
        .entities
        .get_mut(rifleman)
        .expect("rifleman")
        .set_invulnerable(true);
    unlock_panzerfausts(&mut game, 1);
    arm_newly_spawned_rifleman(&mut game, 1, rifleman);
    refresh_world(&mut game);
    (game, rifleman, tank)
}

pub(super) fn enqueue_attack(game: &mut Game, rifleman: u32, target: u32, queued: bool) {
    game.enqueue(
        1,
        Command::Attack {
            units: vec![rifleman],
            target,
            queued,
        },
    );
}

pub(super) fn player_events(events: &[(u32, Vec<Event>)], player_id: u32) -> &[Event] {
    events
        .iter()
        .find(|(id, _)| *id == player_id)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[])
}

fn unlock_panzerfausts(game: &mut Game, player_id: u32) {
    game.state
        .players
        .iter_mut()
        .find(|player| player.id == player_id)
        .expect("player should exist")
        .upgrades
        .insert(UpgradeKind::Panzerfausts);
}

fn arm_newly_spawned_rifleman(game: &mut Game, player_id: u32, rifleman: u32) {
    crate::game::services::production::sync_spawned_upgrade_effects(
        &mut game.state.entities,
        &game.state.players,
        player_id,
        rifleman,
    );
}

fn launch_count(events: &[(u32, Vec<Event>)], player_id: u32, rifleman: u32) -> usize {
    events
        .iter()
        .find(|(id, _)| *id == player_id)
        .map(|(_, events)| {
            events
                .iter()
                .filter(|event| {
                    matches!(event, Event::PanzerfaustLaunch { from, .. } if *from == rifleman)
                })
                .count()
        })
        .unwrap_or(0)
}

#[test]
fn upgrade_does_not_arm_existing_riflemen() {
    let mut game = empty_flat_game(&players());
    let rifleman = spawn_on_tile(&mut game, 1, EntityKind::Rifleman, 8, 8);
    let machine_gunner = spawn_on_tile(&mut game, 1, EntityKind::MachineGunner, 9, 8);
    refresh_world(&mut game);

    assert_eq!(
        game.snapshot_for(1)
            .entities
            .iter()
            .find(|entity| entity.id == rifleman)
            .and_then(|entity| entity.panzerfaust_loaded),
        None
    );

    unlock_panzerfausts(&mut game, 1);

    let snapshot = game.snapshot_for(1);
    assert_eq!(
        snapshot
            .entities
            .iter()
            .find(|entity| entity.id == rifleman)
            .and_then(|entity| entity.panzerfaust_loaded),
        None
    );
    assert_eq!(
        snapshot
            .entities
            .iter()
            .find(|entity| entity.id == machine_gunner)
            .and_then(|entity| entity.panzerfaust_loaded),
        None
    );
}

#[test]
fn loaded_rifleman_fires_once_at_a_tank_and_stays_a_rifleman() {
    let (mut game, rifleman, tank) = panzerfaust_fixture();
    let starting_hp = game.state.entities.get(tank).expect("tank").hp;

    game.enqueue(
        1,
        Command::Attack {
            units: vec![rifleman],
            target: tank,
            queued: false,
        },
    );

    let mut launches = 0;
    for _ in 0..120 {
        launches += launch_count(&game.tick(), 1, rifleman);
    }

    let attacker = game.state.entities.get(rifleman).expect("rifleman remains");
    assert_eq!(attacker.kind, EntityKind::Rifleman);
    assert_eq!(launches, 1);
    assert_eq!(
        game.snapshot_for(1)
            .entities
            .iter()
            .find(|entity| entity.id == rifleman)
            .and_then(|entity| entity.panzerfaust_loaded),
        Some(false)
    );
    assert!(
        game.state.entities.get(tank).expect("tank").hp
            <= starting_hp.saturating_sub(crate::rules::combat::panzerfaust_loaded_shot_damage(
                EntityKind::Tank,
                Some(crate::rules::terrain::TerrainKind::Open),
            ),),
        "the detached Panzerfaust impact should land while the spent Rifleman resumes rifle fire"
    );
}

#[test]
fn automatic_panzerfaust_fire_only_targets_real_vehicles_in_range() {
    let mut game = empty_flat_game(&players());
    let rifleman = spawn_on_tile(&mut game, 1, EntityKind::Rifleman, 8, 8);
    let infantry = spawn_on_tile(&mut game, 2, EntityKind::Rifleman, 10, 8);
    let mortar = spawn_on_tile(&mut game, 2, EntityKind::MortarTeam, 11, 8);
    unlock_panzerfausts(&mut game, 1);
    arm_newly_spawned_rifleman(&mut game, 1, rifleman);
    refresh_world(&mut game);

    let mut launches = 0;
    for _ in 0..45 {
        launches += launch_count(&game.tick(), 1, rifleman);
    }
    assert_eq!(launches, 0);
    assert!(game.state.entities.get(infantry).is_some());
    assert!(game.state.entities.get(mortar).is_some());

    let scout_car = spawn_on_tile(&mut game, 2, EntityKind::ScoutCar, 11, 9);
    refresh_world(&mut game);
    for _ in 0..45 {
        launches += launch_count(&game.tick(), 1, rifleman);
    }
    assert_eq!(launches, 1);
    assert!(game.state.entities.get(scout_car).is_none());
}

#[test]
fn out_of_range_vehicle_does_not_suppress_normal_rifle_fire() {
    let mut game = empty_flat_game(&players());
    let rifleman = spawn_on_tile(&mut game, 1, EntityKind::Rifleman, 8, 8);
    let infantry = spawn_on_tile(&mut game, 2, EntityKind::Rifleman, 10, 8);
    let tank = spawn_on_tile(&mut game, 2, EntityKind::Tank, 14, 8);
    unlock_panzerfausts(&mut game, 1);
    arm_newly_spawned_rifleman(&mut game, 1, rifleman);
    refresh_world(&mut game);
    let infantry_hp = game.state.entities.get(infantry).expect("infantry").hp;
    let tank_hp = game.state.entities.get(tank).expect("tank").hp;

    let mut launches = 0;
    for _ in 0..45 {
        launches += launch_count(&game.tick(), 1, rifleman);
    }

    assert_eq!(launches, 0);
    assert!(game.state.entities.get(infantry).expect("infantry").hp < infantry_hp);
    assert_eq!(game.state.entities.get(tank).expect("tank").hp, tank_hp);
}

#[test]
fn out_of_range_launcher_does_not_block_rifle_chase_and_explicit_attack_can_close() {
    let mut game = empty_flat_game(&players());
    let rifleman = spawn_on_tile(&mut game, 1, EntityKind::Rifleman, 8, 8);
    let tank = spawn_on_tile(&mut game, 2, EntityKind::Tank, 18, 8);
    unlock_panzerfausts(&mut game, 1);
    arm_newly_spawned_rifleman(&mut game, 1, rifleman);
    refresh_world(&mut game);
    let start = game
        .state
        .entities
        .get(rifleman)
        .map(|entity| (entity.pos_x, entity.pos_y))
        .expect("rifleman");

    let mut launches = 0;
    for _ in 0..45 {
        launches += launch_count(&game.tick(), 1, rifleman);
    }
    let idle_pos = game
        .state
        .entities
        .get(rifleman)
        .map(|entity| (entity.pos_x, entity.pos_y))
        .expect("rifleman");
    assert_eq!(launches, 0);
    assert_ne!(
        idle_pos, start,
        "ordinary Rifleman combat should remain active"
    );

    game.enqueue(
        1,
        Command::Attack {
            units: vec![rifleman],
            target: tank,
            queued: false,
        },
    );
    for _ in 0..300 {
        launches += launch_count(&game.tick(), 1, rifleman);
        if launches > 0 {
            break;
        }
    }
    assert_eq!(launches, 1);
}
