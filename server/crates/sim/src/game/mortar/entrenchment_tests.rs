use super::*;
use crate::game::map::Map;
use crate::protocol::terrain;
use std::collections::HashMap;

fn open_map(size: u32) -> Map {
    Map {
        size,
        terrain: vec![terrain::GRASS; (size * size) as usize],
        starts: vec![(4, 4), (size - 5, size - 5)],
        base_sites: Vec::new(),
    }
}

fn visible_team_fog(map: &Map, entities: &EntityStore) -> Fog {
    let mut fog = Fog::new(map.size);
    fog.recompute(&[1, 2, 3], entities, map);
    fog
}

fn mark_entrenched(entities: &mut EntityStore, id: u32) {
    entities
        .get_mut(id)
        .expect("entity should exist")
        .movement
        .as_mut()
        .expect("entity should have movement")
        .occupied_trench_id = Some(1);
}

#[test]
fn mortar_outer_area_damage_is_reduced_against_entrenched_infantry() {
    let map = open_map(20);
    let mut entities = EntityStore::new();
    let attacker = entities
        .spawn_unit(1, EntityKind::MortarTeam, 100.0, 100.0)
        .expect("mortar should spawn");
    let victim = entities
        .spawn_unit(2, EntityKind::Rifleman, 200.0, 160.0)
        .expect("victim should spawn");
    mark_entrenched(&mut entities, victim);
    let before = entities.get(victim).expect("victim should exist").hp;
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    let fog = visible_team_fog(&map, &entities);
    let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);
    let mut firing_reveals = Vec::new();
    let shell = MortarShell {
        owner: 1,
        attacker,
        x: 160.0,
        y: 160.0,
        impact_tick: 0,
    };

    resolve(
        &mut entities,
        &teams,
        &fog,
        &mut events,
        &mut firing_reveals,
        &shell,
        10,
    );

    let after = entities.get(victim).expect("victim should survive").hp;
    assert_eq!(
        before - after,
        30,
        "entrenched infantry should take 75% of outer mortar splash"
    );
}
