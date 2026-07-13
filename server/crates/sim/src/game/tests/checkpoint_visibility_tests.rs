use std::collections::HashMap;

use super::checkpoint_helpers::{
    assert_equivalent_games, player_ids, repair_after_authoritative_test_spawn,
    restore_checkpoint_and_assert_equivalent, tick_pair_and_assert_equivalent, tick_pair_for,
};
use super::fixtures::empty_flat_game;
use super::lab::{LabOp, LabOpOutcome, LabSpawnEntity};
use super::*;
use crate::game::ability_projectile::{AbilityProjectileReturnTarget, AbilityProjectileSpec};
use crate::game::ability_runtime::{
    AbilityObjectPayload, AbilityWorldObjectKind, AbilityWorldObjectSpec,
};
use crate::game::firing_reveal;
use crate::game::fog::LingeringSightSource;
use crate::game::map::{Map, MapMetadata};
use crate::game::services::occupancy::footprint_center;
use crate::rules::combat::WeaponKind;

#[test]
fn visibility_combat_checkpoint_preserves_fog_memory_trenches_and_reveals() {
    let mut baseline = empty_flat_game(&phase6_players());
    let scout = spawn_unit_at_tile(&mut baseline, 1, EntityKind::Rifleman, 20, 8);
    let ally_scout = spawn_unit_at_tile(&mut baseline, 3, EntityKind::Rifleman, 21, 8);
    spawn_unit_at_tile(&mut baseline, 4, EntityKind::Rifleman, 50, 50);
    let trench_pos = baseline.state.map.tile_center(8, 18);
    let trench = baseline
        .state
        .trenches
        .create(&baseline.state.map, trench_pos.0, trench_pos.1)
        .expect("trench should be created");
    let occupant = baseline
        .state
        .entities
        .spawn_unit(1, EntityKind::Rifleman, trench_pos.0, trench_pos.1)
        .expect("occupying rifleman should spawn");
    baseline
        .state
        .entities
        .get_mut(occupant)
        .expect("occupant should exist")
        .movement
        .as_mut()
        .expect("occupant should have movement")
        .occupied_trench_id = Some(trench);

    let depot_pos = footprint_center(&baseline.state.map, EntityKind::Depot, 23, 8);
    let remembered_depot = baseline
        .state
        .entities
        .spawn_building(2, EntityKind::Depot, depot_pos.0, depot_pos.1, true)
        .expect("enemy depot should spawn");
    let hidden_attacker_pos = baseline.state.map.tile_center(38, 38);
    let hidden_attacker = baseline
        .state
        .entities
        .spawn_unit(
            2,
            EntityKind::AntiTankGun,
            hidden_attacker_pos.0,
            hidden_attacker_pos.1,
        )
        .expect("hidden attacker should spawn");
    repair_after_authoritative_test_spawn(&mut baseline);
    assert!(
        memory_contains(&baseline, 1, remembered_depot)
            && memory_contains(&baseline, 3, remembered_depot),
        "same-team building memory should record the scouted enemy depot"
    );
    assert!(
        snapshot_entity(&baseline.snapshot_for(1), occupant)
            .and_then(|entity| entity.occupied_trench_id)
            == Some(trench),
        "owner snapshot should project occupied trench id before checkpoint"
    );

    let far = baseline.state.map.tile_center(5, 45);
    baseline
        .state
        .entities
        .get_mut(scout)
        .expect("scout should exist")
        .set_position(far.0, far.1);
    let ally_far = baseline.state.map.tile_center(7, 45);
    baseline
        .state
        .entities
        .get_mut(ally_scout)
        .expect("ally scout should exist")
        .set_position(ally_far.0, ally_far.1);
    baseline.state.entities.remove(remembered_depot);
    repair_after_authoritative_test_spawn(&mut baseline);
    assert!(
        memory_contains(&baseline, 1, remembered_depot)
            && memory_contains(&baseline, 3, remembered_depot),
        "destroyed enemy building memory should remain while its footprint is hidden"
    );

    baseline.state.lingering_sight.push(
        LingeringSightSource::new(
            1,
            hidden_attacker_pos.0,
            hidden_attacker_pos.1,
            3,
            baseline.tick_count().saturating_add(12),
        )
        .expect("lingering sight source should be valid"),
    );
    let teams = baseline.team_relations();
    let reveal_players = player_ids(&baseline);
    let reveal_tick = baseline.tick_count();
    firing_reveal::record_firing_reveals_for_victim_team(
        &mut baseline.state.firing_reveals,
        reveal_players,
        &baseline.state.fog,
        &teams,
        1,
        2,
        hidden_attacker,
        hidden_attacker_pos,
        reveal_tick,
        config::TICK_HZ,
    );
    repair_after_authoritative_test_spawn(&mut baseline);
    assert!(
        snapshot_entity(&baseline.snapshot_for(1), hidden_attacker).is_some(),
        "victim-team firing reveal should expose the hidden attacker to player 1"
    );
    assert!(
        snapshot_entity(&baseline.snapshot_for(3), hidden_attacker).is_some(),
        "victim-team firing reveal should expose the hidden attacker to living teammates"
    );
    assert!(
        snapshot_entity(&baseline.snapshot_for(4), hidden_attacker).is_none(),
        "third-party players must not receive victim-team firing reveal visibility"
    );

    let mut restored = restore_checkpoint_and_assert_equivalent(
        &baseline,
        "fog/memory/trench/reveal checkpoint import",
    );
    tick_pair_for(
        &mut baseline,
        &mut restored,
        4,
        "fog/memory/trench/reveal continuation",
    );

    let baseline_probe = baseline
        .state
        .entities
        .spawn_unit(3, EntityKind::Rifleman, depot_pos.0, depot_pos.1)
        .expect("baseline scout should spawn on remembered footprint");
    let restored_probe = restored
        .state
        .entities
        .spawn_unit(3, EntityKind::Rifleman, depot_pos.0, depot_pos.1)
        .expect("restored scout should spawn on remembered footprint");
    assert_eq!(
        baseline_probe, restored_probe,
        "post-checkpoint scout allocation should use the same stable id"
    );
    repair_after_authoritative_test_spawn(&mut baseline);
    repair_after_authoritative_test_spawn(&mut restored);
    assert_equivalent_games(
        &baseline,
        &restored,
        "remembered building cleared after restored footprint scouting",
    );
    assert!(
        !memory_contains(&baseline, 1, remembered_depot)
            && !memory_contains(&baseline, 3, remembered_depot),
        "scouting the remembered footprint should clear stale building memory"
    );
    let future_trench_pos = baseline.state.map.tile_center(9, 18);
    let baseline_trench = baseline
        .state
        .trenches
        .create(
            &baseline.state.map,
            future_trench_pos.0,
            future_trench_pos.1,
        )
        .expect("baseline post-restore trench should allocate");
    let restored_trench = restored
        .state
        .trenches
        .create(
            &restored.state.map,
            future_trench_pos.0,
            future_trench_pos.1,
        )
        .expect("restored post-restore trench should allocate");
    assert_eq!(
        baseline_trench, restored_trench,
        "trench store next id should survive checkpoint import"
    );
    repair_after_authoritative_test_spawn(&mut baseline);
    repair_after_authoritative_test_spawn(&mut restored);
    assert_equivalent_games(
        &baseline,
        &restored,
        "post-checkpoint trench allocation remains equivalent",
    );
}

#[test]
fn visibility_combat_checkpoint_preserves_smoke_ability_shells_and_combat_state() {
    let mut baseline = empty_flat_game(&phase6_players());
    let hero = spawn_unit_at_tile(&mut baseline, 1, EntityKind::Ekat, 8, 8);
    let scout = spawn_unit_at_tile(&mut baseline, 1, EntityKind::ScoutCar, 6, 9);
    let mortar = spawn_unit_at_tile(&mut baseline, 1, EntityKind::MortarTeam, 8, 13);
    let artillery = spawn_unit_at_tile(&mut baseline, 1, EntityKind::Artillery, 11, 13);
    let tank = spawn_unit_at_tile(&mut baseline, 1, EntityKind::Tank, 14, 13);
    let at_gun = spawn_unit_at_tile(&mut baseline, 1, EntityKind::AntiTankGun, 17, 13);
    let shell_target = spawn_unit_at_tile(&mut baseline, 2, EntityKind::Tank, 26, 13);
    spawn_unit_at_tile(&mut baseline, 2, EntityKind::Rifleman, 9, 8);
    spawn_unit_at_tile(&mut baseline, 4, EntityKind::Rifleman, 50, 50);

    let active_smoke = baseline.state.map.tile_center(16, 12);
    baseline
        .state
        .smokes
        .spawn(
            active_smoke.0,
            active_smoke.1,
            config::SMOKE_CLOUD_RADIUS_TILES,
            config::SMOKE_CLOUD_DURATION_TICKS,
            baseline.tick_count(),
        )
        .expect("active smoke should spawn");
    let pending_smoke = baseline.state.map.tile_center(17, 12);
    assert!(baseline.state.smokes.schedule(
        pending_smoke.0,
        pending_smoke.1,
        config::SMOKE_CLOUD_RADIUS_TILES,
        config::SMOKE_CLOUD_DURATION_TICKS,
        baseline.tick_count().saturating_add(2),
    ));

    seed_ability_runtime(&mut baseline, hero);
    seed_combat_state(&mut baseline, tank, at_gun, artillery, mortar, shell_target);
    let target = baseline
        .state
        .entities
        .get(shell_target)
        .expect("shell target should exist");
    let mut launch_events = event_map_for(&baseline);
    baseline.state.mortar_shells.schedule(
        &mut launch_events,
        &baseline.state.fog,
        &baseline.team_relations(),
        1,
        mortar,
        baseline
            .state
            .entities
            .get(mortar)
            .expect("mortar should exist")
            .pos_x,
        baseline
            .state
            .entities
            .get(mortar)
            .expect("mortar should exist")
            .pos_y,
        target.pos_x,
        target.pos_y,
        baseline.tick_count(),
        true,
    );
    baseline.state.artillery_shells.schedule(
        1,
        artillery,
        target.pos_x,
        target.pos_y,
        baseline.tick_count(),
    );
    repair_after_authoritative_test_spawn(&mut baseline);

    assert!(
        !baseline.snapshot_full_for(1).smokes.is_empty(),
        "full-world snapshot should include active smoke before checkpoint"
    );
    assert_owner_payload_privacy(&baseline, "pre-checkpoint ability projection");
    assert_eq!(
        baseline
            .state
            .entities
            .get(scout)
            .expect("scout should exist")
            .ability_uses_remaining(ability::AbilityKind::Smoke),
        Some(1),
        "seeded finite smoke uses should be part of entity-local ability state"
    );

    let mut restored = restore_checkpoint_and_assert_equivalent(
        &baseline,
        "smoke/ability/shell checkpoint import",
    );
    let mut saw_mortar_privacy = false;
    let mut saw_artillery_privacy = false;
    for tick in 0..=config::ARTILLERY_SHELL_DELAY_TICKS + 2 {
        let events = tick_pair_and_assert_equivalent(
            &mut baseline,
            &mut restored,
            &format!("smoke/ability/shell continuation tick {tick}"),
        );
        if events_for(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::MortarImpact { .. }))
        {
            assert!(
                events_for(&events, 4)
                    .iter()
                    .all(|event| !matches!(event, Event::MortarImpact { .. })),
                "third-party player should not receive hidden mortar impact payload"
            );
            saw_mortar_privacy = true;
        }
        if events_for(&events, 1)
            .iter()
            .any(|event| matches!(event, Event::ArtilleryImpact { .. }))
        {
            assert!(
                events_for(&events, 4)
                    .iter()
                    .all(|event| !matches!(event, Event::ArtilleryImpact { .. })),
                "third-party player should not receive hidden artillery impact payload"
            );
            saw_artillery_privacy = true;
        }
    }
    assert!(
        saw_mortar_privacy,
        "mortar shell should impact after restore"
    );
    assert!(
        saw_artillery_privacy,
        "artillery shell should impact after restore"
    );
    let baseline_object = spawn_runtime_return_marker_for_id_check(&mut baseline, hero);
    let restored_object = spawn_runtime_return_marker_for_id_check(&mut restored, hero);
    assert_eq!(
        baseline_object, restored_object,
        "ability runtime next id should survive checkpoint import and continuation"
    );
    assert_equivalent_games(
        &baseline,
        &restored,
        "post-checkpoint ability runtime allocation remains equivalent",
    );
}

#[test]
fn projection_privacy_checkpoint_filters_hidden_targets_and_ability_payloads() {
    let mut baseline = empty_flat_game(&phase6_players());
    let hero = spawn_unit_at_tile(&mut baseline, 1, EntityKind::Ekat, 8, 8);
    spawn_unit_at_tile(&mut baseline, 2, EntityKind::Rifleman, 9, 8);
    let tank = spawn_unit_at_tile(&mut baseline, 1, EntityKind::Tank, 22, 22);
    let hidden_target = spawn_unit_at_tile(&mut baseline, 2, EntityKind::Tank, 34, 34);
    spawn_unit_at_tile(&mut baseline, 4, EntityKind::Rifleman, 22, 23);
    seed_ability_runtime(&mut baseline, hero);
    {
        let tank = baseline
            .state
            .entities
            .get_mut(tank)
            .expect("tank should exist");
        tank.set_order(Order::attack(hidden_target));
        tank.set_target_id(Some(hidden_target));
        tank.set_weapon_facing(1.2);
    }
    repair_after_authoritative_test_spawn(&mut baseline);

    let restored =
        restore_checkpoint_and_assert_equivalent(&baseline, "projection privacy checkpoint import");
    assert_owner_payload_privacy(&restored, "restored ability projection");
    let player_snapshot = restored.snapshot_for(1);
    let player_view =
        snapshot_entity(&player_snapshot, tank).expect("owner should see own tank projection");
    assert_eq!(
        player_view.target_id,
        Some(hidden_target),
        "owner projection keeps its own hidden combat target id"
    );
    let third_party_snapshot = restored.snapshot_for(4);
    let third_party_view = snapshot_entity(&third_party_snapshot, tank)
        .expect("third party should see the tank body through local fog");
    assert_eq!(
        third_party_view.target_id, None,
        "third-party projection must not leak hidden target id after restore"
    );
    assert_eq!(
        third_party_view.weapon_facing, None,
        "third-party projection must not leak hidden target direction after restore"
    );
}

#[test]
fn visibility_combat_checkpoint_preserves_lab_god_mode_and_observer_analysis() {
    let mut baseline = Game::new_lab(
        &phase6_players()[..2],
        0x5150_0606,
        flat_lab_map(),
        lab_metadata(),
    );
    let unit_pos = baseline.state.map.tile_center(20, 20);
    let LabOpOutcome::Spawned { entity_id: unit } = baseline
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 1,
            kind: EntityKind::Rifleman,
            x: unit_pos.0,
            y: unit_pos.1,
            completed: true,
        }))
        .expect("lab spawn should succeed")
    else {
        panic!("unexpected lab spawn outcome");
    };
    baseline
        .apply_lab_op(LabOp::SetPlayerGodMode {
            player_id: 1,
            enabled: true,
        })
        .expect("lab god mode should enable");
    assert!(
        baseline
            .state
            .entities
            .get(unit)
            .expect("lab unit should exist")
            .invulnerable(),
        "lab god mode should mirror to owned units before checkpoint"
    );

    let mut restored =
        restore_checkpoint_and_assert_equivalent(&baseline, "lab god mode checkpoint import");
    assert_eq!(
        baseline.observer_analysis(),
        restored.observer_analysis(),
        "observer analysis output should rebuild identically after import"
    );
    for game in [&mut baseline, &mut restored] {
        let tick = game.tick_count();
        let entity = game
            .state
            .entities
            .get_mut(unit)
            .expect("lab unit should exist after import");
        let hp = entity.hp;
        assert!(
            !entity.apply_damage(10, Some((2, (0.0, 0.0), tick))),
            "restored lab god mode flags should keep units invulnerable"
        );
        assert_eq!(entity.hp, hp);
    }
    tick_pair_for(
        &mut baseline,
        &mut restored,
        3,
        "lab god mode observer-analysis continuation",
    );
}

fn seed_ability_runtime(game: &mut Game, hero: u32) {
    let hero_pos = {
        let hero = game.state.entities.get(hero).expect("hero should exist");
        (hero.pos_x, hero.pos_y)
    };
    game.state
        .ability_runtime
        .spawn_world_object(AbilityWorldObjectSpec {
            owner: 1,
            caster_id: hero,
            ability: ability::AbilityKind::EkatTeleport,
            kind: AbilityWorldObjectKind::ReturnMarker,
            x: hero_pos.0 + config::TILE_SIZE as f32,
            y: hero_pos.1,
            created_tick: game.tick_count(),
            expires_tick: game
                .tick_count()
                .saturating_add(config::EKAT_RETURN_MARKER_DURATION_TICKS),
            payload: AbilityObjectPayload::DashReturn {
                earliest_return_tick: game
                    .tick_count()
                    .saturating_add(config::EKAT_RETURN_MIN_DELAY_TICKS),
            },
        })
        .expect("return marker should spawn");
    game.state
        .ability_runtime
        .spawn_world_object(AbilityWorldObjectSpec {
            owner: 1,
            caster_id: hero,
            ability: ability::AbilityKind::EkatMagicAnchor,
            kind: AbilityWorldObjectKind::MagicAnchor,
            x: hero_pos.0,
            y: hero_pos.1 + config::TILE_SIZE as f32,
            created_tick: game.tick_count(),
            expires_tick: game
                .tick_count()
                .saturating_add(config::EKAT_MAGIC_ANCHOR_DURATION_TICKS),
            payload: AbilityObjectPayload::MagicAnchor {
                radius: config::EKAT_MAGIC_ANCHOR_RADIUS_TILES * config::TILE_SIZE as f32,
            },
        })
        .expect("magic anchor should spawn");
    game.state
        .ability_runtime
        .spawn_projectile(AbilityProjectileSpec {
            owner: 1,
            caster_id: hero,
            source_object_id: None,
            ability: ability::AbilityKind::EkatLineShot,
            origin: hero_pos,
            endpoint: (hero_pos.0 + config::TILE_SIZE as f32 * 5.0, hero_pos.1),
            return_target: AbilityProjectileReturnTarget::Entity { id: hero },
            speed_px_per_tick: config::EKAT_LINE_SHOT_SPEED_PX_PER_TICK,
            width_px: config::EKAT_LINE_SHOT_WIDTH_TILES * config::TILE_SIZE as f32 * 0.5,
            damage: 0,
            created_tick: game.tick_count(),
            expires_tick: game.tick_count().saturating_add(config::TICK_HZ * 8),
        })
        .expect("line projectile should spawn");
    let lockout_until = game.tick_count().saturating_add(9);
    let hero = game
        .state
        .entities
        .get_mut(hero)
        .expect("hero should exist");
    hero.start_ability_cooldown(ability::AbilityKind::EkatTeleport, 17);
    hero.start_ability_lockout_until(ability::AbilityKind::EkatMagicAnchor, lockout_until);
}

fn seed_combat_state(
    game: &mut Game,
    tank: u32,
    at_gun: u32,
    artillery: u32,
    mortar: u32,
    target: u32,
) {
    let reaction_tick = game.tick_count();
    {
        let tank = game
            .state
            .entities
            .get_mut(tank)
            .expect("tank should exist");
        tank.set_order(Order::attack(target));
        tank.set_target_id(Some(target));
        tank.set_attack_cd(5);
        tank.set_weapon_facing(0.4);
        tank.set_desired_weapon_facing(0.7);
        tank.start_weapon_firing_reveal_response_delay(WeaponKind::TankCannon, target, 3);
        tank.lock_tank_armor_reaction_source((640.0, 384.0), reaction_tick);
        let combat = tank.combat.as_mut().expect("tank should have combat");
        combat.attack_move_no_target_ticks = 2;
        combat.tank_stationary_range_ticks = 4;
    }
    {
        let at_gun = game
            .state
            .entities
            .get_mut(at_gun)
            .expect("anti-tank gun should exist");
        at_gun.set_weapon_setup(WeaponSetup::TearingDownToRedeploy { ticks: 3 });
        at_gun.set_pending_redeploy_facing(Some(1.1));
        at_gun.set_weapon_facing(0.2);
        at_gun.set_desired_weapon_facing(1.1);
    }
    {
        let artillery = game
            .state
            .entities
            .get_mut(artillery)
            .expect("artillery should exist");
        artillery.set_weapon_setup(WeaponSetup::Deployed);
        artillery.set_order(Order::artillery_point_fire(640.0, 384.0));
        artillery.increment_artillery_shots_fired();
        artillery.increment_artillery_shots_fired();
    }
    {
        let mortar = game
            .state
            .entities
            .get_mut(mortar)
            .expect("mortar should exist");
        mortar.set_autocast_enabled(ability::AbilityKind::MortarFire, false);
        mortar.start_ability_cooldown(ability::AbilityKind::MortarFire, 11);
    }
    let scout_id = game
        .state
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::ScoutCar)
        .map(|entity| entity.id);
    if let Some(scout) = scout_id {
        let scout = game
            .state
            .entities
            .get_mut(scout)
            .expect("scout should exist");
        assert!(scout.consume_ability_use(ability::AbilityKind::Smoke));
        scout.start_ability_cooldown(ability::AbilityKind::Smoke, 0);
    }
}

fn assert_owner_payload_privacy(game: &Game, label: &str) {
    let owner_snapshot = game.snapshot_for(1);
    assert!(
        owner_snapshot
            .ability_objects
            .iter()
            .any(|object| object.owner == 1 && object.owner_state.is_some()),
        "{label}: owner should receive owner-only ability object payload"
    );
    let enemy_snapshot = game.snapshot_for(2);
    assert!(
        enemy_snapshot
            .ability_objects
            .iter()
            .any(|object| object.owner == 1 && object.owner_state.is_none()),
        "{label}: enemy should receive public ability object fields without owner-only payload"
    );
}

fn spawn_runtime_return_marker_for_id_check(game: &mut Game, hero: u32) -> u32 {
    let hero_pos = {
        let hero = game.state.entities.get(hero).expect("hero should exist");
        (hero.pos_x, hero.pos_y)
    };
    game.state
        .ability_runtime
        .spawn_world_object(AbilityWorldObjectSpec {
            owner: 1,
            caster_id: hero,
            ability: ability::AbilityKind::EkatTeleport,
            kind: AbilityWorldObjectKind::ReturnMarker,
            x: hero_pos.0,
            y: hero_pos.1,
            created_tick: game.tick_count(),
            expires_tick: game
                .tick_count()
                .saturating_add(config::EKAT_RETURN_MARKER_DURATION_TICKS),
            payload: AbilityObjectPayload::DashReturn {
                earliest_return_tick: game
                    .tick_count()
                    .saturating_add(config::EKAT_RETURN_MIN_DELAY_TICKS),
            },
        })
        .expect("runtime id-check return marker should spawn")
}

fn event_map_for(game: &Game) -> HashMap<u32, Vec<Event>> {
    player_ids(game)
        .into_iter()
        .map(|player| (player, Vec::new()))
        .collect()
}

fn events_for(events: &[(u32, Vec<Event>)], player: u32) -> &[Event] {
    events
        .iter()
        .find_map(|(event_player, events)| (*event_player == player).then_some(events.as_slice()))
        .unwrap_or(&[])
}

fn memory_contains(game: &Game, player: u32, building: u32) -> bool {
    game.state
        .building_memory
        .entries_for_player_for_test(player)
        .any(|entry| entry.id == building)
}

fn snapshot_entity(snapshot: &Snapshot, entity_id: u32) -> Option<&EntityView> {
    snapshot
        .entities
        .iter()
        .find(|entity| entity.id == entity_id)
}

fn spawn_unit_at_tile(game: &mut Game, owner: u32, kind: EntityKind, tx: u32, ty: u32) -> u32 {
    let pos = game.state.map.tile_center(tx, ty);
    game.state
        .entities
        .spawn_unit(owner, kind, pos.0, pos.1)
        .unwrap_or_else(|| panic!("{kind:?} should spawn"))
}

fn phase6_players() -> [PlayerInit; 4] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "ekat".to_string(),
            name: "Alpha".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Bravo".into(),
            color: "#000".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Charlie".into(),
            color: "#0f0".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 4,
            team_id: 4,
            faction_id: "kriegsia".to_string(),
            name: "Delta".into(),
            color: "#00f".into(),
            is_ai: false,
        },
    ]
}

fn flat_lab_map() -> Map {
    const SIZE: u32 = 64;
    Map {
        size: SIZE,
        terrain: vec![terrain::GRASS; (SIZE * SIZE) as usize],
        starts: vec![(16, 16), (48, 48)],
        base_sites: Vec::new(),
    }
}

fn lab_metadata() -> MapMetadata {
    MapMetadata {
        name: "Checkpoint Phase 6 Lab".to_string(),
        schema_version: crate::game::map::CURRENT_MAP_VERSION,
        content_hash: "checkpoint-phase-6-lab".to_string(),
    }
}
