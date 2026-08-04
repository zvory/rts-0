use super::*;
use crate::game::entity::{MovePhase, Order};

const WORKER_SPECS: [((f32, f32), (f32, f32)); 4] = [
    ((560.648, 597.900_15), (1328.0, 2320.0)),
    ((544.299_6, 605.429_75), (1168.0, 2352.0)),
    ((527.765_3, 601.773_56), (1200.0, 2288.0)),
    ((512.963_6, 591.553_3), (1136.0, 2384.0)),
];

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
        let mut entities = EntityStore::new();
        entities
            .spawn_unit(1, EntityKind::MachineGunner, 560.0, 528.0)
            .ok_or_else(|| "failed to spawn replay-256 Machine Gunner".to_string())?;
        let mut units = Vec::with_capacity(WORKER_SPECS.len());
        for (start, goal) in WORKER_SPECS {
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
    fn stalled_rally_workers_recover_and_reach_their_goals() {
        let mut setup = Game::new_replay_256_worker_expansion_rally_scenario(
            EntityKind::Worker,
            4,
            0x2560_0220,
        )
        .expect("scenario");
        for _ in 0..1_500 {
            setup.game.tick();
        }

        let snapshot = setup.game.snapshot_full_for(setup.player_id);
        for (index, id) in setup.units.iter().enumerate() {
            let worker = snapshot
                .entities
                .iter()
                .find(|entity| entity.id == *id)
                .expect("worker");
            let goal = WORKER_SPECS[index].1;
            let distance = (worker.x - goal.0).hypot(worker.y - goal.1);
            assert!(
                distance <= 16.0,
                "worker {id} remained {distance:.1}px from its goal"
            );
        }
    }
}
