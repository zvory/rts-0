use super::*;

#[test]
fn lab_cultivator_spawns_set_owner_faction_and_initialize_each_dao() {
    use crate::protocol::LabSpawnDao;
    for (dao, upgrade) in [
        (LabSpawnDao::Wind, UpgradeKind::DaoWind),
        (LabSpawnDao::War, UpgradeKind::DaoOfHeaviness),
        (LabSpawnDao::Magic, UpgradeKind::DaoOfMagic),
    ] {
        let mut game = new_game();
        let (x, y) = free_unit_position(&game, EntityKind::Warrior);
        let outcome = game
            .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 2,
                kind: EntityKind::Warrior,
                x,
                y,
                completed: true,
                dao: Some(dao),
                dao_master: false,
                faction_id: Some("cultivators"),
            }))
            .expect("specialized spawn");
        let LabOpOutcome::Spawned { entity_id } = outcome else {
            panic!("spawn outcome")
        };
        let entity = game.state.entities.get(entity_id).expect("spawned entity");
        assert_eq!(entity.dao(), Some(upgrade));
        assert_eq!(entity.hp, entity.max_hp);
        assert_eq!(entity.dao_advance_ticks, 0);
        assert_eq!(
            game.player_inits()
                .iter()
                .find(|p| p.id == 2)
                .unwrap()
                .faction_id,
            "cultivators"
        );
        assert_eq!(
            game.snapshot_full_for(2)
                .player_factions
                .iter()
                .find(|p| p.id == 2)
                .unwrap()
                .faction_id,
            "cultivators"
        );
        // Seat identity is public even while viewing only the opposing team's private state.
        let snapshot = game.snapshot_for_observer(&crate::game::ObserverView::Players(vec![1]));
        assert!(snapshot.player_resources.iter().all(|p| p.id == 1));
        assert_eq!(
            snapshot
                .player_factions
                .iter()
                .find(|p| p.id == 2)
                .unwrap()
                .faction_id,
            "cultivators"
        );
        assert_eq!(
            game.snapshot_for(1)
                .player_factions
                .iter()
                .find(|p| p.id == 2)
                .unwrap()
                .faction_id,
            "cultivators"
        );
    }
}

#[test]
fn lab_spawns_all_dao_masters_with_promotion_stats_and_body() {
    use crate::game::ability::AbilityKind;

    for kind in [
        EntityKind::WindAdept,
        EntityKind::WarAdept,
        EntityKind::MagicAdept,
    ] {
        let mut game = new_game();
        let (x, y) = free_unit_position(&game, kind);
        let stats = config::unit_stats(kind).unwrap();
        game.state.entities.spawn_unit(1, kind, x, y).unwrap();

        let nearby_x = x + stats.radius * (1.5 + 0.5 * config::DAO_MASTER_BODY_SCALE);
        let mut spawn = LabSpawnEntity {
            owner: 2,
            kind,
            x: nearby_x,
            y,
            completed: true,
            dao: None,
            dao_master: true,
            faction_id: Some("cultivators"),
        };
        assert!(game
            .validate_unit_position(&game.state.entities, kind, nearby_x, y)
            .is_ok());
        let error = game.apply_lab_op(LabOp::SpawnEntity(spawn)).unwrap_err();
        assert!(matches!(error, LabError::Placement { blockers, .. } if !blockers.is_empty()));
        assert_eq!(game.player_inits()[1].faction_id, "kriegsia");

        spawn.x += 2.0 * config::TILE_SIZE as f32;
        let LabOpOutcome::Spawned { entity_id } =
            game.apply_lab_op(LabOp::SpawnEntity(spawn)).unwrap()
        else {
            panic!("spawn outcome");
        };
        let master = game.state.entities.get(entity_id).unwrap();
        assert!(master.dao_master);
        assert_eq!(master.kind, kind);
        assert_eq!(master.insight, 0);
        assert_eq!(master.dao_advance_ticks, 0);
        assert_eq!(master.hp, master.max_hp);
        assert_eq!(
            master.max_hp,
            (stats.hp as f32 * config::DAO_MASTER_HP_MULTIPLIER).round() as u32
        );
        if kind == EntityKind::WindAdept {
            assert_eq!(
                master.ability_uses_remaining(AbilityKind::WindDash),
                Some(2)
            );
        }
    }
}

#[test]
fn lab_invalid_dao_or_faction_spawn_is_atomic() {
    let mut game = new_game();
    let (x, y) = free_unit_position(&game, EntityKind::Worker);
    let count = game.state.entities.iter().count();
    for (kind, dao, faction_id) in [
        (
            EntityKind::Worker,
            Some(crate::protocol::LabSpawnDao::War),
            Some("cultivators"),
        ),
        (EntityKind::Rifleman, None, Some("cultivators")),
        (EntityKind::Worker, None, Some("missing")),
    ] {
        assert!(game
            .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
                owner: 2,
                kind,
                x,
                y,
                completed: true,
                dao,
                dao_master: false,
                faction_id,
            }))
            .is_err());
        assert_eq!(game.state.entities.iter().count(), count);
        assert_eq!(
            game.player_inits()
                .iter()
                .find(|p| p.id == 2)
                .unwrap()
                .faction_id,
            "kriegsia"
        );
    }
}

#[test]
fn lab_p2_base_novice_can_evolve_after_cultivator_spawn() {
    let mut game = new_game();
    let (x, y) = free_unit_position(&game, EntityKind::Warrior);
    let LabOpOutcome::Spawned { entity_id } = game
        .apply_lab_op(LabOp::SpawnEntity(LabSpawnEntity {
            owner: 2,
            kind: EntityKind::Warrior,
            x,
            y,
            completed: true,
            dao: None,
            dao_master: false,
            faction_id: Some("cultivators"),
        }))
        .unwrap()
    else {
        panic!("spawn outcome")
    };
    game.state.entities.get_mut(entity_id).unwrap().insight = config::DISCIPLE_INSIGHT_REQUIRED;
    game.issue_lab_command_as(
        2,
        Command::UseAbility {
            target_id: None,
            ability: "evolveMagic".to_string(),
            units: vec![entity_id],
            x: None,
            y: None,
            queued: false,
        },
        LabCommandOptions {
            ignore_command_limits: true,
        },
    )
    .unwrap();
    game.tick();
    assert_eq!(
        game.state.entities.get(entity_id).unwrap().dao(),
        Some(UpgradeKind::DaoOfMagic)
    );
}
