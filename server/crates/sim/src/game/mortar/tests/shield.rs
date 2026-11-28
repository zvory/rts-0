use super::*;

#[test]
fn disciple_shield_armors_mortar_and_rocket_hits_and_drains_full_damage() {
    let map = open_map(20);
    let inner = config::ROCKET_BARRAGE_INNER_RADIUS_TILES * config::TILE_SIZE as f32;
    let teams = TeamRelations::from_player_teams([(1, 1), (2, 2)]);
    for (rocket, offset, piercing) in [(false, 0.0, false), (true, 0.0, true), (true, 32.0, false)]
    {
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
                let mut firing_reveals = Vec::new();
                let shell = MortarShell {
                    double_damage: false,
                    owner: 1,
                    attacker: 0,
                    x: Coord::from_world(160.0),
                    y: Coord::from_world(160.0),
                    from_x: Coord::from_world(if inside { 160.0 } else { 100.0 }),
                    from_y: Coord::from_world(if inside { 160.0 } else { 100.0 }),
                    launch_tick: 0,
                    impact_tick: 0,
                    launched: true,
                    rocket,
                };
                resolve(
                    MortarResolutionContext {
                        entities: &mut entities,
                        teams: &teams,
                        fog: &fog,
                        map: &map,
                        events: &mut events,
                        firing_reveals: &mut firing_reveals,
                        tick: 10,
                    },
                    &shell,
                );
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
                let full_damage = shell_damage(rocket, offset * offset, inner * inner, piercing);
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
