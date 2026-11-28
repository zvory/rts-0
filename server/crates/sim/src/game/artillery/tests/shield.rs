use super::*;

#[test]
fn disciple_shield_armors_artillery_and_drains_full_core_or_falloff() {
    let map = open_map(20);
    let inner = config::ARTILLERY_INNER_RADIUS_TILES * config::TILE_SIZE as f32;
    let outer = config::ARTILLERY_OUTER_RADIUS_TILES * config::TILE_SIZE as f32;
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    for (offset, piercing) in [(0.0, true), (32.0, false)] {
        for inside in [false, true] {
            let mut unshielded_damage = 0;
            for shielded in [false, true] {
                let mut entities = EntityStore::new();
                let victim = entities
                    .spawn_unit(2, EntityKind::Tank, 160.0 + offset, 160.0)
                    .unwrap();
                let caster = entities
                    .spawn_unit(2, EntityKind::MagicAdept, 400.0, 400.0)
                    .unwrap();
                if shielded {
                    entities
                        .get_mut(caster)
                        .unwrap()
                        .place_shield(1, 160.0, 160.0, 0, 150, true);
                }
                let hp = entities.get(victim).unwrap().hp;
                let fog = visible_team_fog(&map, &entities);
                let mut events = HashMap::from([(1, Vec::new()), (2, Vec::new())]);
                let shell = ArtilleryShell {
                    double_damage: false,
                    owner: 1,
                    attacker: 0,
                    from_x: Coord::from_world(if inside { 160.0 } else { 0.0 }),
                    from_y: Coord::from_world(if inside { 160.0 } else { 0.0 }),
                    x: Coord::from_world(160.0),
                    y: Coord::from_world(160.0),
                    impact_tick: 0,
                };
                resolve_shell(&map, &mut entities, &teams, &fog, &mut events, &shell, 10);
                let damage = hp - entities.get(victim).unwrap().hp;
                if !shielded {
                    assert!(damage > 0);
                    unshielded_damage = damage;
                    continue;
                }
                assert_eq!(
                    damage,
                    if inside {
                        unshielded_damage
                    } else {
                        ((unshielded_damage as f32) * if piercing { 1.0 } else { 0.0 }).round()
                            as u32
                    }
                );
                let full_damage = artillery_damage(offset * offset, inner * inner, outer * outer);
                assert_eq!(
                    entities
                        .get(caster)
                        .unwrap()
                        .shield
                        .as_ref()
                        .map_or(0, |s| s.remaining),
                    config::DISCIPLE_SHIELD_CAPACITY.saturating_sub(if inside {
                        0
                    } else {
                        full_damage
                    })
                );
            }
        }
    }
}
