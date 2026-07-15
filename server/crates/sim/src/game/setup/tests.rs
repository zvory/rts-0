use super::*;
use crate::game::ability::AbilityKind;
use crate::game::entity::Order;
use crate::rules::faction::EMPTY_FIXTURE_FACTION_ID;

fn owned_kind_count(game: &Game, owner: u32, kind: EntityKind) -> usize {
    game.state
        .entities
        .iter()
        .filter(|e| e.owner == owner && e.kind == kind)
        .count()
}

#[test]
fn fixture_faction_start_uses_catalog_loadout_and_shared_resources() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "Kriegsia".to_string(),
            color: "#cc1111".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: EMPTY_FIXTURE_FACTION_ID.to_string(),
            name: "Fixture".to_string(),
            color: "#1133bb".to_string(),
            is_ai: false,
        },
    ];
    let game = Game::new(&players, 9);
    game.assert_invariants();

    let fixture = game.state.players.iter().find(|p| p.id == 2).unwrap();
    assert_eq!(fixture.faction_id, EMPTY_FIXTURE_FACTION_ID);
    assert_eq!(fixture.steel, 125);
    assert_eq!(fixture.oil, 25);
    assert_eq!(fixture.supply_cap, config::INTRINSIC_SUPPLY_CAP);
    assert_eq!(
        fixture.supply_used,
        crate::rules::economy::supply_cost(EntityKind::ScoutCar)
    );
    assert_eq!(owned_kind_count(&game, 2, EntityKind::Depot), 1);
    assert_eq!(owned_kind_count(&game, 2, EntityKind::ScoutCar), 1);
    assert_eq!(owned_kind_count(&game, 2, EntityKind::CityCentre), 0);
    assert_eq!(owned_kind_count(&game, 2, EntityKind::Worker), 0);

    let loadout = game
        .starting_loadouts()
        .iter()
        .find(|loadout| loadout.player_id == 2)
        .unwrap();
    assert_eq!(loadout.faction_id, EMPTY_FIXTURE_FACTION_ID);
    assert_eq!(loadout.loadout_id, "phase2_empty_fixture.scout_depot");
    assert_eq!(loadout.starting_steel, 125);
    assert_eq!(loadout.starting_oil, 25);

    let resource_nodes = game
        .state
        .entities
        .iter()
        .filter(|e| e.kind.is_node())
        .count();
    assert!(
        resource_nodes > 0,
        "fixture starts still use universal Steel/Oil nodes"
    );
}

#[test]
fn unknown_faction_start_and_commands_fail_closed() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "unknown_faction".to_string(),
        name: "Unknown".to_string(),
        color: "#cc1111".to_string(),
        is_ai: false,
    }];
    let mut game = Game::new(&players, 9);
    game.assert_invariants();

    assert_eq!(
        game.state
            .entities
            .iter()
            .filter(|entity| entity.kind.is_node())
            .count(),
        game.state.map.base_sites.len()
            * (config::STEEL_PATCHES_PER_BASE as usize + config::OIL_PATCHES_PER_BASE as usize),
        "an invalid faction must not leave its permanent start/base site without resources"
    );

    assert_eq!(
        (
            game.state.players[0].steel,
            game.state.players[0].oil,
            game.state.players[0].supply_used,
            game.state.players[0].supply_cap
        ),
        (0, 0, 0, config::INTRINSIC_SUPPLY_CAP)
    );
    assert_eq!(owned_kind_count(&game, 1, EntityKind::CityCentre), 0);
    let loadout = &game.starting_loadouts()[0];
    assert_eq!(loadout.loadout_id, "unknown_faction.invalid");
    assert_eq!((loadout.starting_steel, loadout.starting_oil), (0, 0));

    let (x, y) = game.state.map.tile_center(4, 4);
    let worker = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, x, y)
        .unwrap();
    let city_centre = game
        .state
        .entities
        .spawn_building(1, EntityKind::CityCentre, x + 96.0, y, true)
        .unwrap();
    let research_complex = game
        .state
        .entities
        .spawn_building(1, EntityKind::ResearchComplex, x + 192.0, y, true)
        .unwrap();
    let artillery = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Artillery, x, y + 96.0)
        .unwrap();
    let node = game
        .state
        .entities
        .spawn_node(EntityKind::Steel, x + 320.0, y + 320.0)
        .unwrap();
    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    let resources_before = (
        game.state.players[0].steel,
        game.state.players[0].oil,
        game.state.players[0].supply_used,
        game.state.players[0].supply_cap,
    );
    assert_eq!(resources_before, (0, 0, 0, config::INTRINSIC_SUPPLY_CAP));

    for cmd in [
        SimCommand::Build {
            units: vec![worker],
            building: EntityKind::Depot,
            tile_x: 8,
            tile_y: 8,
            queued: false,
        },
        SimCommand::Train {
            building: city_centre,
            unit: EntityKind::Worker,
        },
        SimCommand::Research {
            building: research_complex,
            upgrade: upgrade::UpgradeKind::TankUnlock,
        },
        SimCommand::Gather {
            units: vec![worker],
            node,
            queued: false,
        },
        SimCommand::UseAbility {
            ability: AbilityKind::PointFire,
            units: vec![artillery],
            x: Some(x + 256.0),
            y: Some(y + 96.0),
            queued: false,
        },
    ] {
        game.enqueue(1, cmd);
    }
    game.tick();

    assert_eq!(
        (
            game.state.players[0].steel,
            game.state.players[0].oil,
            game.state.players[0].supply_used,
            game.state.players[0].supply_cap
        ),
        resources_before,
        "rejected unknown-faction commands must not spend resources or reserve supply"
    );
    assert!(!matches!(
        game.state.entities.get(worker).expect("worker").order(),
        Order::Build(_) | Order::Gather(_)
    ));
    assert!(game
        .state
        .entities
        .get(city_centre)
        .expect("city centre")
        .prod_queue()
        .is_empty());
    assert!(game
        .state
        .entities
        .get(research_complex)
        .expect("research complex")
        .research_queue()
        .is_empty());
    let artillery = game.state.entities.get(artillery).expect("artillery");
    assert!(!matches!(artillery.order(), Order::Ability(_)));
    assert_eq!(artillery.ability_cooldown_ticks(AbilityKind::PointFire), 0);
}

#[test]
fn standard_starting_loadout_matches_phase0_inventory() {
    assert_eq!(config::STARTING_WORKERS, 6);
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: "kriegsia".to_string(),
            name: "One".to_string(),
            color: "#cc1111".to_string(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: "kriegsia".to_string(),
            name: "Two".to_string(),
            color: "#1133bb".to_string(),
            is_ai: false,
        },
    ];
    let game = Game::new(&players, 7);

    assert_eq!(game.starting_steel(), config::STARTING_STEEL);
    assert_eq!(game.starting_oil(), config::STARTING_OIL);
    assert_eq!(game.starting_loadouts()[0].loadout_id, "kriegsia.standard");

    for player in &game.state.players {
        assert_eq!(player.faction_id, DEFAULT_FACTION_ID);
        assert_eq!(player.steel, config::STARTING_STEEL);
        assert_eq!(player.oil, config::STARTING_OIL);
        assert_eq!(player.supply_cap, config::INTRINSIC_SUPPLY_CAP);
        assert_eq!(player.supply_used, config::STARTING_WORKERS);
        assert_eq!(
            owned_kind_count(&game, player.id, EntityKind::CityCentre),
            1
        );
        assert_eq!(
            owned_kind_count(&game, player.id, EntityKind::Worker),
            config::STARTING_WORKERS as usize
        );
        assert_eq!(owned_kind_count(&game, player.id, EntityKind::Depot), 0);
        assert_eq!(owned_kind_count(&game, player.id, EntityKind::Barracks), 0);
        assert_eq!(owned_kind_count(&game, player.id, EntityKind::Factory), 0);
        assert_eq!(
            owned_kind_count(&game, player.id, EntityKind::Steelworks),
            0
        );
    }

    assert!(game
        .state
        .entities
        .iter()
        .any(|e| e.owner == 0 && e.kind == EntityKind::Steel));
    assert!(game
        .state
        .entities
        .iter()
        .any(|e| e.owner == 0 && e.kind == EntityKind::Oil));

    let start = game.start_payload();
    assert!(start
        .players
        .iter()
        .all(|player| player.faction_id == DEFAULT_FACTION_ID));
    assert!(game
        .player_inits()
        .iter()
        .all(|player| player.faction_id == DEFAULT_FACTION_ID));
}

fn spawned_resource_sites(map: &Map) -> EntityStore {
    let mut entities = EntityStore::new();
    for &start in &map.starts {
        spawn_base_resources(&mut entities, map, start);
    }
    for &site in &map.base_sites {
        if !map.starts.contains(&site) {
            spawn_base_resources(&mut entities, map, site);
        }
    }
    entities
}

#[test]
fn default_spawns_resources_for_every_base_site_with_one_player() {
    let players = [PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: DEFAULT_FACTION_ID.to_string(),
        name: "Solo".to_string(),
        color: "#cc1111".to_string(),
        is_ai: false,
    }];
    let game = Game::new(&players, 0x1020_3040);
    let base_count = game.state.map.base_sites.len();

    assert_eq!(
        game.state
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Steel)
            .count(),
        base_count * config::STEEL_PATCHES_PER_BASE as usize,
    );
    assert_eq!(
        game.state
            .entities
            .iter()
            .filter(|entity| entity.kind == EntityKind::Oil)
            .count(),
        base_count * config::OIL_PATCHES_PER_BASE as usize,
    );
}

#[test]
fn base_steel_patches_split_across_both_sides_of_city_centre() {
    let players = [
        (1, super::teams::normalize_team_id(1, 1)),
        (2, super::teams::normalize_team_id(2, 2)),
    ];
    let map = Map::generate_for_players(&players, 9);
    let start = map.starts[0];
    let (hx, hy) = map.tile_center(start.0, start.1);
    let center = map.world_size_px() * 0.5;
    let dir_x = center - hx;
    let dir_y = center - hy;
    let len = (dir_x * dir_x + dir_y * dir_y).sqrt();
    assert!(len > f32::EPSILON);
    let dir_x = dir_x / len;
    let dir_y = dir_y / len;

    let mut entities = EntityStore::new();
    spawn_base_resources(&mut entities, &map, start);

    let mut toward_center = 0;
    let mut away_from_center = 0;
    for steel in entities
        .iter()
        .filter(|entity| entity.kind == EntityKind::Steel)
    {
        let projection = (steel.pos_x - hx) * dir_x + (steel.pos_y - hy) * dir_y;
        if projection > config::TILE_SIZE as f32 {
            toward_center += 1;
        } else if projection < -(config::TILE_SIZE as f32) {
            away_from_center += 1;
        }
    }

    assert_eq!(toward_center, config::STEEL_PATCHES_PER_BASE.div_ceil(2));
    assert_eq!(away_from_center, config::STEEL_PATCHES_PER_BASE / 2);
}

#[test]
fn bundled_oil_patches_have_buildable_pump_jack_sites() {
    for available_map in Map::list_available() {
        for player_count in available_map.min_players..=available_map.max_players {
            for seed in 0..32 {
                let map = Map::load(&available_map.name, player_count as usize, seed)
                    .unwrap_or_else(|err| {
                        panic!(
                            "map {} should load for player_count={player_count} seed={seed}: {err}",
                            available_map.name
                        )
                    });
                let entities = spawned_resource_sites(&map);

                for oil in entities
                    .iter()
                    .filter(|entity| entity.kind == EntityKind::Oil)
                {
                    let (tile_x, tile_y) = map.tile_of(oil.pos_x, oil.pos_y);
                    assert!(
                        services::standability::building_site_clear(
                            &map,
                            &entities,
                            EntityKind::PumpJack,
                            tile_x,
                            tile_y,
                        ),
                        "oil node {} at tile ({tile_x}, {tile_y}) should leave a buildable Pump Jack site for map={} player_count={player_count} seed={seed}",
                        oil.id,
                        available_map.name
                    );
                }
            }
        }
    }
}
