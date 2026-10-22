use super::*;

#[test]
fn hidden_machine_gun_hits_reveal_shooter_and_feedback_to_victim_team() {
    for kind in [EntityKind::ResourceDepot, EntityKind::Rifleman] {
        let map = open_map(32);
        let mut entities = EntityStore::new();
        let attacker = entities
            .spawn_unit(1, EntityKind::MachineGunner, 100.0, 300.0)
            .unwrap();
        let victim = if kind == EntityKind::ResourceDepot {
            entities
                .spawn_building(2, kind, 280.0, 300.0, true)
                .unwrap()
        } else {
            entities.spawn_unit(2, kind, 280.0, 300.0).unwrap()
        };
        let mut grid = vec![false; 32 * 32];
        grid[9 * 32 + 8] = true;
        let mut fog = Fog::from_checkpoint_grids(
            32,
            32,
            BTreeMap::from([(2, grid.clone()), (3, grid)]),
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
        );
        let teams = TeamRelations::from_player_teams([(1, 1), (2, 2), (3, 2), (4, 4)]);
        let spatial = SpatialIndex::build(&entities, map.width, map.height);
        let mut events = HashMap::from([(1, vec![]), (2, vec![]), (3, vec![]), (4, vec![])]);
        let mut reveals = vec![];
        fire(
            &map,
            &mut entities,
            &teams,
            &spatial,
            &LineOfSight::new(&map),
            &fog,
            &SmokeCloudStore::new(),
            &mut SmallRng::seed_from_u64(1),
            &mut events,
            &mut reveals,
            attacker,
            victim,
            rules::weapon_profile(rules::WeaponKind::MachineGunnerMg).unwrap(),
            209.2,
            1,
        );
        for viewer in [2, 3] {
            assert!(
                events[&viewer].iter().any(|event| matches!(event,
                    Event::Attack { from, to, reveal: Some(_), shot_origin: Some(_), .. }
                    if *from == attacker && *to == victim
                )),
                "{kind:?}: victim team needs shooter reveal, tracer, and weapon audio event"
            );
            assert!(reveals
                .iter()
                .any(|source| source.viewer() == viewer && source.entity_id() == attacker));
            assert!(!fog.is_visible_world(viewer, 100.0, 300.0));
        }
        assert!(events[&2].iter().any(
            |event| matches!(event, Event::Notice { msg, .. } if msg == "alert:under_attack")
        ));
        assert!(events[&4].is_empty());
        fog.stamp_firing_reveal_sources_with_smoke(
            &reveals,
            &entities,
            &map,
            &SmokeCloudStore::new(),
        );
        for viewer in [2, 3] {
            assert!(fog.is_visible_world(viewer, 100.0, 300.0));
            let presentation = fog.presentation_union_for(viewer, &[viewer]);
            assert!(!presentation.is_visible_world(viewer, 100.0, 300.0));
            assert!(!fog.is_visible_world(viewer, 100.0, 332.0));
        }
    }
}
