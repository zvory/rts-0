use super::*;
use crate::game::ability::AbilityKind;
use crate::game::entity::Order;
use crate::rules::faction::EMPTY_FIXTURE_FACTION_ID;

fn owned_kind_count(game: &Game, owner: u32, kind: EntityKind) -> usize {
    game.entities
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

    let fixture = game.players.iter().find(|p| p.id == 2).unwrap();
    assert_eq!(fixture.faction_id, EMPTY_FIXTURE_FACTION_ID);
    assert_eq!(fixture.steel, 125);
    assert_eq!(fixture.oil, 25);
    assert_eq!(fixture.supply_cap, config::DEPOT_SUPPLY);
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

    let resource_nodes = game.entities.iter().filter(|e| e.kind.is_node()).count();
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
        (
            game.players[0].steel,
            game.players[0].oil,
            game.players[0].supply_used,
            game.players[0].supply_cap
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(owned_kind_count(&game, 1, EntityKind::CityCentre), 0);
    let loadout = &game.starting_loadouts()[0];
    assert_eq!(loadout.loadout_id, "unknown_faction.invalid");
    assert_eq!((loadout.starting_steel, loadout.starting_oil), (0, 0));

    let (x, y) = game.map.tile_center(4, 4);
    let worker = game
        .entities
        .spawn_unit(1, EntityKind::Worker, x, y)
        .unwrap();
    let city_centre = game
        .entities
        .spawn_building(1, EntityKind::CityCentre, x + 96.0, y, true)
        .unwrap();
    let research_complex = game
        .entities
        .spawn_building(1, EntityKind::ResearchComplex, x + 192.0, y, true)
        .unwrap();
    let artillery = game
        .entities
        .spawn_unit(1, EntityKind::Artillery, x, y + 96.0)
        .unwrap();
    let node = game
        .entities
        .spawn_node(EntityKind::Steel, x + 320.0, y + 320.0)
        .unwrap();
    systems::recompute_supply(&mut game.players, &game.entities);
    let resources_before = (
        game.players[0].steel,
        game.players[0].oil,
        game.players[0].supply_used,
        game.players[0].supply_cap,
    );
    assert_eq!(resources_before, (0, 0, 0, 0));

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
            game.players[0].steel,
            game.players[0].oil,
            game.players[0].supply_used,
            game.players[0].supply_cap
        ),
        resources_before,
        "rejected unknown-faction commands must not spend resources or reserve supply"
    );
    assert!(!matches!(
        game.entities.get(worker).expect("worker").order(),
        Order::Build(_) | Order::Gather(_)
    ));
    assert!(game
        .entities
        .get(city_centre)
        .expect("city centre")
        .prod_queue()
        .is_empty());
    assert!(game
        .entities
        .get(research_complex)
        .expect("research complex")
        .research_queue()
        .is_empty());
    let artillery = game.entities.get(artillery).expect("artillery");
    assert!(!matches!(artillery.order(), Order::Ability(_)));
    assert_eq!(artillery.ability_cooldown_ticks(AbilityKind::PointFire), 0);
}

#[test]
fn standard_starting_loadout_matches_phase0_inventory() {
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

    for player in &game.players {
        assert_eq!(player.faction_id, DEFAULT_FACTION_ID);
        assert_eq!(player.steel, config::STARTING_STEEL);
        assert_eq!(player.oil, config::STARTING_OIL);
        assert_eq!(player.supply_cap, config::CITY_CENTRE_SUPPLY);
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
        .entities
        .iter()
        .any(|e| e.owner == 0 && e.kind == EntityKind::Steel));
    assert!(game
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
        spawn_base_resources(&mut entities, &map, start);
    }
    for &site in &map.expansion_sites {
        if !map.starts.contains(&site) {
            spawn_base_resources(&mut entities, &map, site);
        }
    }
    entities
}

#[test]
fn generated_default_oil_patches_have_buildable_pump_jack_sites() {
    for player_count in 1..=4 {
        for seed in 0..32 {
            let map = Map::generate(player_count, seed);
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
                    "oil node {} at tile ({tile_x}, {tile_y}) should leave a buildable Pump Jack site for player_count={player_count} seed={seed}",
                    oil.id
                );
            }
        }
    }
}
