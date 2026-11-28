use super::*;

#[test]
fn meth_rifleman_post_shot_lock_preserves_new_move_orders_and_expires() {
    for (kind, meth, should_lock) in [
        (EntityKind::Rifleman, true, true),
        (EntityKind::BowNovice, false, true),
        (EntityKind::Rifleman, false, false),
        (EntityKind::Panzerfaust, true, false),
    ] {
        let map = open_map(20);
        let mut entities = EntityStore::new();
        let id = entities.spawn_unit(1, kind, 100.0, 100.0).unwrap();
        entities
            .spawn_building(2, EntityKind::Depot, 180.0, 100.0, true)
            .unwrap();
        // Use a spent Panzerfaust so this checks its ordinary rifle shot.
        if kind == EntityKind::Panzerfaust {
            entities
                .get_mut(id)
                .unwrap()
                .combat
                .as_mut()
                .unwrap()
                .panzerfaust = Some(crate::game::entity::PanzerfaustState::Spent);
        }
        let mut owner = player_state(1, false);
        if meth {
            owner.upgrades.insert(UpgradeKind::Methamphetamines);
        }
        let players = [owner, player_state(2, false)];
        run_combat_tick_on_map(&mut entities, &players, &map);
        assert!(entities.get(id).unwrap().attack_cd() > 0);
        let goal = (100.0, 400.0);
        let unit = entities.get_mut(id).unwrap();
        unit.set_order(Order::move_to(goal.0, goal.1));
        unit.set_path(vec![goal]);
        unit.set_path_goal(Some(goal));
        unit.mark_move_phase(MovePhase::Moving);
        // The durable timestamp survives checkpoint-style entity serialization.
        let bytes = serde_json::to_vec(unit).unwrap();
        let restored: crate::game::entity::Entity = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            restored.combat.as_ref().unwrap().rifle_post_shot_tick,
            should_lock.then_some(10)
        );
        for tick in 11..15 {
            run_movement_tick_on_map(&mut entities, &map, tick);
            let unit = entities.get(id).unwrap();
            if should_lock {
                assert_eq!((unit.pos_x(), unit.pos_y()), (100.0, 100.0));
                assert_eq!(unit.next_waypoint(), Some(goal));
            } else {
                assert!(unit.pos_y() > 100.0);
            }
        }
        run_movement_tick_on_map(&mut entities, &map, 15);
        assert!(entities.get(id).unwrap().pos_y() > 100.0);
        // Reload attempts must not refresh the lock timestamp.
        run_combat_tick_on_map_with_seed_smokes_and_tick(
            &mut entities,
            &players,
            &map,
            1,
            &SmokeCloudStore::new(),
            16,
        );
        assert_eq!(
            entities
                .get(id)
                .unwrap()
                .combat
                .as_ref()
                .unwrap()
                .rifle_post_shot_tick,
            should_lock.then_some(10)
        );
    }
}
