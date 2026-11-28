use super::fixtures::*;
use super::*;
use crate::game::ability::AbilityKind;
use crate::game::ability_projectile::AbilityProjectileReturnTarget;

fn fixture() -> (Game, u32, Vec<u32>) {
    let mut game = empty_flat_game(&[PlayerInit {
        id: 1,
        team_id: 1,
        faction_id: "cultivators".into(),
        name: "Buyer".into(),
        color: "#fff".into(),
        is_ai: false,
    }]);
    game.state.players[0].set_resources(0, 2000);
    let shop = game
        .state
        .entities
        .spawn_building(1, EntityKind::Shop, 320.0, 320.0, true)
        .unwrap();
    let mut units = Vec::new();
    for (i, kind) in [
        EntityKind::Warrior,
        EntityKind::WarAdept,
        EntityKind::MagicAdept,
        EntityKind::WindAdept,
    ]
    .into_iter()
    .enumerate()
    {
        let id = game
            .state
            .entities
            .spawn_unit(1, kind, 640.0 + i as f32 * 64.0, 320.0)
            .unwrap();
        game.state.entities.get_mut(id).unwrap().insight = if i == 0 { 50 } else { 100 };
        units.push(id);
    }
    game.state.entities.get_mut(units[3]).unwrap().dao_master = true;
    game.state.entities.get_mut(units[3]).unwrap().insight = 200;
    game.rebuild_final_spatial();
    game.state
        .fog
        .recompute(&[1], &game.state.entities, &game.state.map);
    (game, shop, units)
}

fn buy(game: &mut Game, shop: u32, units: &[u32], ability: AbilityKind) {
    game.enqueue(
        1,
        Command::UseAbility {
            ability,
            units: std::iter::once(shop).chain(units.iter().copied()).collect(),
            x: None,
            y: None,
            target_id: None,
            queued: false,
        },
    );
}

fn recipients(game: &Game, ability: AbilityKind) -> Vec<u32> {
    let mut result: Vec<_> = game
        .state
        .ability_runtime
        .projectiles()
        .filter(|p| p.ability == ability)
        .filter_map(|p| match p.return_target {
            AbilityProjectileReturnTarget::Entity { id } => Some(id),
            _ => None,
        })
        .collect();
    result.sort_unstable();
    result
}

#[test]
fn buy_menu_prefers_rank_then_insight_then_existing_items() {
    let (mut game, shop, units) = fixture();
    // Neither a well-equipped Novice nor a high-insight Adept outranks a Master.
    game.state
        .entities
        .get_mut(units[0])
        .unwrap()
        .receive_shop_item(AbilityKind::ArmorTreasure);
    game.state.entities.get_mut(units[1]).unwrap().insight = 199;
    buy(&mut game, shop, &units, AbilityKind::Haste);
    game.tick();
    assert_eq!(recipients(&game, AbilityKind::Haste), vec![units[3]]);
    // Same-item deliveries reserve their recipient before arrival.
    buy(&mut game, shop, &units, AbilityKind::Haste);
    game.tick();
    assert_eq!(
        recipients(&game, AbilityKind::Haste),
        vec![units[1], units[3]]
    );
    // Equal rank and insight prefer the already-equipped unit.
    game.state.entities.get_mut(units[2]).unwrap().insight = 199;
    game.state
        .entities
        .get_mut(units[2])
        .unwrap()
        .receive_shop_item(AbilityKind::Regen);
    buy(&mut game, shop, &units[1..3], AbilityKind::DoubleDamage);
    game.tick();
    // The in-flight Haste counts as an item too; stable id breaks the tie.
    assert_eq!(recipients(&game, AbilityKind::DoubleDamage), vec![units[1]]);
    game.state
        .entities
        .get_mut(units[2])
        .unwrap()
        .receive_shop_item(AbilityKind::ArmorTreasure);
    buy(&mut game, shop, &units[1..3], AbilityKind::VitalityTreasure);
    game.tick();
    assert_eq!(
        recipients(&game, AbilityKind::VitalityTreasure),
        vec![units[1]]
    );
}

#[test]
fn rapid_buy_menu_purchases_skip_duplicates_and_full_carriers() {
    let (mut game, shop, units) = fixture();
    for _ in 0..5 {
        buy(&mut game, shop, &units, AbilityKind::Regen);
    }
    game.tick();
    assert_eq!(recipients(&game, AbilityKind::Regen), units);
    assert_eq!(
        game.state.players[0].oil, 1860,
        "only four eligible purchases are charged"
    );
    let target = units[1];
    buy(&mut game, shop, &[target], AbilityKind::Haste);
    buy(&mut game, shop, &[target], AbilityKind::ArmorTreasure);
    buy(&mut game, shop, &[target], AbilityKind::DoubleDamage);
    game.tick();
    assert!(
        recipients(&game, AbilityKind::DoubleDamage).is_empty(),
        "three in-flight goods fill an Adept"
    );
    assert_eq!(game.state.players[0].oil, 1720);
}

#[test]
fn buy_menu_escape_buys_once_and_never_activates_equipped_escape() {
    let (mut game, shop, units) = fixture();
    let master = units[3];
    game.state.entities.get_mut(master).unwrap().escape_treasure = Some(shop);
    buy(&mut game, shop, &[master], AbilityKind::EscapeTreasure);
    game.tick();
    assert_eq!(
        game.state.entities.get(master).unwrap().escape_treasure,
        Some(shop)
    );
    assert_eq!(game.state.players[0].oil, 2000);
    buy(&mut game, shop, &units, AbilityKind::EscapeTreasure);
    buy(&mut game, shop, &units, AbilityKind::EscapeTreasure);
    game.tick();
    assert_eq!(recipients(&game, AbilityKind::EscapeTreasure).len(), 1);
    assert_eq!(game.state.players[0].oil, 1930);
}

#[test]
fn buy_menu_rejects_serfs_foreign_units_and_stale_shops() {
    let (mut game, shop, units) = fixture();
    let serf = game
        .state
        .entities
        .spawn_unit(1, EntityKind::Worker, 900.0, 320.0)
        .unwrap();
    let foreign = game
        .state
        .entities
        .spawn_unit(2, EntityKind::Warrior, 960.0, 320.0)
        .unwrap();
    buy(
        &mut game,
        shop,
        &[serf, foreign, u32::MAX],
        AbilityKind::Regen,
    );
    game.tick();
    assert!(recipients(&game, AbilityKind::Regen).is_empty());
    let spare = game
        .state
        .entities
        .spawn_building(1, EntityKind::Shop, 1120.0, 320.0, true)
        .unwrap();
    game.state
        .entities
        .get_mut(units[3])
        .unwrap()
        .escape_treasure = Some(spare);
    game.state.entities.remove(shop);
    buy(&mut game, shop, &units, AbilityKind::EscapeTreasure);
    buy(&mut game, shop, &units, AbilityKind::Regen);
    game.tick();
    assert_eq!(game.state.players[0].oil, 2000);
    assert_eq!(
        game.state.entities.get(units[3]).unwrap().escape_treasure,
        Some(spare),
        "a stale purchase source must never manually activate an item from a different Shop"
    );
}

#[test]
fn buy_menu_concentrates_distinct_items_on_equipped_equal_rank_units() {
    let (mut game, shop, units) = fixture();
    let equipped = units[2];
    game.state
        .entities
        .get_mut(equipped)
        .unwrap()
        .receive_shop_item(AbilityKind::Regen);
    buy(&mut game, shop, &units[1..3], AbilityKind::Haste);
    game.tick();
    assert_eq!(recipients(&game, AbilityKind::Haste), vec![equipped]);
}
