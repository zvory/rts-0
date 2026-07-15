use super::*;

const SUPPORTED_TARGETS: [u32; 2] = [200, 300];
const UNIT_ORDER: [EntityKind; 10] = [
    EntityKind::Worker,
    EntityKind::Rifleman,
    EntityKind::MachineGunner,
    EntityKind::Panzerfaust,
    EntityKind::AntiTankGun,
    EntityKind::MortarTeam,
    EntityKind::Artillery,
    EntityKind::ScoutCar,
    EntityKind::Tank,
    EntityKind::CommandCar,
];
impl Game {
    pub fn new_supply_stress_scenario(
        target_supply: u32,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if !SUPPORTED_TARGETS.contains(&target_supply) {
            return Err(format!(
                "unsupported active supply-stress target {target_supply}; expected 200 or 300"
            ));
        }

        let teams = [(1, 1), (2, 2)];
        let map = Map::load_for_players("1v1", &teams, seed)?;
        let composition = supply_stress_composition(target_supply)?;
        let mut entities = EntityStore::new();
        for (player_index, player_id) in [1, 2].into_iter().enumerate() {
            let start = map
                .starts
                .get(player_index)
                .copied()
                .ok_or_else(|| format!("1v1 map is missing start {player_index}"))?;
            let (x, y) = map.tile_center(start.0, start.1);
            entities
                .spawn_building(player_id, EntityKind::CityCentre, x, y, true)
                .ok_or_else(|| format!("failed to spawn player {player_id} City Centre"))?;
        }

        let positions = standable_interleaved_positions(&map, &entities, composition.len() * 2)?;
        let mut units = Vec::with_capacity(composition.len());
        let mut position_index = 0;
        for kind in composition {
            for player_id in [1, 2] {
                let (x, y) = positions[position_index];
                position_index += 1;
                let id = entities
                    .spawn_unit(player_id, kind, x, y)
                    .ok_or_else(|| format!("failed to spawn player {player_id} {kind}"))?;
                if let Some(entity) = entities.get_mut(id) {
                    entity.set_invulnerable(true);
                }
                if player_id == 1 {
                    units.push(id);
                }
            }
        }

        let player_id = 1;
        let start_tile = map.starts.first().copied().unwrap_or((0, 0));
        let goal = map.tile_center(start_tile.0, start_tile.1);
        let mut game = build_dev_scenario_game_with_teams(
            map,
            entities,
            teams,
            player_id,
            start_tile,
            seed,
            &format!("dev:supply_stress_active:{target_supply}"),
        );
        game.state.lab_god_mode_players.extend([1, 2]);
        game.sync_lab_god_mode_flags();

        DevScenarioSetup {
            game,
            player_id,
            units,
            goal,
            issue_after_ticks: u32::MAX,
        }
        .checkpoint_backed(&format!("dev:supply_stress_active:{target_supply}"))
    }
}

fn supply_stress_composition(target_supply: u32) -> Result<Vec<EntityKind>, String> {
    let mut composition = Vec::new();
    let mut supply = 0;
    let mut skipped = 0;
    while supply < target_supply {
        let kind = UNIT_ORDER[composition.len().saturating_add(skipped) % UNIT_ORDER.len()];
        let cost = crate::rules::economy::supply_cost(kind);
        let next = supply.saturating_add(cost);
        if cost > 0 && next <= target_supply {
            composition.push(kind);
            supply = next;
            skipped = 0;
        } else {
            skipped += 1;
            if skipped >= UNIT_ORDER.len() {
                return Err(format!(
                    "cannot compose exact supply target {target_supply} from {supply}"
                ));
            }
        }
    }
    Ok(composition)
}

fn standable_interleaved_positions(
    map: &Map,
    entities: &EntityStore,
    count: usize,
) -> Result<Vec<(f32, f32)>, String> {
    let occupancy = crate::game::services::occupancy::Occupancy::build(map, entities);
    let center = map.world_size_px() * 0.5;
    let mut positions: Vec<_> = (1..map.size.saturating_sub(1))
        .step_by(2)
        .flat_map(|ty| {
            let occupancy = &occupancy;
            (1..map.size.saturating_sub(1))
                .step_by(2)
                .filter_map(move |tx| {
                    let position = map.tile_center(tx, ty);
                    UNIT_ORDER
                        .iter()
                        .all(|kind| {
                            crate::game::services::standability::unit_static_standable(
                                map, occupancy, *kind, position.0, position.1,
                            )
                        })
                        .then_some(position)
                })
        })
        .collect();
    positions.sort_by(|left, right| {
        let left_distance = (left.0 - center).powi(2) + (left.1 - center).powi(2);
        let right_distance = (right.0 - center).powi(2) + (right.1 - center).powi(2);
        left_distance
            .total_cmp(&right_distance)
            .then_with(|| left.1.total_cmp(&right.1))
            .then_with(|| left.0.total_cmp(&right.0))
    });
    if positions.len() < count {
        return Err(format!(
            "1v1 map has only {} shared standable stress positions; need {count}",
            positions.len()
        ));
    }
    positions.truncate(count);
    Ok(positions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn active_supply_stress_targets_are_exact_and_deterministic() {
        for (target, expected_entities, expected_projected, expected_counts) in [
            (200, 136, 135, [7, 7, 7, 7, 7, 7, 6, 7, 6, 6]),
            (300, 202, 201, [12, 10, 10, 10, 10, 10, 10, 10, 9, 9]),
        ] {
            let setup = Game::new_supply_stress_scenario(target, 0x5a00_0300)
                .expect("supported supply stress setup");
            let snapshot = setup.game.snapshot_full_for(1);
            assert_eq!(snapshot.entities.len(), expected_entities);
            let player_projection = setup.game.snapshot_for(1);
            assert_eq!(player_projection.entities.len(), expected_projected);
            assert!(player_projection.entities.iter().any(|entity| {
                entity.owner == 1 && entity.kind == EntityKind::CityCentre.to_string()
            }));
            assert!(!player_projection.entities.iter().any(|entity| {
                entity.owner == 2 && entity.kind == EntityKind::CityCentre.to_string()
            }));
            let resources: Vec<_> = snapshot
                .player_resources
                .iter()
                .map(|player| (player.id, player.supply_used, player.supply_cap))
                .collect();
            assert_eq!(
                resources,
                vec![
                    (1, target, config::PLAYER_SUPPLY_CAP),
                    (2, target, config::PLAYER_SUPPLY_CAP),
                ]
            );

            let mut by_owner = BTreeMap::<u32, BTreeMap<String, usize>>::new();
            for entity in snapshot.entities.iter().filter(|entity| entity.owner > 0) {
                *by_owner
                    .entry(entity.owner)
                    .or_default()
                    .entry(entity.kind.clone())
                    .or_default() += 1;
            }
            for owner in [1, 2] {
                let counts = by_owner.get(&owner).expect("both owners are projected");
                assert_eq!(counts.get("city_centre"), Some(&1));
                for (kind, expected) in UNIT_ORDER.into_iter().zip(expected_counts) {
                    assert_eq!(
                        counts.get(&kind.to_string()),
                        Some(&expected),
                        "{target} supply owner {owner} {kind}"
                    );
                }
            }
        }
    }

    #[test]
    fn active_supply_stress_rejects_unbounded_targets() {
        assert!(Game::new_supply_stress_scenario(199, 1).is_err());
        assert!(Game::new_supply_stress_scenario(301, 1).is_err());
    }
}
