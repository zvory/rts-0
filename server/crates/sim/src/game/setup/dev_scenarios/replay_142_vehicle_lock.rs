use super::*;

impl Game {
    pub fn new_replay_142_vehicle_lock_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::ScoutCar {
            return Err(format!("unsupported replay-142 lock unit {unit}"));
        }
        if unit_count != 2 {
            return Err(format!(
                "unsupported replay-142 lock unit count {unit_count}"
            ));
        }

        let mut map = flat_dev_map(1);
        let start_tile = (map.size / 2, map.size - 20);
        let anchor = map.tile_center(start_tile.0, start_tile.1);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = start_tile;
        }

        // Replay 112 / match 142, immediately before Soupman's tick-14,176 group order.
        // Positions are translated as one rigid formation so the original vehicle spacing,
        // facings, base landmark, and distant order bearing remain unchanged.
        let mut entities = EntityStore::new();
        entities
            .spawn_building(
                1,
                EntityKind::CityCentre,
                anchor.0 + 32.0,
                anchor.1 + 448.0,
                true,
            )
            .ok_or_else(|| "failed to spawn replay-142 City Centre".to_string())?;

        let specs = [
            (EntityKind::CommandCar, 0.0, 32.0, -2.343_303_7),
            (EntityKind::Tank, 19.576_172, -23.522_705, -2.452_928_5),
            (EntityKind::ScoutCar, 0.0, 0.0, -2.599_882_6),
            (EntityKind::ScoutCar, 64.0, 0.0, -2.699_532_3),
            (
                EntityKind::MachineGunner,
                29.971_436,
                134.346_19,
                -2.530_904_8,
            ),
        ];
        let mut units = Vec::with_capacity(specs.len());
        for (kind, dx, dy, facing) in specs {
            let id = entities
                .spawn_unit(1, kind, anchor.0 + dx, anchor.1 + dy)
                .ok_or_else(|| format!("failed to spawn replay-142 {kind}"))?;
            if let Some(entity) = entities.get_mut(id) {
                entity.set_facing(facing);
            }
            units.push(id);
        }

        let player_id = 1;
        let game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:replay_142_vehicle_lock",
        );

        DevScenarioSetup {
            game,
            player_id,
            units,
            goal: (anchor.0 + 1_015.082, anchor.1 - 1_880.655_8),
            issue_after_ticks: config::TICK_HZ,
            attack_move: false,
        }
        .checkpoint_backed("dev:replay_142_vehicle_lock")
    }
}
