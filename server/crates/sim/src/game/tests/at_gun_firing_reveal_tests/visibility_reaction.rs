use super::*;
use crate::protocol::DEFAULT_FACTION_ID;

mod lifecycle;

fn allied_three_players() -> [PlayerInit; 3] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "Counter".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 1,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "Ally".into(),
            color: "#0ff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "Enemy".into(),
            color: "#f00".into(),
            is_ai: false,
        },
    ]
}

fn grant_ordinary_sight_near(game: &mut Game, owner: u32, target: u32) {
    let target = game
        .state
        .entities
        .get(target)
        .expect("sight target should exist");
    game.state
        .entities
        .spawn_unit(owner, EntityKind::ScoutPlane, target.pos_x, target.pos_y)
        .expect("vision scout plane should spawn");
    refresh_visibility_for_test(game);
}

#[test]
fn ordinary_sight_bypasses_an_active_firing_reveal_before_reaction_starts() {
    let (mut game, enemy_at, _counter_at) = hidden_enemy_at_gun_with_counter_fixture();
    game.tick();
    let hp_after_reveal = game.state.entities.get(enemy_at).expect("enemy AT gun").hp;
    game.state
        .entities
        .get_mut(enemy_at)
        .expect("enemy AT gun")
        .set_attack_cd(u32::MAX);
    grant_ordinary_sight_near(&mut game, 1, enemy_at);

    assert!(game
        .state
        .fog
        .active_firing_reveal_episode(1, enemy_at)
        .is_some());
    assert_eq!(game.state.fog.firing_reveal_only_episode(1, enemy_at), None);
    game.tick();
    assert!(game
        .state
        .entities
        .get(enemy_at)
        .is_none_or(|entity| entity.hp < hp_after_reveal));
}

#[test]
fn ordinary_sight_bypasses_an_in_progress_firing_reveal_reaction_gate() {
    let (mut game, enemy_at, counter_at) = hidden_enemy_at_gun_with_counter_fixture();
    game.tick();
    let hp_after_reveal = game.state.entities.get(enemy_at).expect("enemy AT gun").hp;
    game.state
        .entities
        .get_mut(enemy_at)
        .expect("enemy AT gun")
        .set_attack_cd(u32::MAX);
    game.tick();
    assert_eq!(
        game.state.entities.get(enemy_at).expect("enemy AT gun").hp,
        hp_after_reveal
    );
    assert_eq!(
        game.state
            .entities
            .get(counter_at)
            .expect("counter AT gun")
            .attack_cd(),
        0
    );

    grant_ordinary_sight_near(&mut game, 1, enemy_at);
    game.tick();
    assert!(game
        .state
        .entities
        .get(enemy_at)
        .is_none_or(|entity| entity.hp < hp_after_reveal));
}

fn allied_reveal_reaction_fixture() -> (Game, u32, u32, (f32, f32), u32) {
    let mut game = empty_flat_game(&allied_three_players());
    let victim_pos = game.state.map.tile_center(10, 10);
    let enemy_pos = (victim_pos.0 + config::TILE_SIZE as f32 * 5.0, victim_pos.1);
    let counter_pos = (victim_pos.0, victim_pos.1 + config::TILE_SIZE as f32 * 10.0);
    game.state
        .entities
        .spawn_building(1, EntityKind::CityCentre, victim_pos.0, victim_pos.1, true)
        .expect("city centre");
    let counter = game
        .state
        .entities
        .spawn_unit(1, EntityKind::AntiTankGun, counter_pos.0, counter_pos.1)
        .expect("counter AT gun");
    let enemy = game
        .state
        .entities
        .spawn_unit(3, EntityKind::AntiTankGun, enemy_pos.0, enemy_pos.1)
        .expect("enemy AT gun");
    deploy_anti_tank_gun_toward(&mut game, counter, enemy_pos);
    deploy_anti_tank_gun_toward(&mut game, enemy, victim_pos);
    refresh_visibility_for_test(&mut game);

    game.tick();
    let hp_after_reveal = game.state.entities.get(enemy).expect("enemy AT gun").hp;
    game.state
        .entities
        .get_mut(enemy)
        .expect("enemy AT gun")
        .set_attack_cd(u32::MAX);
    game.tick();
    assert_eq!(
        game.state.entities.get(enemy).expect("enemy AT gun").hp,
        hp_after_reveal
    );

    (game, counter, enemy, enemy_pos, hp_after_reveal)
}

fn grant_allied_ordinary_sight(game: &mut Game, enemy: u32, enemy_pos: (f32, f32)) {
    game.state
        .entities
        .spawn_unit(
            2,
            EntityKind::Worker,
            enemy_pos.0 + config::TILE_SIZE as f32 * 4.0,
            enemy_pos.1,
        )
        .expect("allied spotter");
    refresh_visibility_for_test(game);
    assert!(game
        .state
        .fog
        .firing_reveal_only_episode(1, enemy)
        .is_some());
    assert_eq!(game.state.fog.firing_reveal_only_episode(2, enemy), None);
}

#[test]
fn allied_ordinary_sight_bypasses_reaction_for_explicit_team_visible_fire() {
    let (mut game, counter, enemy, enemy_pos, hp_after_reveal) = allied_reveal_reaction_fixture();
    grant_allied_ordinary_sight(&mut game, enemy, enemy_pos);
    game.enqueue(
        1,
        Command::Attack {
            units: vec![counter],
            target: enemy,
            queued: false,
        },
    );
    game.tick();

    assert!(game
        .state
        .entities
        .get(enemy)
        .is_none_or(|entity| entity.hp < hp_after_reveal));
}

#[test]
fn allied_ordinary_sight_bypasses_reaction_for_team_visible_auto_acquisition() {
    let (mut game, _counter, enemy, enemy_pos, hp_after_reveal) = allied_reveal_reaction_fixture();
    grant_allied_ordinary_sight(&mut game, enemy, enemy_pos);
    game.tick();

    assert!(game
        .state
        .entities
        .get(enemy)
        .is_none_or(|entity| entity.hp < hp_after_reveal));
}

#[test]
fn repeated_hidden_shots_extend_one_stable_reveal_episode() {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let shooter_pos = game.state.map.tile_center(30, 30);
    let shooter = game
        .state
        .entities
        .spawn_unit(2, EntityKind::MachineGunner, shooter_pos.0, shooter_pos.1)
        .expect("hidden shooter");
    refresh_visibility_for_test(&mut game);
    let teams = game.team_relations();
    firing_reveal::record_firing_reveals_for_victim_team(
        &mut game.state.firing_reveals,
        [1],
        &game.state.fog,
        &teams,
        1,
        2,
        shooter,
        shooter_pos,
        0,
        config::TICK_HZ,
    );
    refresh_visibility_for_test(&mut game);
    firing_reveal::record_firing_reveals_for_victim_team(
        &mut game.state.firing_reveals,
        [1],
        &game.state.fog,
        &teams,
        1,
        2,
        shooter,
        shooter_pos,
        10,
        config::TICK_HZ,
    );

    assert_eq!(game.state.firing_reveals.len(), 1);
    let source = game.state.firing_reveals[0];
    assert_eq!(source.started_at_tick(), 0);
    assert!(source.is_active_at(config::TICK_HZ + config::TICK_HZ / 2 + 5));
}

#[test]
fn colocated_revealed_entities_both_keep_reveal_only_provenance() {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let hidden_pos = game.state.map.tile_center(30, 30);
    let first = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Rifleman, hidden_pos.0, hidden_pos.1)
        .expect("first hidden unit");
    let second = game
        .state
        .entities
        .spawn_unit(2, EntityKind::MachineGunner, hidden_pos.0, hidden_pos.1)
        .expect("second hidden unit");
    let bystander = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Worker, hidden_pos.0, hidden_pos.1)
        .expect("colocated bystander");
    let teams = game.team_relations();
    firing_reveal::record_global_firing_reveals_for_enemy_players(
        &mut game.state.firing_reveals,
        &[1, 2],
        &teams,
        2,
        first,
        0,
        config::TICK_HZ,
    );
    refresh_visibility_for_test(&mut game);
    assert_eq!(
        game.state
            .fog
            .firing_reveal_only_source_at_world(1, hidden_pos.0, hidden_pos.1),
        Some(crate::game::entity::FiringRevealEpisode {
            viewer: 1,
            source_entity: first,
            started_at_tick: 0,
        }),
        "a colocated non-firer is visible through the firing entity's tile provenance"
    );
    firing_reveal::record_firing_reveals_for_victim_team(
        &mut game.state.firing_reveals,
        [1],
        &game.state.fog,
        &teams,
        1,
        2,
        second,
        hidden_pos,
        0,
        config::TICK_HZ,
    );
    refresh_visibility_for_test(&mut game);

    assert_eq!(game.state.fog.firing_reveal_only_episode(1, first), Some(0));
    assert_eq!(
        game.state.fog.firing_reveal_only_episode(1, second),
        Some(0)
    );
    assert_eq!(
        game.state.fog.firing_reveal_only_episode(1, bystander),
        None
    );
}

#[test]
fn reveal_provenance_does_not_follow_a_source_onto_an_ordinary_visible_tile_mid_tick() {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let hidden_pos = game.state.map.tile_center(30, 30);
    let visible_pos = game.state.map.tile_center(8, 8);
    game.state
        .entities
        .spawn_unit(1, EntityKind::Worker, visible_pos.0, visible_pos.1)
        .expect("ordinary spotter");
    let shooter = game
        .state
        .entities
        .spawn_unit(2, EntityKind::MachineGunner, hidden_pos.0, hidden_pos.1)
        .expect("hidden shooter");
    let teams = game.team_relations();
    firing_reveal::record_global_firing_reveals_for_enemy_players(
        &mut game.state.firing_reveals,
        &[1, 2],
        &teams,
        2,
        shooter,
        0,
        config::TICK_HZ,
    );
    refresh_visibility_for_test(&mut game);
    assert!(game
        .state
        .fog
        .firing_reveal_only_source_at_world(1, hidden_pos.0, hidden_pos.1)
        .is_some());

    game.state
        .entities
        .get_mut(shooter)
        .expect("shooter")
        .set_position(visible_pos.0, visible_pos.1);
    assert!(game
        .state
        .fog
        .is_visible_world(1, visible_pos.0, visible_pos.1));
    assert_eq!(
        game.state
            .fog
            .firing_reveal_only_source_at_world(1, visible_pos.0, visible_pos.1),
        None,
        "entity-level provenance must not delay fire on an ordinarily visible destination tile"
    );
}

fn first_counterfire_tick(game: &mut Game, enemy_at: u32, hp_before: u32) -> u32 {
    for _ in 0..=config::TICK_HZ + 1 {
        game.tick();
        if game
            .state
            .entities
            .get(enemy_at)
            .is_none_or(|entity| entity.hp < hp_before)
        {
            return game.tick_count();
        }
    }
    panic!("counterfire should occur within the reaction window");
}

#[test]
fn checkpoint_restore_preserves_active_reveal_reaction_fire_tick() {
    let (mut baseline, enemy_at, _counter_at) = hidden_enemy_at_gun_with_counter_fixture();
    baseline.tick();
    let hp_after_reveal = baseline
        .state
        .entities
        .get(enemy_at)
        .expect("enemy AT gun")
        .hp;
    baseline
        .state
        .entities
        .get_mut(enemy_at)
        .expect("enemy AT gun")
        .set_attack_cd(u32::MAX);
    baseline.tick();
    baseline.tick();

    let checkpoint = baseline
        .checkpoint_payload_text_for_test()
        .expect("checkpoint");
    let mut missing_provenance: serde_json::Value =
        serde_json::from_str(&checkpoint).expect("checkpoint JSON");
    assert!(missing_provenance["fog"]["firingRevealVisibility"]
        .as_object()
        .is_some_and(|entries| !entries.is_empty()));
    missing_provenance["fog"]["firingRevealVisibility"] = serde_json::json!({});
    let missing_provenance = serde_json::to_string(&missing_provenance).expect("checkpoint JSON");
    assert!(Game::restore_checkpoint_payload_text_for_test(
        &missing_provenance,
        baseline.state.map.clone(),
        baseline.state.map_metadata.clone(),
    )
    .is_err());

    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &checkpoint,
        baseline.state.map.clone(),
        baseline.state.map_metadata.clone(),
    )
    .expect("checkpoint should restore");

    assert_eq!(
        first_counterfire_tick(&mut restored, enemy_at, hp_after_reveal),
        first_counterfire_tick(&mut baseline, enemy_at, hp_after_reveal)
    );
}

#[test]
fn move_spam_does_not_block_scout_car_fire_on_ordinarily_visible_revealed_mg() {
    let mut game = empty_flat_game(&human_vs_ai_players());
    let scout_pos = game.state.map.tile_center(8, 12);
    let mg_pos = game.state.map.tile_center(13, 12);
    let scout = game
        .state
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, scout_pos.0, scout_pos.1)
        .expect("scout car should spawn");
    let machine_gunner = game
        .state
        .entities
        .spawn_unit(2, EntityKind::MachineGunner, mg_pos.0, mg_pos.1)
        .expect("machine gunner should spawn");
    {
        let scout = game.state.entities.get_mut(scout).expect("scout car");
        scout.set_facing(0.0);
        scout.set_weapon_facing(0.0);
    }
    {
        let machine_gunner = game
            .state
            .entities
            .get_mut(machine_gunner)
            .expect("machine gunner");
        machine_gunner.set_attack_cd(u32::MAX);
        machine_gunner
            .movement
            .as_mut()
            .expect("movement")
            .occupied_trench_id = Some(1);
    }
    let teams = game.team_relations();
    let reveal_started_at_tick = game.tick_count();
    firing_reveal::record_global_firing_reveals_for_enemy_players(
        &mut game.state.firing_reveals,
        &[1, 2],
        &teams,
        2,
        machine_gunner,
        reveal_started_at_tick,
        config::TICK_HZ * 3,
    );
    refresh_visibility_for_test(&mut game);

    assert!(game.state.fog.is_visible_world(1, mg_pos.0, mg_pos.1));
    assert!(game
        .state
        .fog
        .active_firing_reveal_episode(1, machine_gunner)
        .is_some());
    assert_eq!(
        game.state.fog.firing_reveal_only_episode(1, machine_gunner),
        None
    );
    let hp_before = game
        .state
        .entities
        .get(machine_gunner)
        .expect("machine gunner")
        .hp;

    for _ in 0..5 {
        game.enqueue(
            1,
            Command::Move {
                units: vec![scout],
                x: mg_pos.0 - config::TILE_SIZE as f32 * 2.0,
                y: mg_pos.1,
                queued: false,
            },
        );
        game.tick();
    }
    assert!(game
        .state
        .entities
        .get(machine_gunner)
        .is_none_or(|entity| entity.hp < hp_before));
}
