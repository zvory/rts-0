use super::*;

#[test]
fn double_xp_purchase_delivery_checkpoint_and_privacy() {
    let (mut game, shop, unit) = item_fixture_for_kind(EntityKind::Warrior);
    buy(&mut game, shop, unit, AbilityKind::DoubleXp);
    game.tick();
    assert_eq!(game.state.players[0].oil, 965);
    assert!(!game
        .state
        .entities
        .get(unit)
        .unwrap()
        .shop_item_active(AbilityKind::DoubleXp));
    buy(&mut game, shop, unit, AbilityKind::DoubleXp);
    game.tick();
    assert_eq!(
        game.state.players[0].oil, 965,
        "in-flight duplicates spend nothing"
    );
    for _ in 0..42 {
        game.tick();
    }
    let carrier = game.state.entities.get(unit).unwrap();
    assert!(carrier.shop_item_active(AbilityKind::DoubleXp));
    assert_eq!(carrier.carried_item_count(), 1);
    assert_eq!(carrier.item_radiance_tier(), 1);
    assert!(game
        .snapshot_for(1)
        .entities
        .iter()
        .find(|e| e.id == unit)
        .unwrap()
        .abilities
        .iter()
        .any(|a| a.ability == "doubleXp" && a.remaining_uses == Some(1)));
    assert!(game
        .snapshot_for(2)
        .entities
        .iter()
        .all(|e| e.id != unit || e.abilities.iter().all(|a| a.ability != "doubleXp")));
    let mut game = Game::restore_checkpoint_payload_text_for_test(
        &game.checkpoint_payload_text_for_test().unwrap(),
        game.state.map.clone(),
        game.state.map_metadata.clone(),
    )
    .unwrap();
    game.state.entities.remove(shop);
    let carrier = game.state.entities.get_mut(unit).unwrap();
    carrier.gain_insight(7, EntityKind::Rifleman);
    assert_eq!(carrier.insight, 14, "restored item survives its Shop");
}

#[test]
fn double_xp_multiplies_new_rewards_before_fractional_carry_and_caps() {
    for kind in [
        EntityKind::Warrior,
        EntityKind::WindAdept,
        EntityKind::WarAdept,
        EntityKind::MagicAdept,
    ] {
        for target in [
            EntityKind::Rifleman,
            EntityKind::Tank,
            EntityKind::ResourceDepot,
            EntityKind::PumpJack,
        ] {
            let (mut game, _, id) = item_fixture_for_kind(kind);
            let unit = game.state.entities.get_mut(id).unwrap();
            unit.insight = 0;
            unit.insight_fraction = 0;
            // Keep the pre-purchase fraction at its original value.
            unit.gain_insight(1, EntityKind::ResourceDepot);
            let before = unit.insight;
            let fraction = unit.insight_fraction;
            unit.receive_shop_item(AbilityKind::DoubleXp);
            let (n, d) = config::disciple_insight_ratio(kind, target);
            unit.gain_insight(3, target);
            assert_eq!(unit.insight, before + ((6 * n + fraction) / d) as u32);
            assert_eq!(unit.insight_fraction, (6 * n + fraction) % d);
            unit.gain_insight(u32::MAX, target);
            assert_eq!(unit.insight, unit.insight_cap());
            assert_eq!(unit.insight_fraction, 0);
        }
    }
    let (mut game, _, id) = item_fixture_for_kind(EntityKind::MagicAdept);
    let unit = game.state.entities.get_mut(id).unwrap();
    unit.insight = 0;
    unit.receive_shop_item(AbilityKind::DoubleXp);
    unit.gain_magic_insight(20);
    assert_eq!(unit.insight, 40);
    unit.gain_magic_insight(u32::MAX);
    assert_eq!(unit.insight, 200);
    unit.dao_master = true;
    unit.insight = 0;
    unit.gain_magic_insight(20);
    unit.gain_insight(20, EntityKind::Tank);
    assert_eq!(unit.insight, 0);
}
