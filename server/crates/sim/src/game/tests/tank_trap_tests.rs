use super::fixtures::*;
use super::*;

#[test]
fn ai_with_building_but_no_units_is_eliminated() {
    let players = human_vs_ai_players();
    let mut game = Game::new(&players, 0x1234_5678);
    let ai_units: Vec<u32> = game
        .state
        .entities
        .iter()
        .filter(|e| e.owner == 2 && e.is_unit())
        .map(|e| e.id)
        .collect();
    for id in ai_units {
        game.state.entities.remove(id);
    }

    assert!(
        !game.alive_players().contains(&2),
        "AI players have special elimination: no units means defeated"
    );
}

#[test]
fn tank_trap_does_not_keep_player_alive() {
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
    let mut game = Game::new(&players, 0x1234_5678);
    let p2_buildings: Vec<u32> = game
        .state
        .entities
        .iter()
        .filter(|entity| entity.owner == 2 && entity.is_building())
        .map(|entity| entity.id)
        .collect();
    for id in p2_buildings {
        game.state.entities.remove(id);
    }
    game.state
        .entities
        .spawn_building(2, EntityKind::TankTrap, 160.0, 160.0, true)
        .expect("Tank Trap should spawn");

    assert!(
        !game.alive_players().contains(&2),
        "Tank Traps are attackable buildings but not elimination-survival buildings"
    );
}

#[test]
fn pump_jack_does_not_keep_player_alive() {
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
    let mut game = Game::new(&players, 0x1234_5678);
    let p2_buildings: Vec<u32> = game
        .state
        .entities
        .iter()
        .filter(|entity| entity.owner == 2 && entity.is_building())
        .map(|entity| entity.id)
        .collect();
    for id in p2_buildings {
        game.state.entities.remove(id);
    }
    game.state
        .entities
        .spawn_building(2, EntityKind::PumpJack, 160.0, 160.0, true)
        .expect("Pump Jack should spawn");

    assert!(
        !game.alive_players().contains(&2),
        "Pump Jacks extract resources but are not elimination-survival buildings"
    );
}

#[test]
fn tank_trap_grants_no_local_sight() {
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
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    let x = (game.state.map.size - 2) as f32 * config::TILE_SIZE as f32;
    let y = x;

    assert!(
        !game.state.fog.is_visible_world(1, x, y),
        "fixture should place the far corner outside opening fog"
    );
    game.state
        .entities
        .spawn_building(1, EntityKind::TankTrap, x, y, true)
        .expect("Tank Trap should spawn");
    game.state
        .fog
        .recompute(&[1, 2], &game.state.entities, &game.state.map);

    assert!(
        !game.state.fog.is_visible_world(1, x, y),
        "Tank Traps should not reveal even their own tile"
    );
    assert!(
        !game
            .state
            .fog
            .is_visible_world(1, x - config::TILE_SIZE as f32, y),
        "Tank Traps should not reveal adjacent tiles"
    );
}

#[test]
fn tank_trap_can_be_damaged_and_removed_by_death_cleanup() {
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
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    let id = game
        .state
        .entities
        .spawn_building(2, EntityKind::TankTrap, 160.0, 160.0, true)
        .expect("Tank Trap should spawn");
    let hp = game
        .state
        .entities
        .get(id)
        .expect("Tank Trap should exist")
        .hp;

    game.state
        .entities
        .get_mut(id)
        .expect("Tank Trap should exist")
        .apply_damage(hp, Some((1, (160.0, 160.0), 2)));
    let teams = teams::TeamRelations::from_player_teams(
        game.state.players.iter().map(|p| (p.id, p.team_id)),
    );
    let mut events: HashMap<u32, Vec<Event>> = game
        .state
        .players
        .iter()
        .map(|player| (player.id, Vec::new()))
        .collect();
    services::death::death_system(
        &mut game.state.entities,
        &game.state.fog,
        &game.state.smokes,
        &teams,
        &mut game.state.players,
        &mut game.state.lingering_sight,
        &mut events,
        game.state.tick,
    );

    assert!(
        game.state.entities.get(id).is_none(),
        "death cleanup should remove destroyed Tank Traps"
    );
}
