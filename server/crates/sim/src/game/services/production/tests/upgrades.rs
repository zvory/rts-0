use super::*;
use crate::game::entity::PanzerfaustState;
#[test]
fn panzerfausts_research_unlocks_without_mutating_existing_riflemen() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let rifleman = entities
        .spawn_unit(1, EntityKind::Rifleman, 160.0, 160.0)
        .expect("rifleman should spawn");
    let (x, y) = footprint_center(&map, EntityKind::TrainingCentre, 10, 10);
    let training_centre = entities
        .spawn_building(1, EntityKind::TrainingCentre, x, y, true)
        .expect("training centre should spawn");
    entities
        .get_mut(training_centre)
        .expect("training centre")
        .push_research(ResearchItem {
            upgrade: UpgradeKind::Panzerfausts,
            progress: 1,
            total: 1,
            paid: true,
        });
    let mut players = vec![player(1)];

    tick_production(&map, &mut entities, &mut players);

    assert!(players[0].upgrades.contains(&UpgradeKind::Panzerfausts));
    assert_eq!(
        entities
            .get(rifleman)
            .and_then(|entity| entity.combat.as_ref())
            .and_then(|combat| combat.panzerfaust),
        None
    );
}

#[test]
fn panzerfaust_produced_after_research_spawns_loaded() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let barracks = spawn_building_training(
        &map,
        &mut entities,
        10,
        10,
        EntityKind::Barracks,
        EntityKind::Panzerfaust,
    );
    let mut players = vec![player(1)];
    players[0].upgrades.insert(UpgradeKind::Panzerfausts);

    tick_production(&map, &mut entities, &mut players);

    assert!(entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());
    let panzerfaust = entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Panzerfaust)
        .expect("panzerfaust should spawn");
    assert_eq!(
        panzerfaust
            .combat
            .as_ref()
            .and_then(|combat| combat.panzerfaust),
        Some(PanzerfaustState::Loaded)
    );
}

#[test]
fn riflemen_produced_after_panzerfausts_research_remain_unarmed() {
    let map = flat_map(24);
    let mut entities = EntityStore::new();
    let barracks = spawn_building_training(
        &map,
        &mut entities,
        10,
        10,
        EntityKind::Barracks,
        EntityKind::Rifleman,
    );
    let mut players = vec![player(1)];
    players[0].upgrades.insert(UpgradeKind::Panzerfausts);

    tick_production(&map, &mut entities, &mut players);

    assert!(entities
        .get(barracks)
        .expect("barracks")
        .prod_queue()
        .is_empty());
    let rifleman = entities
        .iter()
        .find(|entity| entity.owner == 1 && entity.kind == EntityKind::Rifleman)
        .expect("rifleman should spawn");
    assert_eq!(
        rifleman
            .combat
            .as_ref()
            .and_then(|combat| combat.panzerfaust),
        None
    );
}
