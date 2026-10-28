use super::*;

fn fixture(kind: EntityKind) -> (EntityStore, TeamRelations, u32, u32) {
    let mut entities = EntityStore::new();
    let node_kind = if kind == EntityKind::SteelMine {
        EntityKind::Steel
    } else {
        EntityKind::Oil
    };
    let node = entities.spawn_node(node_kind, 256.0, 256.0).unwrap();
    let extractor = entities.spawn_building(1, kind, 256.0, 256.0, true).unwrap();
    let teams = TeamRelations::from_player_teams([(1, 7), (2, 7), (3, 3)]);
    (entities, teams, extractor, node)
}

#[test]
fn unsupported_extractors_die_at_exactly_twenty_seconds_and_preserve_nodes() {
    for kind in [EntityKind::SteelMine, EntityKind::PumpJack] {
        let (mut entities, teams, id, node) = fixture(kind);
        let max_hp = entities.get(id).unwrap().max_hp;
        for elapsed in 1..=config::EXTRACTOR_DECAY_TICKS {
            assert!(tick(&mut entities, &teams).is_empty());
            let expected = max_hp - (u64::from(max_hp) * u64::from(elapsed)
                / u64::from(config::EXTRACTOR_DECAY_TICKS)) as u32;
            assert_eq!(entities.get(id).unwrap().hp, expected);
            if elapsed < config::EXTRACTOR_DECAY_TICKS {
                assert!(expected > 0);
            }
            // Checkpointed entity state must retain fractional damage across restores.
            if elapsed == 217 {
                let saved = serde_json::to_vec(&entities).unwrap();
                entities = serde_json::from_slice(&saved).unwrap();
            }
        }
        assert!(entities.get(node).unwrap().remaining().unwrap() > 0);
    }
}

#[test]
fn rebuilt_allied_base_stops_decay_without_healing_and_loss_restarts_decay() {
    for kind in [EntityKind::SteelMine, EntityKind::PumpJack] {
        for anchor_kind in [EntityKind::ResourceDepot, EntityKind::Zamok] {
            let (mut entities, teams, id, _) = fixture(kind);
            for _ in 0..config::EXTRACTOR_DECAY_TICKS / 2 {
                tick(&mut entities, &teams);
            }
            let damaged_hp = entities.get(id).unwrap().hp;
            let anchor = entities.spawn_building(2, anchor_kind, 192.0, 256.0, true).unwrap();
            for _ in 0..config::HARVEST_TICKS {
                tick(&mut entities, &teams);
            }
            assert_eq!(entities.get(id).unwrap().hp, damaged_hp);
            assert_eq!(entities.get(id).unwrap().resource_extractor.as_ref().unwrap().decay_remainder, 0);
            // A destroyed base provides no support even before the death system removes it.
            entities.get_mut(anchor).unwrap().apply_damage(u32::MAX, None);
            let max_hp = entities.get(id).unwrap().max_hp;
            let remaining_ticks = (damaged_hp * config::EXTRACTOR_DECAY_TICKS).div_ceil(max_hp);
            for _ in 0..remaining_ticks - 1 {
                tick(&mut entities, &teams);
            }
            assert!(entities.get(id).unwrap().hp > 0);
            tick(&mut entities, &teams);
            assert_eq!(entities.get(id).unwrap().hp, 0);
        }
    }
}

#[test]
fn unfinished_and_invulnerable_extractors_do_not_lose_hp() {
    for kind in [EntityKind::SteelMine, EntityKind::PumpJack] {
        let (mut entities, teams, id, _) = fixture(kind);
        entities.get_mut(id).unwrap().set_invulnerable(true);
        let unfinished = entities.spawn_building(1, kind, 320.0, 256.0, false).unwrap();
        let unfinished_hp = entities.get(unfinished).unwrap().hp;
        for _ in 0..config::EXTRACTOR_DECAY_TICKS {
            tick(&mut entities, &teams);
        }
        assert_eq!(entities.get(id).unwrap().hp, entities.get(id).unwrap().max_hp);
        assert_eq!(entities.get(unfinished).unwrap().hp, unfinished_hp);
    }
}
