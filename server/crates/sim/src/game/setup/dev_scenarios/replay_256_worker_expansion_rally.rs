use super::*;
use crate::game::entity::{MovePhase, Order};

impl Game {
    pub fn new_replay_256_worker_expansion_rally_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Worker || unit_count != 4 {
            return Err(format!(
                "replay-256 worker oscillation requires four workers, got {unit_count} {unit}"
            ));
        }

        let mut map = flat_dev_map(1);
        let start_tile = (17, 18);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = start_tile;
        }
        // Replay 220 / match 256 at tick 13,916 reduced to the five entities that preserve the
        // loop: four rallied workers, the stationary unit beside their stale next waypoint, and
        // each worker's distinct far-side rally destination.
        let specs = [
            ((560.648, 597.900_15), (1328.0, 2320.0)),
            ((544.299_6, 605.429_75), (1168.0, 2352.0)),
            ((527.765_3, 601.773_56), (1200.0, 2288.0)),
            ((512.963_6, 591.553_3), (1136.0, 2384.0)),
        ];
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::MachineGunner, 560.0, 528.0)
            .ok_or_else(|| "failed to spawn replay-256 Machine Gunner".to_string())?;
        let mut units = Vec::with_capacity(specs.len());
        for (start, goal) in specs {
            let id = entities
                .spawn_unit(1, EntityKind::Worker, start.0, start.1)
                .ok_or_else(|| "failed to spawn replay-256 Worker".to_string())?;
            if let Some(worker) = entities.get_mut(id) {
                worker.set_order(Order::move_to(goal.0, goal.1));
                worker.mark_move_phase(MovePhase::Moving);
                worker.set_path(vec![goal, (528.0, 560.0)]);
                worker.set_path_goal(Some(goal));
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
            "dev:replay_256_worker_expansion_rally",
        );

        DevScenarioSetup {
            game,
            player_id,
            units,
            goal: (545.0, 600.0),
            issue_after_ticks: u32::MAX,
            order: DevScenarioOrder::Move,
        }
        .checkpoint_backed("dev:replay_256_worker_expansion_rally")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_replay_layout_reproduces_four_worker_oscillation() {
        let mut setup = Game::new_replay_256_worker_expansion_rally_scenario(
            EntityKind::Worker,
            4,
            0x2560_0220,
        )
        .expect("scenario");
        let tracked = setup.units.clone();
        let lead_id = tracked[0];
        let mut previous: Option<(f32, f32)> = None;
        let mut previous_delta: Option<(f32, f32)> = None;
        let mut reversals = 0u32;
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        let mut distance = 0.0f32;
        let mut min_pocket_count = tracked.len();

        for _ in 0..=300 {
            let snapshot = setup.game.snapshot_full_for(setup.player_id);
            if let Some(lead) = snapshot.entities.iter().find(|entity| entity.id == lead_id) {
                min_x = min_x.min(lead.x);
                max_x = max_x.max(lead.x);
                min_y = min_y.min(lead.y);
                max_y = max_y.max(lead.y);
                if let Some((x, y)) = previous {
                    let delta = (lead.x - x, lead.y - y);
                    distance += delta.0.hypot(delta.1);
                    if let Some(last) = previous_delta {
                        if delta.0 * last.0 + delta.1 * last.1 < -0.01 {
                            reversals += 1;
                        }
                    }
                    if delta.0.abs() + delta.1.abs() > 0.001 {
                        previous_delta = Some(delta);
                    }
                }
                previous = Some((lead.x, lead.y));
            }
            let pocket_count = snapshot
                .entities
                .iter()
                .filter(|entity| tracked.contains(&entity.id))
                .filter(|entity| {
                    (entity.x - 545.0).abs() <= 50.0 && (entity.y - 600.0).abs() <= 50.0
                })
                .count();
            min_pocket_count = min_pocket_count.min(pocket_count);
            setup.game.tick();
        }

        let span = (max_x - min_x).hypot(max_y - min_y);
        assert_eq!(
            min_pocket_count, 4,
            "all four workers should remain in the pocket"
        );
        assert!(
            reversals >= 100,
            "lead worker only reversed {reversals} times"
        );
        assert!(
            distance >= 100.0,
            "lead worker only moved {distance:.1} pixels"
        );
        assert!(span <= 24.0, "lead worker escaped a {span:.1}-pixel span");
    }
}
