use std::collections::HashMap;

use super::scoring::entity_score_value;
use super::*;
use crate::game::command::SimCommand as Command;
use crate::game::entity::{Entity, EntityKind, GatherPhase, Order, WeaponSetup};
use crate::protocol::{kinds, terrain, AbilityCooldownView, EntityView, Event, OrderPlanMarker};
use crate::rules::{combat, terrain::TerrainKind};

fn human_vs_ai_players() -> [PlayerInit; 2] {
    [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Human".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Computer".into(),
            color: "#000".into(),
            is_ai: true,
        },
    ]
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
        crate::protocol::kind_to_wire(e.kind),
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
    if e.kind == EntityKind::AntiTankGun {
        v.setup_state = Some(e.weapon_setup().to_protocol_str().to_string());
        if e.owner == viewer {
            v.setup_facing = e.emplacement_facing();
        }
    }
    if e.is_building() && !e.prod_queue().is_empty() {
        if let Some(front) = e.prod_queue().first() {
            v.prod_kind = Some(crate::protocol::kind_to_wire(front.unit).to_string());
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
    if e.owner == viewer {
        for kind in [ability::AbilityKind::Charge, ability::AbilityKind::Smoke] {
            if ability::carried_by(kind, e.kind) {
                v.abilities.push(AbilityCooldownView {
                    ability: kind.to_protocol_str().to_string(),
                    cooldown_left: e.ability_cooldown_ticks(kind),
                    remaining_uses: e.ability_uses_remaining(kind),
                    autocast_enabled: e.autocast_enabled(kind),
                    active_object_id: None,
                    available_tick: None,
                    lockout_until_tick: None,
                    expires_in: None,
                });
            }
        }
        if let Order::Attack(order) = e.order() {
            if let Some(target) = game.entities.get(order.intent.target) {
                if target_visible {
                    v.order_plan.push(OrderPlanMarker {
                        kind: "attack".to_string(),
                        x: target.pos_x,
                        y: target.pos_y,
                    });
                }
            }
        }
    }
    v
}

fn flat_tank_move_fixture() -> (Game, u32, (f32, f32)) {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
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

fn empty_flat_game(players: &[PlayerInit]) -> Game {
    let mut game = Game::new_for_replay(players, 0x1234_5678);
    for tile in &mut game.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }
    game.smokes = SmokeCloudStore::new();
    game.mortar_shells = MortarShellStore::default();
    game.artillery_shells = artillery::ArtilleryShellStore::default();
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    game
}

#[test]
fn replay_keyframe_clone_preserves_ability_runtime_state() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = empty_flat_game(&players);
    let caster = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, 128.0, 128.0)
        .expect("caster should spawn");
    let object_id = game
        .ability_runtime
        .spawn_world_object(ability_runtime::AbilityWorldObjectSpec {
            owner: 1,
            caster_id: caster,
            ability: ability::AbilityKind::EkatTeleport,
            kind: ability_runtime::AbilityWorldObjectKind::ReturnMarker,
            x: 128.0,
            y: 128.0,
            created_tick: 0,
            expires_tick: 30,
            payload: ability_runtime::AbilityObjectPayload::DashReturn {
                earliest_return_tick: 1,
            },
        })
        .expect("ability object should spawn");

    let clone = game.clone_for_replay_keyframe();

    assert_eq!(
        clone
            .ability_runtime
            .world_objects()
            .map(|object| object.id.get())
            .collect::<Vec<_>>(),
        vec![object_id]
    );
}

#[test]
fn game_tick_cleans_up_expired_ability_runtime_state() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = empty_flat_game(&players);
    let caster = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, 128.0, 128.0)
        .expect("caster should spawn");
    game.ability_runtime
        .spawn_world_object(ability_runtime::AbilityWorldObjectSpec {
            owner: 1,
            caster_id: caster,
            ability: ability::AbilityKind::EkatTeleport,
            kind: ability_runtime::AbilityWorldObjectKind::ReturnMarker,
            x: 128.0,
            y: 128.0,
            created_tick: 0,
            expires_tick: 1,
            payload: ability_runtime::AbilityObjectPayload::None,
        })
        .expect("ability object should spawn");

    game.tick();

    assert_eq!(game.ability_runtime.world_objects().count(), 0);
}

#[test]
fn snapshot_projects_abilities_from_owner_faction_catalog() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::EMPTY_FIXTURE_FACTION_ID.to_string(),
        name: "Fixture".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let scout = game
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, pos.0, pos.1)
        .expect("scout should spawn");
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    game.fog.recompute(&[1], &game.entities, &game.map);

    let snapshot = game.snapshot_for(1);
    let scout_view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == scout)
        .expect("scout should project");

    assert!(
        scout_view.abilities.is_empty(),
        "fixture faction scout cars should not inherit Kriegsia Smoke affordances"
    );
}

#[test]
fn ekat_start_projects_hero_zamok_and_abilities() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::EKAT_FACTION_ID.to_string(),
        name: "Ekat".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let game = Game::new_for_replay(&players, 0x1234_5678);

    assert_eq!(game.players[0].steel, 0);
    assert_eq!(game.players[0].oil, 0);
    assert!(game
        .entities
        .iter()
        .any(|entity| entity.owner == 1 && entity.kind == EntityKind::Zamok));
    let hero = game
        .entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Ekat)
        .expect("Ekat should start with her hero");
    assert_eq!(hero.hp, 300);

    let snapshot = game.snapshot_for(1);
    let hero_view = snapshot
        .entities
        .iter()
        .find(|entity| entity.id == hero.id)
        .expect("hero should project");
    let ability_ids: Vec<_> = hero_view
        .abilities
        .iter()
        .map(|ability| ability.ability.as_str())
        .collect();
    assert_eq!(
        ability_ids,
        vec![
            crate::protocol::abilities::EKAT_TELEPORT,
            crate::protocol::abilities::EKAT_LINE_SHOT,
        ]
    );
}

#[test]
fn ekat_regenerates_one_hp_per_second_while_alive() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::EKAT_FACTION_ID.to_string(),
        name: "Ekat".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    game.entities
        .get_mut(hero)
        .expect("hero exists")
        .apply_damage(50, None);

    for _ in 0..config::TICK_HZ {
        game.tick();
    }

    assert_eq!(game.entities.get(hero).expect("hero exists").hp, 251);
}

fn ekat_player() -> PlayerInit {
    PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::EKAT_FACTION_ID.to_string(),
        name: "Ekat".into(),
        color: "#fff".into(),
        is_ai: false,
    }
}

fn kriegsia_enemy() -> PlayerInit {
    PlayerInit {
        id: 2,
        team_id: 2,
        faction_id: crate::rules::faction::DEFAULT_FACTION_ID.to_string(),
        name: "Enemy".into(),
        color: "#000".into(),
        is_ai: false,
    }
}

fn enqueue_ekat_dash(game: &mut Game, hero: u32, target: (f32, f32)) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::EkatTeleport,
            units: vec![hero],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
}

fn enqueue_ekat_return(game: &mut Game, hero: u32, target_object_id: Option<u32>) {
    game.enqueue(
        1,
        Command::RecastAbility {
            ability: ability::AbilityKind::EkatTeleport,
            units: vec![hero],
            target_object_id,
            queued: false,
        },
    );
}

fn enqueue_ekat_line_shot(game: &mut Game, hero: u32, target: (f32, f32)) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::EkatLineShot,
            units: vec![hero],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
}

fn active_return_marker_id(game: &Game, hero: u32) -> Option<u32> {
    game.ability_runtime
        .active_return_marker(
            1,
            hero,
            ability::AbilityKind::EkatTeleport,
            None,
            game.current_tick(),
        )
        .map(|marker| marker.id.get())
}

#[test]
fn ekat_dash_moves_up_to_five_tiles_leaves_return_marker_and_starts_cooldown() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: crate::rules::faction::EKAT_FACTION_ID.to_string(),
        name: "Ekat".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();

    let hero_entity = game.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - target.0).abs() < f32::EPSILON);
    assert!((hero_entity.pos_y - target.1).abs() < f32::EPSILON);
    assert_eq!(
        hero_entity.ability_cooldown_ticks(ability::AbilityKind::EkatTeleport),
        config::EKAT_TELEPORT_COOLDOWN_TICKS.saturating_sub(1)
    );
    let marker = game
        .ability_runtime
        .active_return_marker(1, hero, ability::AbilityKind::EkatTeleport, None, 1)
        .expect("dash should leave a return marker");
    assert!((marker.x - pos.0).abs() < f32::EPSILON);
    assert!((marker.y - pos.1).abs() < f32::EPSILON);
    assert_eq!(
        marker.expires_in(game.current_tick()),
        Some(config::EKAT_RETURN_MARKER_DURATION_TICKS as u16)
    );
}

#[test]
fn ekat_dash_rejects_invalid_landing_without_marker_or_cooldown() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = game.map.tile_center(15, 10);
    let blocked_index = game.map.index(15, 10);
    game.map.terrain[blocked_index] = terrain::ROCK;
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();

    let hero_entity = game.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - pos.0).abs() < f32::EPSILON);
    assert_eq!(
        hero_entity.ability_cooldown_ticks(ability::AbilityKind::EkatTeleport),
        0
    );
    assert!(active_return_marker_id(&game, hero).is_none());
}

#[test]
fn ekat_return_cannot_happen_in_same_tick_as_dash() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    enqueue_ekat_return(&mut game, hero, None);
    game.tick();

    let hero_entity = game.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - target.0).abs() < f32::EPSILON);
    assert!(active_return_marker_id(&game, hero).is_some());
}

#[test]
fn ekat_return_recasts_to_marker_and_consumes_it_after_delay() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();
    let marker_id = active_return_marker_id(&game, hero).expect("return marker exists");
    enqueue_ekat_return(&mut game, hero, Some(marker_id));
    game.tick();

    let hero_entity = game.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - pos.0).abs() < f32::EPSILON);
    assert!((hero_entity.pos_y - pos.1).abs() < f32::EPSILON);
    assert!(active_return_marker_id(&game, hero).is_none());
}

#[test]
fn ekat_return_fails_after_marker_expires() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();
    for _ in 0..config::EKAT_RETURN_MARKER_DURATION_TICKS {
        game.tick();
    }
    enqueue_ekat_return(&mut game, hero, None);
    game.tick();

    let hero_entity = game.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - target.0).abs() < f32::EPSILON);
    assert!(active_return_marker_id(&game, hero).is_none());
}

#[test]
fn ekat_return_fails_when_marker_destination_is_blocked() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();
    let marker_id = active_return_marker_id(&game, hero).expect("return marker exists");
    let blocked_index = game.map.index(10, 10);
    game.map.terrain[blocked_index] = terrain::ROCK;
    enqueue_ekat_return(&mut game, hero, Some(marker_id));
    game.tick();

    let hero_entity = game.entities.get(hero).expect("hero exists");
    assert!((hero_entity.pos_x - target.0).abs() < f32::EPSILON);
    assert_eq!(active_return_marker_id(&game, hero), Some(marker_id));
}

#[test]
fn ekat_return_with_stale_caster_is_panic_free() {
    let players = [ekat_player()];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();
    game.entities.remove(hero);
    enqueue_ekat_return(&mut game, hero, None);
    game.tick();

    assert!(game.entities.get(hero).is_none());
}

#[test]
fn ekat_dash_return_marker_projection_respects_fog() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 5.0, pos.1);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    game.entities
        .spawn_unit(2, EntityKind::Worker, target.0 + 500.0, target.1 + 500.0)
        .expect("enemy should spawn");

    enqueue_ekat_dash(&mut game, hero, target);
    game.tick();
    let marker_id = active_return_marker_id(&game, hero).expect("return marker exists");

    assert!(game
        .snapshot_for(1)
        .ability_objects
        .iter()
        .any(|object| object.id == marker_id));
    assert!(!game
        .snapshot_for(2)
        .ability_objects
        .iter()
        .any(|object| object.id == marker_id));
}

#[test]
fn ekat_line_shot_spawns_moving_projectile_and_starts_cooldown_without_immediate_damage() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 6.0, pos.1);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    let enemy = game
        .entities
        .spawn_unit(
            2,
            EntityKind::Rifleman,
            pos.0 + config::TILE_SIZE as f32 * 3.0,
            pos.1,
        )
        .expect("enemy should spawn");
    let ally = game
        .entities
        .spawn_unit(
            1,
            EntityKind::Rifleman,
            pos.0 + config::TILE_SIZE as f32 * 4.0,
            pos.1,
        )
        .expect("ally should spawn");

    enqueue_ekat_line_shot(&mut game, hero, target);
    game.tick();

    let projectile = game
        .ability_runtime
        .world_objects()
        .find(|object| object.ability == ability::AbilityKind::EkatLineShot)
        .expect("line shot should spawn a projected ability object");
    assert_eq!(
        projectile.kind,
        ability_runtime::AbilityWorldObjectKind::LineProjectile
    );
    assert!(
        projectile.x > pos.0 && projectile.x < target.0,
        "projectile should move out from Ekat instead of applying instant full-line damage"
    );
    assert_eq!(
        game.entities.get(enemy).expect("enemy exists").hp,
        game.entities.get(enemy).expect("enemy exists").max_hp
    );
    assert_eq!(game.entities.get(ally).expect("ally exists").hp, 45);
    assert_eq!(
        game.entities
            .get(hero)
            .expect("hero exists")
            .ability_cooldown_ticks(ability::AbilityKind::EkatLineShot),
        config::EKAT_LINE_SHOT_COOLDOWN_TICKS.saturating_sub(1)
    );
}

#[test]
fn ekat_line_shot_hits_enemies_on_outbound_and_return_legs() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 6.0, pos.1);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    let enemy = game
        .entities
        .spawn_building(
            2,
            EntityKind::Depot,
            pos.0 + config::TILE_SIZE as f32 * 5.0,
            pos.1,
            true,
        )
        .expect("enemy should spawn");
    let enemy_max_hp = game.entities.get(enemy).expect("enemy exists").max_hp;

    enqueue_ekat_line_shot(&mut game, hero, target);
    for _ in 0..23 {
        game.tick();
    }
    let after_outbound = game.entities.get(enemy).expect("enemy exists").hp;
    assert!(
        after_outbound <= enemy_max_hp.saturating_sub(config::EKAT_LINE_SHOT_DAMAGE),
        "outbound leg should damage the enemy"
    );
    for _ in 0..25 {
        game.tick();
    }
    assert!(
        game.entities.get(enemy).expect("enemy exists").hp
            <= after_outbound.saturating_sub(config::EKAT_LINE_SHOT_DAMAGE),
        "return leg should damage the enemy again"
    );
}

#[test]
fn ekat_line_shot_endpoint_clamps_to_range() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let far_target = (pos.0 + config::TILE_SIZE as f32 * 20.0, pos.1);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");
    let inside_range_enemy = game
        .entities
        .spawn_building(
            2,
            EntityKind::Depot,
            pos.0 + config::TILE_SIZE as f32 * 5.5,
            pos.1,
            true,
        )
        .expect("inside range enemy should spawn");
    let beyond_range_enemy = game
        .entities
        .spawn_building(
            2,
            EntityKind::Depot,
            pos.0 + config::TILE_SIZE as f32 * 8.0,
            pos.1,
            true,
        )
        .expect("beyond range enemy should spawn");
    let inside_max_hp = game
        .entities
        .get(inside_range_enemy)
        .expect("inside range enemy exists")
        .max_hp;
    let beyond_max_hp = game
        .entities
        .get(beyond_range_enemy)
        .expect("beyond range enemy exists")
        .max_hp;

    enqueue_ekat_line_shot(&mut game, hero, far_target);
    for _ in 0..23 {
        game.tick();
    }

    assert!(
        game.entities
            .get(inside_range_enemy)
            .expect("inside range enemy exists")
            .hp
            <= inside_max_hp.saturating_sub(config::EKAT_LINE_SHOT_DAMAGE),
        "clamped endpoint should allow targets inside range to be hit"
    );
    assert_eq!(
        game.entities
            .get(beyond_range_enemy)
            .expect("beyond range enemy exists")
            .hp,
        beyond_max_hp
    );
}

#[test]
fn ekat_line_shot_return_tracks_ekats_current_position_after_dash() {
    let players = [ekat_player(), kriegsia_enemy()];
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = (pos.0 + config::TILE_SIZE as f32 * 6.0, pos.1);
    let dash_target = (pos.0, pos.1 + config::TILE_SIZE as f32 * 5.0);
    let hero = game
        .entities
        .spawn_unit(1, EntityKind::Ekat, pos.0, pos.1)
        .expect("hero should spawn");

    enqueue_ekat_line_shot(&mut game, hero, target);
    game.tick();
    enqueue_ekat_dash(&mut game, hero, dash_target);
    game.tick();
    for _ in 0..24 {
        game.tick();
    }

    let projectile = game
        .ability_runtime
        .world_objects()
        .find(|object| object.ability == ability::AbilityKind::EkatLineShot)
        .expect("line shot should still be returning");
    assert!(
        projectile.y > pos.1,
        "returning projectile should bend toward Ekat's dashed position"
    );
}

#[test]
fn artillery_point_fire_queue_is_terminal() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let pos = game.map.tile_center(10, 10);
    let target = game.map.tile_center(22, 10);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    deploy_artillery_toward(&mut game, artillery, target);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
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
    assert!(matches!(entity.order(), Order::ArtilleryPointFire(_)));
    assert!(
        entity.queued_orders().is_empty(),
        "later queued move should not be accepted behind terminal Point Fire"
    );
}

#[test]
fn artillery_target_is_owner_only_and_enemy_events_require_current_vision() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.players[0].steel;
    let pos = game.map.tile_center(10, 10);
    let target = game.map.tile_center(22, 10);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    game.entities
        .spawn_unit(
            2,
            EntityKind::Worker,
            pos.0 + config::TILE_SIZE as f32,
            pos.1,
        )
        .expect("enemy gun spotter should spawn");
    game.entities
        .spawn_unit(2, EntityKind::Worker, target.0, target.1)
        .expect("enemy impact spotter should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    deploy_artillery_toward(&mut game, artillery, target);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );

    let mut owner_saw_target = false;
    let mut enemy_saw_target = false;
    let mut enemy_saw_artillery_reveal = false;
    let mut owner_saw_impact = false;
    let mut enemy_saw_impact = false;
    for _ in 0..(config::ARTILLERY_SETUP_TICKS as u32 + config::ARTILLERY_SHELL_DELAY_TICKS + 8) {
        for (pid, events) in game.tick() {
            for event in events {
                match event {
                    Event::ArtilleryTarget { .. } if pid == 1 => owner_saw_target = true,
                    Event::ArtilleryTarget { .. } if pid == 2 => enemy_saw_target = true,
                    Event::Attack {
                        from,
                        reveal: Some(reveal),
                        ..
                    } if pid == 2 && from == artillery && reveal.kind == kinds::ARTILLERY => {
                        enemy_saw_artillery_reveal = true
                    }
                    Event::ArtilleryImpact { .. } if pid == 1 => owner_saw_impact = true,
                    Event::ArtilleryImpact { .. } if pid == 2 => enemy_saw_impact = true,
                    _ => {}
                }
            }
        }
    }

    assert!(
        owner_saw_target,
        "firing player should see pre-impact target marker"
    );
    assert!(
        !enemy_saw_target,
        "enemy should never receive pre-impact artillery target marker"
    );
    assert!(enemy_saw_artillery_reveal);
    assert!(owner_saw_impact, "owner should see delayed impact");
    assert!(
        enemy_saw_impact,
        "enemy should see delayed impact only with current vision at the impact"
    );
    assert!(
        game.players[0].steel <= initial_steel - config::ARTILLERY_AMMO_COST_STEEL,
        "at least one fired shell should spend steel at fire time"
    );
}

#[test]
fn packed_artillery_point_fire_does_not_auto_setup_or_fire() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.players[0].steel;
    let pos = game.map.tile_center(10, 10);
    let target = game.map.tile_center(22, 10);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(target.0),
            y: Some(target.1),
            queued: false,
        },
    );
    let events = game.tick();

    let entity = game.entities.get(artillery).expect("artillery exists");
    assert!(matches!(entity.weapon_setup(), WeaponSetup::Packed));
    assert!(!matches!(entity.order(), Order::ArtilleryPointFire(_)));
    assert_eq!(game.players[0].steel, initial_steel);
    assert!(
        events
            .iter()
            .flat_map(|(_, events)| events)
            .all(|event| !matches!(event, Event::ArtilleryTarget { .. })),
        "packed point fire should not emit a target marker"
    );
}

#[test]
fn manually_deployed_artillery_can_point_fire() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.players[0].steel;
    let pos = game.map.tile_center(10, 10);
    let setup_target = game.map.tile_center(18, 10);
    let fire_target = game.map.tile_center(22, 10);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");

    game.enqueue(
        1,
        Command::SetupAntiTankGuns {
            units: vec![artillery],
            x: setup_target.0,
            y: setup_target.1,
            queued: false,
        },
    );
    for _ in 0..=config::ARTILLERY_SETUP_TICKS {
        game.tick();
    }
    assert!(matches!(
        game.entities
            .get(artillery)
            .expect("artillery exists")
            .weapon_setup(),
        WeaponSetup::Deployed
    ));

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(fire_target.0),
            y: Some(fire_target.1),
            queued: false,
        },
    );
    let events = game.tick();

    assert_eq!(
        game.players[0].steel,
        initial_steel - config::ARTILLERY_AMMO_COST_STEEL
    );
    assert!(
        events.iter().any(|(pid, events)| {
            *pid == 1
                && events
                    .iter()
                    .any(|event| matches!(event, Event::ArtilleryTarget { from, .. } if *from == artillery))
        }),
        "manual setup should allow artillery point fire and identify the firing gun"
    );
}

#[test]
fn artillery_point_fire_inside_minimum_range_does_not_spend_steel() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let initial_steel = game.players[0].steel;
    let pos = game.map.tile_center(10, 10);
    let min_px = config::ARTILLERY_MIN_RANGE_TILES as f32 * config::TILE_SIZE as f32;
    let too_close = (pos.0 + min_px - 8.0, pos.1);
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, pos.0, pos.1)
        .expect("artillery should spawn");
    deploy_artillery_toward(&mut game, artillery, too_close);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(too_close.0),
            y: Some(too_close.1),
            queued: false,
        },
    );
    let events = game.tick();

    assert_eq!(game.players[0].steel, initial_steel);
    assert!(
        events
            .iter()
            .flat_map(|(_, events)| events)
            .all(|event| !matches!(event, Event::ArtilleryTarget { .. })),
        "minimum-range rejection should not fire or create a target marker"
    );
}

#[test]
fn artillery_shell_inside_building_footprint_deals_full_inner_ap_damage() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let depot = game
        .entities
        .spawn_building(2, EntityKind::Depot, 160.0, 160.0, true)
        .expect("depot should spawn");
    let before = game.entities.get(depot).expect("depot exists").hp;

    resolve_test_artillery_shell(&mut game, 160.0, 160.0);

    let after = game.entities.get(depot).expect("depot survives").hp;
    let expected = combat::effective_damage(
        EntityKind::Artillery,
        EntityKind::Depot,
        config::ARTILLERY_INNER_DAMAGE,
        Some(TerrainKind::Open),
    );
    assert_eq!(before - after, expected);
}

#[test]
fn artillery_shell_outside_building_uses_footprint_distance_falloff() {
    let players = human_vs_ai_players();
    let mut game = empty_flat_game(&players);
    let depot = game
        .entities
        .spawn_building(2, EntityKind::Depot, 160.0, 160.0, true)
        .expect("depot should spawn");
    let stats = config::building_stats(EntityKind::Depot).expect("depot stats");
    let ts = config::TILE_SIZE as f32;
    let half_w = stats.foot_w as f32 * ts * 0.5;
    let inner = config::ARTILLERY_INNER_RADIUS_TILES * ts;
    let outer = config::ARTILLERY_OUTER_RADIUS_TILES * ts;
    let gap = inner + (outer - inner) * 0.5;
    let before = game.entities.get(depot).expect("depot exists").hp;

    resolve_test_artillery_shell(&mut game, 160.0 + half_w + gap, 160.0);

    let after = game.entities.get(depot).expect("depot survives").hp;
    let expected = {
        let t = ((gap - inner) / (outer - inner)).clamp(0.0, 1.0);
        let base = (config::ARTILLERY_INNER_DAMAGE as f32
            + (config::ARTILLERY_OUTER_MIN_DAMAGE as f32 - config::ARTILLERY_INNER_DAMAGE as f32)
                * t)
            .round() as u32;
        combat::effective_damage(
            EntityKind::Rifleman,
            EntityKind::Depot,
            base,
            Some(TerrainKind::Open),
        )
    };
    assert_eq!(before - after, expected);
}

#[test]
fn artillery_shell_damages_allied_entities_without_last_damage_attribution() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#aaa".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let depot = game
        .entities
        .spawn_building(2, EntityKind::Depot, 160.0, 160.0, true)
        .expect("allied depot should spawn");
    let before = game.entities.get(depot).expect("depot exists").hp;

    resolve_test_artillery_shell(&mut game, 160.0, 160.0);

    let depot = game.entities.get(depot).expect("depot survives");
    assert!(
        depot.hp < before,
        "same-team depot should take artillery splash damage"
    );
    assert_eq!(depot.last_damage_owner(), None);
    assert_eq!(depot.last_damage_pos(), None);
    assert_eq!(depot.last_damage_tick(), None);
}

fn resolve_test_artillery_shell(game: &mut Game, x: f32, y: f32) {
    let mut events = HashMap::new();
    events.insert(1, Vec::new());
    let teams = teams::TeamRelations::from_player_teams(
        game.players
            .iter()
            .map(|player| (player.id, player.team_id)),
    );
    game.artillery_shells.schedule(1, 1, x, y, game.tick);
    game.artillery_shells.resolve_due(
        &mut game.entities,
        &teams,
        &game.fog,
        &mut events,
        game.tick + config::ARTILLERY_SHELL_DELAY_TICKS,
    );
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

fn smoke_projection_fixture() -> (Game, u32, u32, u32, (f32, f32)) {
    let players = [
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
    ];
    let mut game = Game::new_for_replay(&players, 0x5EED_5000);
    for tile in &mut game.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }
    let observer_pos = game.map.tile_center(4, 4);
    let smoke_pos = game.map.tile_center(7, 4);
    let friendly_pos = game.map.tile_center(8, 4);
    let observer = game
        .entities
        .spawn_unit(1, EntityKind::Worker, observer_pos.0, observer_pos.1)
        .expect("observer should spawn");
    let friendly = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, friendly_pos.0, friendly_pos.1)
        .expect("friendly should spawn");
    let enemy = game
        .entities
        .spawn_unit(2, EntityKind::Rifleman, smoke_pos.0, smoke_pos.1)
        .expect("enemy should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    game.smokes
        .spawn(
            smoke_pos.0,
            smoke_pos.1,
            config::SMOKE_CLOUD_RADIUS_TILES,
            config::SMOKE_CLOUD_DURATION_TICKS,
            game.tick,
        )
        .expect("smoke should spawn");
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog
        .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);
    (game, observer, friendly, enemy, smoke_pos)
}

fn team_fog_fixture() -> (Game, u32, u32, u32, (f32, f32)) {
    let players = [
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
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#bbb".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Three".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let p1_base = game.map.tile_center(2, 2);
    let p2_base = game.map.tile_center(5, 2);
    let p3_base = game.map.tile_center(55, 55);
    game.entities
        .spawn_building(1, EntityKind::CityCentre, p1_base.0, p1_base.1, true)
        .expect("p1 city centre should spawn");
    game.entities
        .spawn_building(2, EntityKind::CityCentre, p2_base.0, p2_base.1, true)
        .expect("p2 city centre should spawn");
    game.entities
        .spawn_building(3, EntityKind::CityCentre, p3_base.0, p3_base.1, true)
        .expect("p3 city centre should spawn");

    let spotter_pos = game.map.tile_center(28, 30);
    let enemy_pos = game.map.tile_center(30, 30);
    let hidden_enemy_pos = game.map.tile_center(55, 50);
    let spotter = game
        .entities
        .spawn_unit(2, EntityKind::Worker, spotter_pos.0, spotter_pos.1)
        .expect("ally spotter should spawn");
    let visible_enemy = game
        .entities
        .spawn_unit(3, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("visible enemy should spawn");
    let hidden_enemy = game
        .entities
        .spawn_unit(
            3,
            EntityKind::Rifleman,
            hidden_enemy_pos.0,
            hidden_enemy_pos.1,
        )
        .expect("hidden enemy should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog
        .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);
    (game, spotter, visible_enemy, hidden_enemy, enemy_pos)
}

#[test]
fn snapshot_shares_living_teammate_current_vision() {
    let (game, _spotter, visible_enemy, hidden_enemy, enemy_pos) = team_fog_fixture();
    assert!(
        !game.fog.is_visible_world(1, enemy_pos.0, enemy_pos.1),
        "fixture should keep the enemy outside player 1's own raw fog"
    );
    assert!(
        game.fog.is_visible_world(2, enemy_pos.0, enemy_pos.1),
        "fixture should put the enemy inside player 2's raw fog"
    );

    let snapshot = game.snapshot_for(1);

    assert!(
        snapshot
            .entities
            .iter()
            .any(|entity| entity.id == visible_enemy),
        "ally current sight should reveal the enemy in player 1's snapshot"
    );
    assert!(
        snapshot
            .entities
            .iter()
            .all(|entity| entity.id != hidden_enemy),
        "enemies outside every living teammate's current sight should stay hidden"
    );
    let visible_index = ((enemy_pos.1 / config::TILE_SIZE as f32).floor() as u32 * game.map.size
        + (enemy_pos.0 / config::TILE_SIZE as f32).floor() as u32) as usize;
    assert_eq!(
        snapshot.visible_tiles[visible_index], 1,
        "visibleTiles should include the living teammate's current sight"
    );
    assert_eq!(snapshot.player_resources.len(), 0);
    assert!(
        snapshot.steel
            == game
                .players
                .iter()
                .find(|player| player.id == 1)
                .expect("player 1 should exist")
                .steel,
        "recipient economy remains local-player-only"
    );
}

#[test]
fn defeated_teammate_no_longer_contributes_current_vision() {
    let (mut game, _spotter, visible_enemy, _hidden_enemy, _enemy_pos) = team_fog_fixture();

    assert!(
        game.snapshot_for(1)
            .entities
            .iter()
            .any(|entity| entity.id == visible_enemy),
        "precondition: teammate sight reveals the enemy"
    );

    game.eliminate(2);

    assert!(
        game.snapshot_for(1)
            .entities
            .iter()
            .all(|entity| entity.id != visible_enemy),
        "eliminated teammate sight should stop contributing to team current vision"
    );
    assert!(
        game.snapshot_for(2)
            .entities
            .iter()
            .all(|entity| entity.id != visible_enemy),
        "defeated player should receive surviving teammate vision but not their own stale vision"
    );
}

#[test]
fn team_current_vision_keeps_smoke_blocking() {
    let players = [
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
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#bbb".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Three".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let p1_base = game.map.tile_center(2, 2);
    let p2_base = game.map.tile_center(4, 2);
    let p3_base = game.map.tile_center(50, 50);
    game.entities
        .spawn_building(1, EntityKind::CityCentre, p1_base.0, p1_base.1, true)
        .expect("p1 city centre should spawn");
    game.entities
        .spawn_building(2, EntityKind::CityCentre, p2_base.0, p2_base.1, true)
        .expect("p2 city centre should spawn");
    game.entities
        .spawn_building(3, EntityKind::CityCentre, p3_base.0, p3_base.1, true)
        .expect("p3 city centre should spawn");
    let spotter_pos = game.map.tile_center(4, 4);
    let smoke_pos = game.map.tile_center(7, 4);
    let enemy_pos = game.map.tile_center(7, 4);
    game.entities
        .spawn_unit(2, EntityKind::Worker, spotter_pos.0, spotter_pos.1)
        .expect("ally spotter should spawn");
    let enemy = game
        .entities
        .spawn_unit(3, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");
    game.smokes
        .spawn(
            smoke_pos.0,
            smoke_pos.1,
            config::SMOKE_CLOUD_RADIUS_TILES,
            config::SMOKE_CLOUD_DURATION_TICKS,
            game.tick,
        )
        .expect("smoke should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog
        .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);

    let snapshot = game.snapshot_for(1);

    assert!(
        snapshot.entities.iter().all(|entity| entity.id != enemy),
        "team current vision must not reveal enemies hidden inside smoke"
    );
}

#[test]
fn manual_mortar_fire_impacts_without_toast_notice() {
    let players = [
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
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.map.tile_center(8, 8);
    let target_pos = game.map.tile_center(12, 8);
    let mortar = game
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    game.entities
        .get_mut(mortar)
        .expect("mortar should exist")
        .set_weapon_setup(WeaponSetup::Deployed);
    let target = game
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    let accepted_events = game.tick();
    let owner_events = accepted_events
        .iter()
        .find(|(player_id, _)| *player_id == 1)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(
        owner_events.iter().any(|event| matches!(
            event,
            Event::MortarLaunch { from, to_x, to_y, delay_ticks, .. }
                if *from == mortar
                    && (*to_x - target_pos.0).abs() < 0.001
                    && (*to_y - target_pos.1).abs() < 0.001
                    && *delay_ticks == config::MORTAR_SHELL_DELAY_TICKS
        )),
        "accepted mortar command should emit a launch marker with impact timing: {owner_events:?}"
    );
    let enemy_events = accepted_events
        .iter()
        .find(|(player_id, _)| *player_id == 2)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(
        enemy_events
            .iter()
            .all(|event| !matches!(event, Event::MortarLaunch { .. })),
        "manual mortar fire should not reveal launch preview markers to enemies: {enemy_events:?}"
    );
    assert!(
        owner_events
            .iter()
            .all(|event| !matches!(event, Event::Notice { msg, .. } if msg == "Mortar fire")),
        "accepted mortar command should use impact feedback instead of a toast notice: {owner_events:?}"
    );
    let hp_before_impact = game
        .entities
        .get(target)
        .expect("target should still exist")
        .hp;

    let mut impact_events = Vec::new();
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        impact_events = game.tick();
    }

    assert!(
        game.entities
            .get(target)
            .is_none_or(|target_after| target_after.hp < hp_before_impact),
        "manual mortar fire should damage or kill units at the targeted impact point"
    );
    let owner_events = impact_events
        .iter()
        .find(|(player_id, _)| *player_id == 1)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(
        owner_events
            .iter()
            .any(|event| matches!(event, Event::MortarImpact { x, y, .. }
                if (*x - target_pos.0).abs() < 0.001 && (*y - target_pos.1).abs() < 0.001)),
        "delayed mortar impact should emit a visible impact marker: {owner_events:?}"
    );
}

#[test]
fn set_autocast_command_enables_mortar_autocast_from_default_off() {
    let players = [
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
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.map.tile_center(8, 8);
    let mortar = game
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");

    assert_eq!(
        game.entities
            .get(mortar)
            .expect("mortar should exist")
            .autocast_enabled(ability::AbilityKind::MortarFire),
        Some(false),
        "mortar autocast should start disabled"
    );
    game.players[0]
        .upgrades
        .insert(upgrade::UpgradeKind::MortarAutocast);

    game.enqueue(
        1,
        Command::SetAutocast {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            enabled: true,
        },
    );
    for _ in 0..10 {
        game.tick();
    }

    assert_eq!(
        game.entities
            .get(mortar)
            .expect("mortar should exist")
            .autocast_enabled(ability::AbilityKind::MortarFire),
        Some(true),
        "setAutocast should enable mortar autofire"
    );
}

#[test]
fn visible_autocast_mortar_launch_is_sent_to_enemy() {
    let players = [
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
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.map.tile_center(8, 8);
    let target_pos = game.map.tile_center(12, 8);
    let mortar = game
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    {
        let mortar_entity = game.entities.get_mut(mortar).expect("mortar should exist");
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
        mortar_entity.set_autocast_enabled(ability::AbilityKind::MortarFire, true);
    }
    game.entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    game.players[0]
        .upgrades
        .insert(upgrade::UpgradeKind::MortarAutocast);
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    assert!(
        game.fog.is_visible_world(2, mortar_pos.0, mortar_pos.1),
        "test setup requires the enemy to see the autocasting mortar"
    );

    let events = game.tick();
    let enemy_events = events
        .iter()
        .find(|(player_id, _)| *player_id == 2)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);

    assert!(
        enemy_events.iter().any(|event| matches!(
            event,
            Event::MortarLaunch { from, delay_ticks, .. }
                if *from == mortar
                    && *delay_ticks == config::MORTAR_SHELL_DELAY_TICKS
        )),
        "visible autocast mortar fire should show enemy launch preview markers: {enemy_events:?}"
    );
}

#[test]
fn hidden_mortar_launch_is_not_sent_but_impact_reveals_attacker_to_victim() {
    let players = [
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
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.map.tile_center(8, 8);
    let target_pos = game.map.tile_center(17, 8);
    let mortar = game
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    game.entities
        .get_mut(mortar)
        .expect("mortar should exist")
        .set_weapon_setup(WeaponSetup::Deployed);
    let target = game
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    assert!(
        !game.fog.is_visible_world(2, mortar_pos.0, mortar_pos.1),
        "test setup requires the mortar to be hidden before it fires"
    );

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    let accepted_events = game.tick();
    let enemy_events = accepted_events
        .iter()
        .find(|(player_id, _)| *player_id == 2)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(
        enemy_events
            .iter()
            .all(|event| !matches!(event, Event::MortarLaunch { .. })),
        "hidden mortar launch should not leak dust/recoil/shell data: {enemy_events:?}"
    );

    let hp_before_impact = game
        .entities
        .get(target)
        .expect("target should still exist")
        .hp;
    let mut impact_events = Vec::new();
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        impact_events = game.tick();
    }
    assert!(
        game.entities
            .get(target)
            .is_none_or(|target_after| target_after.hp < hp_before_impact),
        "test setup requires the mortar impact to damage the victim"
    );
    let enemy_events = impact_events
        .iter()
        .find(|(player_id, _)| *player_id == 2)
        .map(|(_, events)| events.as_slice())
        .unwrap_or(&[]);
    assert!(
        enemy_events.iter().any(|event| matches!(
            event,
            Event::MortarImpact {
                from: Some(from),
                reveal: Some(reveal),
                x,
                y,
                ..
            } if *from == mortar
                && reveal.kind == kinds::MORTAR_TEAM
                && (*x - target_pos.0).abs() < 0.001
                && (*y - target_pos.1).abs() < 0.001
        )),
        "victim should receive a mortar impact reveal after being hit: {enemy_events:?}"
    );
}

#[test]
fn manual_mortar_fire_waits_for_tube_alignment() {
    let players = [
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
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.map.tile_center(8, 8);
    let target_pos = game.map.tile_center(8, 4);
    let mortar = game
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.entities.get_mut(mortar) {
        mortar_entity.set_facing(0.0);
        mortar_entity.set_weapon_facing(0.0);
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
    }
    let target = game
        .entities
        .spawn_unit(2, EntityKind::Rifleman, target_pos.0, target_pos.1)
        .expect("target should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );

    game.tick();
    let mortar_entity = game.entities.get(mortar).expect("mortar should exist");
    assert_eq!(
        mortar_entity.ability_cooldown_ticks(ability::AbilityKind::MortarFire),
        0,
        "manual mortar fire should not launch while the tube is still turning"
    );
    assert!(
        mortar_entity.facing().abs() <= config::MORTAR_TURN_RATE_RAD_PER_TICK + 0.001,
        "mortar should begin turning toward the manual target, got {:.4}",
        mortar_entity.facing()
    );

    let mut launched = false;
    for _ in 0..20 {
        game.tick();
        let mortar_entity = game.entities.get(mortar).expect("mortar should exist");
        if mortar_entity.ability_cooldown_ticks(ability::AbilityKind::MortarFire) > 0 {
            launched = true;
            break;
        }
    }
    assert!(launched, "manual mortar fire should launch once aligned");
    let hp_before_impact = game
        .entities
        .get(target)
        .expect("target should still exist")
        .hp;

    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        game.tick();
    }

    assert!(
        game.entities
            .get(target)
            .is_none_or(|target_after| target_after.hp < hp_before_impact),
        "manual mortar fire should damage or kill units after the delayed impact"
    );
}

#[test]
fn manual_mortar_fire_damages_friendly_units_at_enemy_rate() {
    let players = [
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
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.map.tile_center(8, 8);
    let target_pos = game.map.tile_center(12, 8);
    let mortar = game
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.entities.get_mut(mortar) {
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
    }
    let friendly = game
        .entities
        .spawn_unit(1, EntityKind::MachineGunner, target_pos.0, target_pos.1)
        .expect("friendly should spawn");
    let enemy = game
        .entities
        .spawn_unit(2, EntityKind::MachineGunner, target_pos.0, target_pos.1)
        .expect("enemy should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    game.tick();
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        game.tick();
    }

    assert!(
        !game.entities.contains(friendly),
        "friendly machine gunner should take the same lethal inner-radius hit as an enemy"
    );
    assert!(
        !game.entities.contains(enemy),
        "enemy machine gunner should take the matching lethal inner-radius hit"
    );
}

#[test]
fn manual_mortar_fire_damages_allied_units_without_kill_credit() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "One".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 7,
            faction_id: "kriegsia".to_string(),
            name: "Two".into(),
            color: "#aaa".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 3,
            team_id: 3,
            faction_id: "kriegsia".to_string(),
            name: "Three".into(),
            color: "#000".into(),
            is_ai: false,
        },
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.map.tile_center(8, 8);
    let target_pos = game.map.tile_center(12, 8);
    let mortar = game
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.entities.get_mut(mortar) {
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
    }
    let ally = game
        .entities
        .spawn_unit(2, EntityKind::MachineGunner, target_pos.0, target_pos.1)
        .expect("ally should spawn");
    game.entities
        .get_mut(ally)
        .expect("ally should exist")
        .set_last_damage_owner(Some(3));
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    game.tick();
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        game.tick();
    }

    assert!(
        !game.entities.contains(ally),
        "same-team machine gunner should take lethal mortar splash"
    );
    let scores = game.scores();
    let attacker = scores.iter().find(|score| score.id == 1).unwrap();
    let ally_owner = scores.iter().find(|score| score.id == 2).unwrap();
    let stale_enemy = scores.iter().find(|score| score.id == 3).unwrap();
    assert_eq!(
        attacker.units_killed, 0,
        "same-team mortar splash must not award kill credit"
    );
    assert_eq!(
        stale_enemy.units_killed, 0,
        "same-team lethal splash must clear stale enemy kill credit"
    );
    assert_eq!(ally_owner.units_lost, 1);
}

#[test]
fn manual_mortar_fire_damages_friendly_buildings() {
    let players = [
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
    ];
    let mut game = empty_flat_game(&players);
    let mortar_pos = game.map.tile_center(8, 8);
    let target_pos = game.map.tile_center(12, 8);
    let mortar = game
        .entities
        .spawn_unit(1, EntityKind::MortarTeam, mortar_pos.0, mortar_pos.1)
        .expect("mortar should spawn");
    if let Some(mortar_entity) = game.entities.get_mut(mortar) {
        mortar_entity.set_weapon_setup(WeaponSetup::Deployed);
    }
    let depot = game
        .entities
        .spawn_building(1, EntityKind::Depot, target_pos.0, target_pos.1, true)
        .expect("depot should spawn");
    let hp_before = game.entities.get(depot).expect("depot exists").hp;
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);

    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::MortarFire,
            units: vec![mortar],
            x: Some(target_pos.0),
            y: Some(target_pos.1),
            queued: false,
        },
    );
    game.tick();
    for _ in 0..config::MORTAR_SHELL_DELAY_TICKS {
        game.tick();
    }

    let hp_after = game.entities.get(depot).expect("depot survives").hp;
    assert!(
        hp_after < hp_before,
        "friendly depot should take mortar impact damage, before={hp_before}, after={hp_after}"
    );
}

#[test]
fn snapshot_projects_visible_smoke_but_hides_enemy_inside_it() {
    let (game, _observer, _friendly, enemy, _smoke_pos) = smoke_projection_fixture();

    let snapshot = game.snapshot_for(1);

    assert_eq!(snapshot.smokes.len(), 1);
    assert!(
        snapshot.entities.iter().all(|entity| entity.id != enemy),
        "enemy inside smoke should be withheld from the opposing player snapshot"
    );
}

#[test]
fn snapshot_visibility_grid_fogs_tiles_behind_smoke() {
    let (game, _observer, _friendly, _enemy, _smoke_pos) = smoke_projection_fixture();

    let snapshot = game.snapshot_for(1);
    let index = |tx: u32, ty: u32| (ty * game.map.size + tx) as usize;

    assert_eq!(snapshot.visible_tiles[index(7, 4)], 1);
    assert_eq!(
        snapshot.visible_tiles[index(11, 4)],
        0,
        "tile behind smoke should be fogged in the server-provided visibility grid"
    );
}

#[test]
fn snapshot_keeps_friendly_unit_inside_smoke_visible_to_owner() {
    let (game, _observer, friendly, _enemy, _smoke_pos) = smoke_projection_fixture();

    let snapshot = game.snapshot_for(1);

    assert!(
        snapshot.entities.iter().any(|entity| entity.id == friendly),
        "friendly unit inside smoke should remain owner-visible"
    );
}

#[test]
fn snapshot_keeps_smoke_visible_to_owner_with_unit_inside() {
    let (mut game, _observer, friendly, _enemy, smoke_pos) = smoke_projection_fixture();

    if let Some(unit) = game.entities.get_mut(friendly) {
        unit.pos_x = smoke_pos.0;
        unit.pos_y = smoke_pos.1;
    }
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog
        .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);

    let snapshot = game.snapshot_for(1);

    assert_eq!(
        snapshot.smokes.len(),
        1,
        "owner should still receive the smoke cloud when their unit is inside it"
    );
}

#[test]
fn smoke_expiration_restores_fog_projection() {
    let (mut game, _observer, _friendly, enemy, _smoke_pos) = smoke_projection_fixture();

    for _ in 0..=config::SMOKE_CLOUD_DURATION_TICKS {
        game.tick();
    }
    let snapshot = game.snapshot_for(1);

    assert!(snapshot.smokes.is_empty());
    assert!(
        snapshot.entities.iter().any(|entity| entity.id == enemy),
        "enemy should become visible again once smoke expires"
    );
}

#[test]
fn smoke_queued_order_skipped_when_caster_dies() {
    let (mut game, scout, target, _) = smoke_command_fixture();
    use crate::game::ability::AbilityKind;
    use crate::game::command::SimCommand;
    // Queue a smoke command (unit already at range per fixture)
    game.enqueue(
        1,
        SimCommand::UseAbility {
            ability: AbilityKind::Smoke,
            units: vec![scout],
            x: Some(target.0),
            y: Some(target.1),
            queued: true,
        },
    );
    // Kill the scout car
    if let Some(e) = game.entities.get_mut(scout) {
        e.hp = 0;
    }
    // Tick — death system runs, then order queue promotion
    game.tick();
    assert_eq!(
        game.smokes.iter().count(),
        0,
        "dead scout car should not launch queued smoke"
    );
}

#[test]
fn smoke_nonfinite_target_coordinates_are_rejected() {
    let (mut game, scout, _target, _) = smoke_command_fixture();
    use crate::game::ability::AbilityKind;
    use crate::game::command::SimCommand;
    game.enqueue(
        1,
        SimCommand::UseAbility {
            ability: AbilityKind::Smoke,
            units: vec![scout],
            x: Some(f32::NAN),
            y: Some(f32::INFINITY),
            queued: false,
        },
    );
    game.tick();
    assert_eq!(
        game.smokes.iter().count(),
        0,
        "non-finite coordinates should be rejected"
    );
}

#[test]
fn scout_car_smoke_has_two_free_uses_then_depletes() {
    let (mut game, scout, _target, _) = smoke_command_fixture();
    let target = game.map.tile_center(12, 8);
    game.players[0].steel = 0;
    game.players[0].oil = 0;

    for expected_remaining in [1, 0] {
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

        let scout_entity = game.entities.get_mut(scout).expect("scout should exist");
        assert_eq!(
            scout_entity.ability_uses_remaining(ability::AbilityKind::Smoke),
            Some(expected_remaining)
        );
        scout_entity.start_ability_cooldown(ability::AbilityKind::Smoke, 0);
    }

    for _ in 0..config::SMOKE_LAUNCH_MAX_DELAY_TICKS {
        game.tick();
    }
    assert_eq!(game.smokes.iter().count(), 2);
    assert_eq!(game.players[0].steel, 0);
    assert_eq!(game.players[0].oil, 0);

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

    let scout_entity = game.entities.get(scout).expect("scout should exist");
    assert_eq!(game.smokes.iter().count(), 2);
    assert_eq!(
        scout_entity.ability_uses_remaining(ability::AbilityKind::Smoke),
        Some(0)
    );
    assert_eq!(
        scout_entity.ability_cooldown_ticks(ability::AbilityKind::Smoke),
        0
    );
}

fn queued_move_fixture() -> (Game, u32, (f32, f32), (f32, f32), (f32, f32)) {
    queued_move_fixture_with_lobby_debug(false)
}

fn smoke_command_fixture() -> (Game, u32, (f32, f32), (f32, f32)) {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = Game::new_for_replay_with_starting_resources(&players, 500, 500, 0x5150_0303);
    for tile in &mut game.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }

    let scout_pos = game.map.tile_center(8, 8);
    let target = game.map.tile_center(20, 8);
    let second_target = game.map.tile_center(21, 10);
    let scout = game
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, scout_pos.0, scout_pos.1)
        .expect("scout car should spawn");
    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog
        .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);
    game.assert_invariants();

    (game, scout, target, second_target)
}

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
    assert!(
        game.entities
            .get(nearby)
            .expect("nearby unit")
            .breakthrough_ticks()
            > 0
    );
    assert_eq!(
        game.entities
            .get(far)
            .expect("far unit")
            .breakthrough_ticks(),
        0
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

fn queued_move_fixture_with_lobby_debug(
    lobby_debug: bool,
) -> (Game, u32, (f32, f32), (f32, f32), (f32, f32)) {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let mut game = if lobby_debug {
        Game::new_with_debug_starting_loadout_and_random_ai_profiles(
            &players,
            crate::config::QUICKSTART_STEEL,
            crate::config::QUICKSTART_OIL,
            0x5150_0001,
        )
    } else {
        Game::new_for_replay(&players, 0x5150_0001)
    };
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

struct MixedQueuedFixture {
    game: Game,
    worker_builder: u32,
    worker_gatherer: u32,
    rifleman: u32,
    enemy: u32,
    node: u32,
    move_goal: (f32, f32),
    attack_move_goal: (f32, f32),
}

fn mixed_queued_fixture() -> MixedQueuedFixture {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
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
    ];
    let mut game =
        Game::new_for_replay_with_starting_resources(&players, 5_000, 5_000, 0x5150_0601);
    for tile in &mut game.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }

    let (cc_x, cc_y) =
        services::occupancy::footprint_center(&game.map, EntityKind::CityCentre, 4, 4);
    game.entities
        .spawn_building(1, EntityKind::CityCentre, cc_x, cc_y, true)
        .expect("player city centre should spawn");
    let (enemy_cc_x, enemy_cc_y) =
        services::occupancy::footprint_center(&game.map, EntityKind::CityCentre, 24, 4);
    game.entities
        .spawn_building(2, EntityKind::CityCentre, enemy_cc_x, enemy_cc_y, true)
        .expect("enemy city centre should spawn");

    let node = game
        .entities
        .spawn_node(EntityKind::Steel, cc_x + 96.0, cc_y)
        .expect("resource node should spawn");
    let worker_builder = game
        .entities
        .spawn_unit(1, EntityKind::Worker, cc_x + 96.0, cc_y + 32.0)
        .expect("builder should spawn");
    let worker_gatherer = game
        .entities
        .spawn_unit(1, EntityKind::Worker, cc_x, cc_y + 96.0)
        .expect("gatherer should spawn");
    let rifleman = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, cc_x + 96.0, cc_y + 160.0)
        .expect("rifleman should spawn");
    let enemy = game
        .entities
        .spawn_unit(2, EntityKind::Rifleman, cc_x + 224.0, cc_y + 160.0)
        .expect("enemy should spawn");

    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog.recompute(&ids, &game.entities, &game.map);
    game.assert_invariants();

    MixedQueuedFixture {
        game,
        worker_builder,
        worker_gatherer,
        rifleman,
        enemy,
        node,
        move_goal: (cc_x + 128.0, cc_y + 160.0),
        attack_move_goal: (cc_x + 192.0, cc_y + 160.0),
    }
}

struct PhaseSixIntentFixture {
    game: Game,
    scout_a: u32,
    scout_b: u32,
    rifleman: u32,
    anti_tank_gun: u32,
    first_move: (f32, f32),
    second_move: (f32, f32),
    smoke_targets: [(f32, f32); 4],
    charge_goal: (f32, f32),
    attack_move_goal: (f32, f32),
    setup_facing: (f32, f32),
}

fn phase_six_intent_fixture() -> PhaseSixIntentFixture {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
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
    ];
    let mut game =
        Game::new_for_replay_with_starting_resources(&players, 5_000, 5_000, 0x5150_0602);
    for tile in &mut game.map.terrain {
        *tile = crate::protocol::terrain::GRASS;
    }
    for id in game.entities.ids() {
        game.entities.remove(id);
    }

    let (steelworks_x, steelworks_y) =
        services::occupancy::footprint_center(&game.map, EntityKind::Steelworks, 4, 4);
    game.entities
        .spawn_building(1, EntityKind::Steelworks, steelworks_x, steelworks_y, true)
        .expect("steelworks should spawn");
    let (training_x, training_y) =
        services::occupancy::footprint_center(&game.map, EntityKind::TrainingCentre, 8, 4);
    game.entities
        .spawn_building(1, EntityKind::TrainingCentre, training_x, training_y, true)
        .expect("training centre should spawn");
    let scout_a_pos = game.map.tile_center(8, 10);
    let scout_b_pos = game.map.tile_center(8, 12);
    let scout_a = game
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, scout_a_pos.0, scout_a_pos.1)
        .expect("first scout should spawn");
    let scout_b = game
        .entities
        .spawn_unit(1, EntityKind::ScoutCar, scout_b_pos.0, scout_b_pos.1)
        .expect("second scout should spawn");
    let rifle_pos = game.map.tile_center(9, 14);
    let rifleman = game
        .entities
        .spawn_unit(1, EntityKind::Rifleman, rifle_pos.0, rifle_pos.1)
        .expect("rifleman should spawn");
    let at_pos = game.map.tile_center(10, 14);
    let anti_tank_gun = game
        .entities
        .spawn_unit(1, EntityKind::AntiTankGun, at_pos.0, at_pos.1)
        .expect("Anti-Tank Gun should spawn");
    let enemy_pos = game.map.tile_center(18, 14);
    game.entities
        .spawn_unit(2, EntityKind::Rifleman, enemy_pos.0, enemy_pos.1)
        .expect("enemy should spawn");

    systems::recompute_supply(&mut game.players, &game.entities);
    game.spatial = services::spatial::SpatialIndex::build(&game.entities, game.map.size);
    let ids: Vec<u32> = game.players.iter().map(|p| p.id).collect();
    game.fog
        .recompute_with_smoke(&ids, &game.entities, &game.map, &game.smokes);
    game.assert_invariants();

    let first_move = game.map.tile_center(12, 10);
    let second_move = game.map.tile_center(14, 12);
    let smoke_targets = [
        game.map.tile_center(13, 10),
        game.map.tile_center(13, 12),
        game.map.tile_center(14, 10),
        game.map.tile_center(14, 12),
    ];
    let charge_goal = game.map.tile_center(12, 14);
    let attack_move_goal = game.map.tile_center(16, 14);
    let setup_facing = game.map.tile_center(18, 14);

    PhaseSixIntentFixture {
        game,
        scout_a,
        scout_b,
        rifleman,
        anti_tank_gun,
        first_move,
        second_move,
        smoke_targets,
        charge_goal,
        attack_move_goal,
        setup_facing,
    }
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
    let path = moved_tank
        .movement
        .as_ref()
        .expect("tank should have movement")
        .path
        .as_slice();
    assert_eq!(
        path.first().copied(),
        Some(goal),
        "reverse-ordered tank path should preserve the exact command goal"
    );
    assert!(
        path.len() > 1,
        "tank movement should keep clearance-shaped intermediate waypoints"
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
fn out_of_range_smoke_moves_into_range_launches_then_idles() {
    let (mut game, scout, target, _) = smoke_command_fixture();

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
        matches!(game.entities.get(scout).unwrap().order(), Order::Ability(_)),
        "out-of-range Smoke should become an active ability movement order"
    );

    for _ in 0..240 {
        if game.smokes.iter().count() > 0 {
            break;
        }
        game.tick();
    }

    let scout_entity = game.entities.get(scout).expect("scout should exist");
    assert_eq!(
        game.smokes.iter().count(),
        1,
        "Smoke cloud should spawn once the scout car reaches launch range"
    );
    assert!(matches!(scout_entity.order(), Order::Idle));
    assert_eq!(
        scout_entity.ability_cooldown_ticks(ability::AbilityKind::Smoke),
        config::SMOKE_ABILITY_COOLDOWN_TICKS
            .saturating_sub(config::SMOKE_LAUNCH_MAX_DELAY_TICKS as u16)
    );
    assert_eq!(game.players[0].steel, 500);
    assert_eq!(game.players[0].oil, 500);
}

#[test]
fn queued_out_of_range_smoke_command_log_replays_deterministically() {
    let (mut live, scout, first_target, second_target) = smoke_command_fixture();

    live.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::Smoke,
            units: vec![scout],
            x: Some(first_target.0),
            y: Some(first_target.1),
            queued: true,
        },
    );
    live.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::Smoke,
            units: vec![scout],
            x: Some(second_target.0),
            y: Some(second_target.1),
            queued: true,
        },
    );

    let mut live_events = Vec::new();
    for tick in 1..=180 {
        for (player_id, events) in live.tick() {
            for event in events {
                live_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert!(
        live.command_log().iter().any(|entry| matches!(
            entry.command,
            crate::protocol::Command::UseAbility {
                ref ability,
                queued: true,
                ..
            } if ability == crate::protocol::abilities::SMOKE
        )),
        "command log should preserve queued Smoke intent"
    );

    let mut replay = smoke_command_fixture().0;
    let command_log = live.command_log().to_vec();
    let mut next_command = 0usize;
    let mut replay_events = Vec::new();
    for tick in 1..=live.tick_count() {
        while let Some(entry) = command_log.get(next_command) {
            if entry.tick != tick {
                break;
            }
            replay.enqueue(
                entry.player_id,
                Command::from_protocol(entry.command.clone()),
            );
            next_command += 1;
        }
        for (player_id, events) in replay.tick() {
            for event in events {
                replay_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert_eq!(next_command, command_log.len());
    assert_eq!(live_events, replay_events);
    assert_eq!(live.snapshot_for(1), replay.snapshot_for(1));
}

#[test]
fn mixed_queued_command_log_replays_deterministically() {
    let MixedQueuedFixture {
        mut game,
        worker_builder,
        worker_gatherer,
        rifleman,
        enemy,
        node,
        move_goal,
        attack_move_goal,
    } = mixed_queued_fixture();

    game.enqueue(
        1,
        Command::Move {
            units: vec![rifleman],
            x: move_goal.0,
            y: move_goal.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![rifleman],
            x: attack_move_goal.0,
            y: attack_move_goal.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Attack {
            units: vec![rifleman],
            target: enemy,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Gather {
            units: vec![worker_gatherer],
            node,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Build {
            units: vec![worker_builder],
            building: EntityKind::Depot,
            tile_x: 12,
            tile_y: 12,
            queued: true,
        },
    );

    let mut live_events = Vec::new();
    for tick in 1..=180 {
        for (player_id, events) in game.tick() {
            for event in events {
                live_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    let command_log = game.command_log().to_vec();
    assert!(
        command_log.iter().any(|entry| matches!(
            entry.command,
            crate::protocol::Command::Attack { queued: true, .. }
        )),
        "command log should preserve queued mixed attack intent"
    );

    let mut replay = mixed_queued_fixture().game;
    let mut next_command = 0usize;
    let mut replay_events = Vec::new();
    for tick in 1..=game.tick_count() {
        while let Some(entry) = command_log.get(next_command) {
            if entry.tick != tick {
                break;
            }
            replay.enqueue(
                entry.player_id,
                Command::from_protocol(entry.command.clone()),
            );
            next_command += 1;
        }
        for (player_id, events) in replay.tick() {
            for event in events {
                replay_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert_eq!(next_command, command_log.len());
    assert_eq!(live_events, replay_events);
    assert_eq!(game.snapshot_for(1), replay.snapshot_for(1));
    assert_eq!(game.snapshot_for(2), replay.snapshot_for(2));
}

#[test]
fn phase_six_mixed_intent_command_log_replays_deterministically() {
    let PhaseSixIntentFixture {
        mut game,
        scout_a,
        scout_b,
        rifleman,
        anti_tank_gun,
        first_move,
        second_move,
        smoke_targets,
        charge_goal,
        attack_move_goal,
        setup_facing,
    } = phase_six_intent_fixture();

    game.enqueue(
        1,
        Command::Move {
            units: vec![scout_a, scout_b],
            x: first_move.0,
            y: first_move.1,
            queued: false,
        },
    );
    for target in smoke_targets {
        game.enqueue(
            1,
            Command::UseAbility {
                ability: ability::AbilityKind::Smoke,
                units: vec![scout_a, scout_b],
                x: Some(target.0),
                y: Some(target.1),
                queued: true,
            },
        );
    }
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![scout_a, scout_b],
            x: second_move.0,
            y: second_move.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Move {
            units: vec![rifleman],
            x: charge_goal.0,
            y: charge_goal.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::UseAbility {
            ability: ability::AbilityKind::Charge,
            units: vec![rifleman],
            x: None,
            y: None,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::AttackMove {
            units: vec![rifleman],
            x: attack_move_goal.0,
            y: attack_move_goal.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::Move {
            units: vec![anti_tank_gun],
            x: charge_goal.0,
            y: charge_goal.1,
            queued: true,
        },
    );
    game.enqueue(
        1,
        Command::SetupAntiTankGuns {
            units: vec![anti_tank_gun],
            x: setup_facing.0,
            y: setup_facing.1,
            queued: true,
        },
    );

    let mut live_events = Vec::new();
    for tick in 1..=260 {
        for (player_id, events) in game.tick() {
            for event in events {
                live_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    let command_log = game.command_log().to_vec();
    assert_eq!(
        command_log
            .iter()
            .filter(|entry| matches!(
                entry.command,
                crate::protocol::Command::UseAbility {
                    ref ability,
                    queued: true,
                    ..
                } if ability == crate::protocol::abilities::SMOKE
            ))
            .count(),
        4,
        "command log should preserve all queued Smoke intents"
    );
    assert!(command_log.iter().any(|entry| matches!(
        entry.command,
        crate::protocol::Command::UseAbility {
            ref ability,
            queued: true,
            ..
        } if ability == crate::protocol::abilities::CHARGE
    )));
    assert!(command_log.iter().any(|entry| matches!(
        entry.command,
        crate::protocol::Command::SetupAntiTankGuns { queued: true, .. }
    )));

    let mut replay = phase_six_intent_fixture().game;
    let mut next_command = 0usize;
    let mut replay_events = Vec::new();
    for tick in 1..=game.tick_count() {
        while let Some(entry) = command_log.get(next_command) {
            if entry.tick != tick {
                break;
            }
            replay.enqueue(
                entry.player_id,
                Command::from_protocol(entry.command.clone()),
            );
            next_command += 1;
        }
        for (player_id, events) in replay.tick() {
            for event in events {
                replay_events.push(super::replay::EventLogEntry {
                    tick,
                    player_id,
                    event,
                });
            }
        }
    }

    assert_eq!(next_command, command_log.len());
    assert_eq!(live_events, replay_events);
    assert_eq!(game.snapshot_for(1), replay.snapshot_for(1));
    assert_eq!(game.snapshot_for(2), replay.snapshot_for(2));
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
        view.order_plan,
        vec![
            crate::protocol::OrderPlanMarker {
                kind: "move".to_string(),
                x: first.0,
                y: first.1,
            },
            crate::protocol::OrderPlanMarker {
                kind: "move".to_string(),
                x: second.0,
                y: second.1,
            },
        ]
    );
}

#[test]
fn lobby_debug_mode_snapshot_shows_runtime_movement_debug_path() {
    for (lobby_debug, expected_debug_path) in [(false, false), (true, true)] {
        let (mut game, unit, first, _, _) = queued_move_fixture_with_lobby_debug(lobby_debug);

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

        let view = game
            .snapshot_for(1)
            .entities
            .into_iter()
            .find(|entity| entity.id == unit)
            .expect("selected unit should be visible to owner");
        assert_eq!(
            view.debug_path.is_some(),
            expected_debug_path,
            "debug path visibility should follow lobby Debug mode"
        );
        if let Some(debug_path) = view.debug_path {
            assert_eq!(
                debug_path.waypoints.first().map(|point| (point.x, point.y)),
                game.entities
                    .get(unit)
                    .and_then(|entity| entity.next_waypoint())
            );
        }
    }
}

#[test]
fn dev_scenario_snapshot_shows_runtime_movement_debug_path() {
    let setup = Game::new_snaking_corridor_scenario(EntityKind::ScoutCar, 1, 0x5150_0002)
        .expect("scenario setup should succeed");
    let mut game = setup.game;
    let unit = setup.units[0];
    game.enqueue(
        setup.player_id,
        Command::Move {
            units: vec![unit],
            x: setup.goal.0,
            y: setup.goal.1,
            queued: false,
        },
    );
    game.tick();

    let view = game
        .snapshot_for(setup.player_id)
        .entities
        .into_iter()
        .find(|entity| entity.id == unit)
        .expect("scenario unit should be visible to owner");
    assert!(
        view.debug_path.is_some(),
        "dev scenario snapshots should include movement debug paths"
    );
}

#[test]
fn wall_chokepoint_dev_scenario_matches_authored_layout() {
    let setup = Game::new_scout_car_wall_chokepoint_scenario(EntityKind::ScoutCar, 15, 0x5150_0003)
        .expect("scenario setup should succeed");
    let mut game = setup.game;

    assert_eq!(setup.units.len(), 15);
    let wall_y = game.map.size - 18;
    let gap_left_x = game.map.size / 2 - 1;
    let gap_right_x = game.map.size / 2;
    assert_eq!(
        game.map.terrain[game.map.index(gap_left_x, wall_y)],
        terrain::GRASS
    );
    assert_eq!(
        game.map.terrain[game.map.index(gap_right_x, wall_y)],
        terrain::GRASS
    );
    assert_eq!(
        game.map.terrain[game.map.index(gap_left_x - 1, wall_y)],
        terrain::ROCK
    );
    assert_eq!(
        game.map.terrain[game.map.index(gap_right_x + 1, wall_y)],
        terrain::ROCK
    );

    let start_y = (wall_y + 10) as f32 * config::TILE_SIZE as f32 + config::TILE_SIZE as f32 * 0.5;
    let north = -std::f32::consts::FRAC_PI_2;
    for unit in &setup.units {
        let entity = game.entities.get(*unit).expect("scenario unit exists");
        assert_eq!(entity.kind, EntityKind::ScoutCar);
        assert!((entity.pos_y - start_y).abs() < 0.1);
        assert!((entity.facing() - north).abs() < 0.001);
    }

    let command_units: Vec<u32> = setup.units.iter().copied().take(8).collect();
    game.enqueue(
        setup.player_id,
        Command::Move {
            units: command_units.clone(),
            x: setup.goal.0,
            y: setup.goal.1,
            queued: false,
        },
    );
    game.tick();
    for unit in command_units {
        let view = game
            .snapshot_for(setup.player_id)
            .entities
            .into_iter()
            .find(|entity| entity.id == unit)
            .expect("scenario scout car should be visible to owner");
        assert!(
            view.debug_path.is_some(),
            "wall chokepoint scenario should issue movement debug paths"
        );
    }
}

#[test]
fn wall_chokepoint_dev_scenario_supports_all_vehicles() {
    for unit in [
        EntityKind::AntiTankGun,
        EntityKind::ScoutCar,
        EntityKind::Tank,
    ] {
        let setup = Game::new_scout_car_wall_chokepoint_scenario(unit, 5, 0x5150_0004)
            .expect("scenario setup should succeed");

        assert_eq!(setup.units.len(), 5);
        for unit_id in setup.units {
            let entity = setup
                .game
                .entities
                .get(unit_id)
                .expect("scenario unit exists");
            assert_eq!(entity.kind, unit);
        }
    }
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
    let teams = game.team_relations();
    services::death::death_system(
        &mut game.entities,
        &game.fog,
        &game.smokes,
        &teams,
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
fn observer_analysis_reports_authoritative_inventory_production_and_losses() {
    let players = human_vs_ai_players();
    let mut game =
        Game::new_for_replay_with_starting_resources(&players, 5_000, 5_000, 0xA11A_0001);
    let city_centre = game
        .entities
        .iter()
        .find(|e| e.owner == 1 && e.kind == EntityKind::CityCentre)
        .map(|e| e.id)
        .expect("player city centre should exist");
    game.enqueue(
        1,
        Command::Train {
            building: city_centre,
            unit: EntityKind::Worker,
        },
    );
    game.tick();

    let victim_unit = game
        .entities
        .iter()
        .find(|e| e.owner == 2 && e.kind == EntityKind::Worker)
        .map(|e| e.id)
        .expect("victim unit should exist");
    let entity = game
        .entities
        .get_mut(victim_unit)
        .expect("victim unit should still exist");
    entity.hp = 0;
    entity.set_last_damage_owner(Some(1));
    let mut events: HashMap<u32, Vec<Event>> =
        game.players.iter().map(|p| (p.id, Vec::new())).collect();
    let mut lingering_sight = Vec::new();
    let tick = game.tick_count();
    let teams = game.team_relations();
    services::death::death_system(
        &mut game.entities,
        &game.fog,
        &game.smokes,
        &teams,
        &mut game.players,
        &mut lingering_sight,
        &mut events,
        tick,
    );

    let analysis = game.observer_analysis();
    assert_eq!(analysis.tick, game.tick_count());
    let player_one = analysis
        .players
        .iter()
        .find(|player| player.id == 1)
        .expect("player one analysis should exist");
    assert!(player_one
        .units
        .iter()
        .any(|row| row.kind == "worker" && row.count == config::STARTING_WORKERS));
    assert!(player_one.production.iter().any(|row| {
        row.building_id == city_centre
            && row.building_kind == "city_centre"
            && row.item_kind == "worker"
            && row.item_type == "unit"
            && row.queue_depth == 1
            && row.progress > 0.0
    }));
    let player_two = analysis
        .players
        .iter()
        .find(|player| player.id == 2)
        .expect("player two analysis should exist");
    assert!(player_two
        .units_lost
        .iter()
        .any(|row| row.kind == "worker" && row.count == 1 && row.steel_value > 0));
    assert_eq!(
        player_two.resources_lost.steel,
        player_two.units_lost[0].steel_value
    );
    assert_eq!(
        player_two.resources_lost.oil,
        player_two.units_lost[0].oil_value
    );
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
fn death_vision_lingers_for_five_seconds_as_visual_only_intel() {
    let players = [
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

    while game.tick_count() <= config::TICK_HZ * 5 {
        game.tick();
    }
    assert!(
        game.snapshot_for(1).entities.iter().all(|e| e.id != enemy),
        "lingering death vision should expire after five seconds"
    );
}

/// Adding an AI identity must not perturb a human-only game's construction.
#[test]
fn human_only_match_has_no_ai_players() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Solo".into(),
        color: "#fff".into(),
        is_ai: false,
    }];
    let game = Game::new(&players, 0x1234_5678);
    assert!(game.players.iter().all(|player| !player.is_ai));
}

#[test]
fn replay_games_preserve_ai_identity_without_controllers() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "kriegsia".to_string(),
        name: "Computer".into(),
        color: "#fff".into(),
        is_ai: true,
    }];
    let game = Game::new_without_ai_controllers(&players, 0x1234_5678);

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
        team_id: 1,
        faction_id: "kriegsia".to_string(),
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
        team_id: 1,
        faction_id: "kriegsia".to_string(),
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
        team_id: 1,
        faction_id: "kriegsia".to_string(),
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
        team_id: 1,
        faction_id: "kriegsia".to_string(),
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

    game.tick();
    assert!(
        matches!(
            game.entities.get(worker).map(|e| e.order()),
            Some(Order::Move(_))
        ),
        "worker should scatter away when its mining City Centre disappears"
    );

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
        "worker should not resume gathering without City Centre coverage"
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
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "A".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
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
        team_id: 1,
        faction_id: "kriegsia".to_string(),
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

    let replay = super::replay::replay_commands(
        &players,
        game.command_log(),
        game.tick_count(),
        game.seed(),
        game.starting_loadouts(),
    )
    .expect("one-player no-commands replay should succeed");
    assert_eq!(replay.events, event_log);
    assert_eq!(replay.final_snapshots[0].snapshot, game.snapshot_for(1));
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
                team_id: id,
                faction_id: "kriegsia".to_string(),
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
                let kind_ord =
                    crate::protocol::kind_to_wire(a.0).cmp(crate::protocol::kind_to_wire(b.0));
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
