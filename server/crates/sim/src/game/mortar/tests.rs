use super::*;
use crate::game::map::Map;
use crate::protocol::terrain;

fn open_map(size: u32) -> Map {
    Map {
        width: size,
        height: size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4), (size - 5, size - 5)],
        ..Default::default()
    }
}

fn visible_team_fog(map: &Map, entities: &EntityStore) -> Fog {
    let mut fog = Fog::new(map.width, map.height);
    fog.recompute(&[1, 2, 3], entities, map);
    fog
}

fn has_under_attack_notice(events: &HashMap<u32, Vec<Event>>, player: u32) -> bool {
    events.get(&player).is_some_and(|player_events| {
        player_events
            .iter()
            .any(|event| matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack"))
    })
}

#[test]
fn half_turn_completes_in_two_hundred_ms() {
    assert_eq!(
        HALF_TURN_TICKS * 1000 / config::TICK_HZ,
        200,
        "mortar half-turn timing should stay at 200 ms"
    );

    let mut entities = EntityStore::new();
    let mortar_id = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    if let Some(mortar) = entities.get_mut(mortar_id) {
        mortar.set_facing(0.0);
        mortar.set_weapon_facing(0.0);
    }

    let target_angle = std::f32::consts::PI;
    for tick in 1..=HALF_TURN_TICKS {
        let ready = {
            let mortar = entities.get_mut(mortar_id).expect("mortar should exist");
            rotate_mortar_for_fire(mortar, target_angle)
        };
        if tick < HALF_TURN_TICKS {
            assert!(
                !ready,
                "mortar should still be rotating on half-turn tick {tick}"
            );
        } else {
            assert!(ready, "mortar should complete a 180-degree turn in 200 ms");
        }
    }

    let mortar = entities.get(mortar_id).expect("mortar should exist");
    assert!(
        angle_delta(mortar.facing(), target_angle).abs() <= FIRE_TOLERANCE_RAD + 0.001,
        "mortar should finish the half-turn aligned with the target, got {:.4}",
        mortar.facing()
    );
}

mod shield;

#[test]
fn fixed_mortar_shell_checkpoint_defaults_to_already_launched() {
    let store: MortarShellStore = serde_json::from_value(serde_json::json!({
        "shells": [{
            "owner": 1,
            "attacker": 7,
            "x": 1310720,
            "y": 1441792,
            "impact_tick": 99
        }]
    }))
    .expect("fixed mortar-shell checkpoint should restore");

    let shell = store.shells.first().expect("restored shell");
    assert!(shell.launched);
    assert!(!shell.rocket);
    assert_eq!(
        (
            shell.from_x.to_world(),
            shell.from_y.to_world(),
            shell.launch_tick
        ),
        (0.0, 0.0, 0)
    );
}

#[test]
fn mortar_falloff_resolves_distance_armor_entrenchment_and_friendly_fire() {
    let map = open_map(20);
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    for (distance_tiles, base) in [
        (0.0, 100),
        (0.25, 88),
        (0.5, 64),
        (0.75, 44),
        (1.0, 31),
        (1.25, 22),
        (1.5, 16),
        (2.0, 10),
        (2.001, 0),
    ] {
        for (kind, entrenched) in [
            (EntityKind::Artillery, false),
            (EntityKind::Rifleman, false),
            (EntityKind::Tank, false),
            (EntityKind::Rifleman, false),
            (EntityKind::MachineGunner, false),
            (EntityKind::Rifleman, true),
            (EntityKind::MachineGunner, true),
        ] {
            for owner in [1, 2] {
                let mut entities = EntityStore::new();
                let victim = entities.spawn_unit(owner, kind, 160.0, 160.0).unwrap();
                if entrenched {
                    entities
                        .get_mut(victim)
                        .unwrap()
                        .movement
                        .as_mut()
                        .unwrap()
                        .occupied_trench_id = Some(1);
                }
                let before = entities.get(victim).unwrap().hp;
                let fog = visible_team_fog(&map, &entities);
                let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);
                let mut firing_reveals = Vec::new();
                let shell = MortarShell {
                    double_damage: false,
                    owner: 1,
                    attacker: 0,
                    x: Coord::from_world(160.0),
                    y: Coord::from_world(160.0 + distance_tiles * config::TILE_SIZE as f32),
                    from_x: Coord::from_world(100.0),
                    from_y: Coord::from_world(100.0),
                    launch_tick: 0,
                    impact_tick: 0,
                    launched: true,
                    rocket: false,
                };
                resolve(
                    MortarResolutionContext {
                        entities: &mut entities,
                        teams: &teams,
                        fog: &fog,
                        map: &map,
                        events: &mut events,
                        firing_reveals: &mut firing_reveals,
                        tick: 10,
                    },
                    &shell,
                );
                let damage = if kind == EntityKind::Tank {
                    base / 4
                } else if entrenched {
                    (base as f32 * 0.75).round() as u32
                } else {
                    base
                };
                assert_eq!(
                    entities.get(victim).unwrap().hp,
                    before.saturating_sub(damage),
                    "distance={distance_tiles}, kind={kind:?}, entrenched={entrenched}, owner={owner}"
                );
            }
        }
    }
}

#[test]
fn rocket_blast_bands_apply_damage_and_armor_at_exact_boundaries() {
    let map = open_map(20);
    for (distance_tiles, base, armored, half_armor, full_armor, soft_building, armored_building) in [
        (0.0, 70, 70, 70, 70, 18, 18),
        (0.5, 70, 70, 70, 70, 18, 18),
        (0.501, 53, 13, 27, 0, 14, 4),
        (1.0, 53, 13, 27, 0, 14, 4),
        (1.001, 21, 5, 11, 0, 6, 2),
        (2.0, 21, 5, 11, 0, 6, 2),
        (2.001, 0, 0, 0, 0, 0, 0),
    ] {
        for (kind, item) in [
            (EntityKind::Tank, false),
            (EntityKind::Artillery, false),
            (EntityKind::Rifleman, false),
            (EntityKind::ResourceDepot, false),
            (EntityKind::SteelMine, false),
            (EntityKind::PumpJack, false),
            (EntityKind::WindAdept, true),
            (EntityKind::WarAdept, false),
            (EntityKind::WarAdept, true),
        ] {
            let mut entities = EntityStore::new();
            let victim = if config::building_stats(kind).is_some() {
                entities
                    .spawn_building(2, kind, 160.0, 160.0, true)
                    .unwrap()
            } else {
                entities.spawn_unit(2, kind, 160.0, 160.0).unwrap()
            };
            // Keep lethal hits from hiding the resolved damage behind the HP cap.
            entities.get_mut(victim).unwrap().set_spawn_health(1000);
            if item {
                entities
                    .get_mut(victim)
                    .unwrap()
                    .receive_shop_item(crate::game::ability::AbilityKind::ArmorTreasure);
            }
            let before = entities.get(victim).unwrap().hp;
            let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
            let fog = visible_team_fog(&map, &entities);
            let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);
            let mut firing_reveals = Vec::new();
            let shell = MortarShell {
                double_damage: false,
                owner: 1,
                attacker: 0,
                x: rts_rules::math::Coord::from_world(160.0),
                y: rts_rules::math::Coord::from_world(
                    160.0 + distance_tiles * config::TILE_SIZE as f32,
                ),
                from_x: rts_rules::math::Coord::from_world(100.0),
                from_y: rts_rules::math::Coord::from_world(100.0),
                launch_tick: 0,
                impact_tick: 0,
                launched: true,
                rocket: true,
            };
            resolve(
                MortarResolutionContext {
                    entities: &mut entities,
                    teams: &teams,
                    fog: &fog,
                    map: &map,
                    events: &mut events,
                    firing_reveals: &mut firing_reveals,
                    tick: 10,
                },
                &shell,
            );
            let expected_damage = match kind {
                EntityKind::Tank => armored,
                EntityKind::ResourceDepot => armored_building,
                EntityKind::SteelMine | EntityKind::PumpJack => soft_building,
                EntityKind::WindAdept => half_armor,
                EntityKind::WarAdept if item => full_armor,
                EntityKind::WarAdept => half_armor,
                _ => base,
            };
            assert_eq!(
                before - entities.get(victim).unwrap().hp,
                expected_damage,
                "kind={kind:?}, item={item}, distance={distance_tiles}"
            );
        }
    }
}

#[test]
fn mortar_under_attack_notice_goes_to_victim_owner_not_teammate() {
    let map = open_map(20);
    let mut entities = EntityStore::new();
    entities
        .spawn_unit(2, EntityKind::Worker, 160.0, 160.0)
        .expect("victim should spawn");
    entities
        .spawn_unit(3, EntityKind::Worker, 176.0, 160.0)
        .expect("victim ally should spawn");
    let fog = visible_team_fog(&map, &entities);
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 7), (3, 7)]);
    let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new()), (3, Vec::new())]);

    push_under_attack_notice(&mut events, &teams, &fog, 1, 2, 160.0, 160.0);

    assert!(has_under_attack_notice(&events, 2));
    assert!(!has_under_attack_notice(&events, 3));
    assert!(!has_under_attack_notice(&events, 1));
}
