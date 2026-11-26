use super::fixtures::*;
use super::*;
use crate::game::ability::AbilityKind;
use crate::game::services::shop_items;

fn fixture() -> Game {
    empty_flat_game(&[
        PlayerInit { id: 1, team_id: 1, faction_id: "cultivators".into(), name: "One".into(), color: "#fff".into(), is_ai: false },
        PlayerInit { id: 2, team_id: 1, faction_id: "kriegsia".into(), name: "Ally".into(), color: "#aaa".into(), is_ai: false },
        PlayerInit { id: 3, team_id: 3, faction_id: "kriegsia".into(), name: "Enemy".into(), color: "#f00".into(), is_ai: false },
    ])
}

#[test]
fn cosmic_energy_counts_only_actual_non_passive_insight_and_survives_restore() {
    let mut game = fixture();
    let id = game.state.entities.spawn_unit(1, EntityKind::MagicAdept, 500.0, 500.0).unwrap();
    let unit = game.state.entities.get_mut(id).unwrap();
    for _ in 0..config::DISCIPLE_PASSIVE_INSIGHT_TICKS { unit.tick_passive_insight(); }
    assert_eq!(unit.insight, 1);
    assert_eq!(unit.cosmic_energy_pending, 0);
    // Exact quarter-rate building carry; no energy until a whole insight is gained.
    unit.gain_insight(3, EntityKind::ResourceDepot);
    assert_eq!(unit.cosmic_energy_pending, 0);
    unit.gain_insight(1, EntityKind::ResourceDepot);
    unit.gain_insight(10, EntityKind::Rifleman);
    unit.gain_magic_insight(20);
    assert_eq!(unit.cosmic_energy_pending, 31);
    game.tick();
    assert_eq!(game.snapshot_for(1).cosmic_energy, 31);
    assert_eq!(game.snapshot_for(2).cosmic_energy, 0);
    assert_eq!(game.snapshot_for(3).cosmic_energy, 0);
    assert_eq!(game.state.entities.get(id).unwrap().cosmic_energy_pending, 0);
    let mut restored = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(), game.state.map.clone(), game.state.map_metadata.clone(),
    ).unwrap();
    restored.tick();
    assert_eq!(restored.snapshot_for(1).cosmic_energy, 31);
    let unit = restored.state.entities.get_mut(id).unwrap();
    unit.insight = 199;
    unit.gain_magic_insight(20);
    assert_eq!(unit.cosmic_energy_pending, 1);
    // Death must not discard income already earned during the tick.
    unit.apply_damage(u32::MAX, None);
    restored.tick();
    assert_eq!(restored.snapshot_for(1).cosmic_energy, 32);
    assert!(restored.state.entities.get(id).is_none());
}

#[test]
fn cosmic_energy_pending_survives_checkpoint_and_masters_earn_none() {
    let mut game = fixture();
    let id = game.state.entities.spawn_unit(1, EntityKind::WarAdept, 500.0, 500.0).unwrap();
    game.tick();
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.receive_shop_item(AbilityKind::DoubleXp);
    unit.gain_insight(7, EntityKind::Tank);
    assert_eq!(unit.cosmic_energy_pending, 28);
    let mut game = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(), game.state.map.clone(), game.state.map_metadata.clone(),
    ).unwrap();
    game.tick();
    assert_eq!(game.snapshot_for(1).cosmic_energy, 28);
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.insight = 200;
    unit.advance_dao_master();
    assert!(unit.dao_master);
    unit.gain_insight(100, EntityKind::Tank);
    unit.gain_non_passive_insight(60);
    assert_eq!(unit.cosmic_energy_pending, 0);
}

#[test]
fn breakthrough_aura_is_inclusive_friendly_non_stacking_and_rebuilt() {
    let mut game = fixture();
    let tile = config::TILE_SIZE as f32;
    let carrier = game.state.entities.spawn_unit(1, EntityKind::WarAdept, 500.0, 500.0).unwrap();
    let ally = game.state.entities.spawn_unit(2, EntityKind::Rifleman, 500.0 + 3.0 * tile, 500.0).unwrap();
    let outside = game.state.entities.spawn_unit(1, EntityKind::Rifleman, 500.0 + 3.0 * tile + 1.0, 500.0).unwrap();
    let enemy = game.state.entities.spawn_unit(3, EntityKind::Rifleman, 500.0, 510.0).unwrap();
    let second = game.state.entities.spawn_unit(1, EntityKind::WarAdept, 500.0, 500.0).unwrap();
    let base = game.state.entities.get(ally).unwrap().movement_speed();
    for id in [carrier, second] { game.state.entities.get_mut(id).unwrap().receive_shop_item(AbilityKind::BreakthroughTreasure); }
    let teams = game.team_relations();
    shop_items::refresh_breakthrough_auras(&mut game.state.entities, &teams);
    assert_eq!(game.state.entities.get(ally).unwrap().movement_speed(), base * 1.25);
    assert!(game.state.entities.get(carrier).unwrap().breakthrough_aura);
    for id in [outside, enemy] { assert!(!game.state.entities.get(id).unwrap().breakthrough_aura); }
    game.state.entities.get_mut(ally).unwrap().receive_shop_item(AbilityKind::Haste);
    assert!((game.state.entities.get(ally).unwrap().movement_speed() - base * 1.15 * 1.25).abs() < 0.00001);
    for id in [carrier, second] { game.state.entities.get_mut(id).unwrap().apply_damage(u32::MAX, None); }
    shop_items::refresh_breakthrough_auras(&mut game.state.entities, &teams);
    assert!(!game.state.entities.get(ally).unwrap().breakthrough_aura);
}
