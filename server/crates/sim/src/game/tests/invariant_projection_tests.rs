use super::*;
use crate::game::systems;
use crate::rules::faction::{catalog_for, DEFAULT_FACTION_ID};

#[test]
fn snapshot_target_invariant_accepts_projected_public_resource_target() {
    let players = [
        PlayerInit {
            id: 1,
            team_id: 1,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "A".into(),
            color: "#fff".into(),
            is_ai: false,
        },
        PlayerInit {
            id: 2,
            team_id: 2,
            faction_id: DEFAULT_FACTION_ID.to_string(),
            name: "B".into(),
            color: "#000".into(),
            is_ai: true,
        },
    ];
    let mut game = Game::new_for_replay(&players, 0x1234_5678);
    for tile in &mut game.state.map.terrain {
        *tile = terrain::GRASS;
    }
    for id in game.state.entities.ids() {
        game.state.entities.remove(id);
    }

    let catalog = catalog_for(DEFAULT_FACTION_ID).expect("default faction catalog");
    let observer_kind = *catalog.units.first().expect("catalog has a sight unit");
    let attacker_kind = catalog
        .units
        .iter()
        .copied()
        .find(|kind| config::unit_stats(*kind).is_some_and(|stats| stats.dmg > 0))
        .expect("catalog has a combat unit");
    let resource_kind = EntityKind::ALL
        .iter()
        .copied()
        .find(|kind| kind.is_node())
        .expect("entity catalog has a resource node");

    let observer_pos = game.state.map.tile_center(4, 4);
    game.state
        .entities
        .spawn_unit(1, observer_kind, observer_pos.0, observer_pos.1)
        .expect("observer should spawn");
    let attacker_pos = game.state.map.tile_center(6, 4);
    let attacker = game
        .state
        .entities
        .spawn_unit(2, attacker_kind, attacker_pos.0, attacker_pos.1)
        .expect("visible enemy attacker should spawn");
    let node_pos = game.state.map.tile_center(50, 50);
    let hidden_node = game
        .state
        .entities
        .spawn_node(resource_kind, node_pos.0, node_pos.1)
        .expect("hidden resource node should spawn");
    game.state
        .entities
        .get_mut(attacker)
        .expect("attacker should exist")
        .set_target_id(Some(hidden_node));

    systems::recompute_supply(&mut game.state.players, &game.state.entities);
    game.rebuild_final_spatial();
    let player_ids = game
        .state
        .players
        .iter()
        .map(|player| player.id)
        .collect::<Vec<_>>();
    game.state
        .fog
        .recompute(&player_ids, &game.state.entities, &game.state.map);

    assert!(!game.state.fog.is_visible_world(1, node_pos.0, node_pos.1));
    let snapshot = game.snapshot_for(1);
    assert_eq!(
        snapshot
            .entities
            .iter()
            .find(|entity| entity.id == attacker)
            .expect("attacker should project")
            .target_id,
        Some(hidden_node)
    );
    assert!(snapshot
        .entities
        .iter()
        .any(|entity| entity.id == hidden_node));
    game.assert_invariants();
}
