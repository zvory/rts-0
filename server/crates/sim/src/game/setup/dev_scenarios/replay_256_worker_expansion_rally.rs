use super::*;
use crate::game::entity::{ProdItem, RallyIntent, RallyKind, WeaponSetup};

const DEPOT_POS: (f32, f32) = (304.0, 304.0);
const MACHINE_GUNNER_POS: (f32, f32) = (560.0, 528.0);
const RALLY: (f32, f32) = (1168.0, 2352.0);
const WORKER_BUILD_TICKS: u32 = 495;

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

        let mut map = Map::load("1v1", 1, seed)
            .map_err(|error| format!("failed to load replay-256 map: {error}"))?;
        let start_tile = (9, 9);
        if let Some(slot) = map.starts.get_mut(0) {
            *slot = start_tile;
        }
        // Replay 220 / match 256 reconstructed from the production source instead of its trapped
        // endpoint: the Resource Depot produces workers at its real cadence and assigns each one
        // a normal far-side rally order. The deployed Machine Gunner recreates the persistent
        // traffic field beside their shared opening route.
        let mut entities = EntityStore::new();
        let depot = entities
            .spawn_building(1, EntityKind::ResourceDepot, DEPOT_POS.0, DEPOT_POS.1, true)
            .ok_or_else(|| "failed to spawn replay-256 Resource Depot".to_string())?;
        let producer = entities
            .get_mut(depot)
            .ok_or_else(|| "spawned replay-256 Resource Depot is missing".to_string())?;
        for _ in 0..unit_count {
            producer.push_production(ProdItem {
                unit: EntityKind::Worker,
                progress: 0,
                total: WORKER_BUILD_TICKS,
                paid: true,
            });
        }
        producer.set_rally_point(Some(RallyIntent::new(RallyKind::Move, RALLY.0, RALLY.1)));

        let machine_gunner = entities
            .spawn_unit(
                1,
                EntityKind::MachineGunner,
                MACHINE_GUNNER_POS.0,
                MACHINE_GUNNER_POS.1,
            )
            .ok_or_else(|| "failed to spawn replay-256 Machine Gunner".to_string())?;
        entities
            .get_mut(machine_gunner)
            .ok_or_else(|| "spawned replay-256 Machine Gunner is missing".to_string())?
            .set_weapon_setup(WeaponSetup::Deployed);
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
            units: vec![machine_gunner],
            goal: RALLY,
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
    fn first_produced_worker_reaches_shared_route_waypoint_without_recovery() {
        let mut setup = Game::new_replay_256_worker_expansion_rally_scenario(
            EntityKind::Worker,
            4,
            0x2560_0220,
        )
        .expect("scenario");
        let route_waypoint = (528.0, 560.0);
        let mut first_worker_id = None;
        let mut closest = f32::MAX;

        for _ in 0..800 {
            setup.game.tick();
            let snapshot = setup.game.snapshot_full_for(setup.player_id);
            let first_worker = snapshot
                .entities
                .iter()
                .filter(|entity| entity.kind == "worker")
                .min_by_key(|entity| entity.id);
            if let Some(worker) = first_worker {
                first_worker_id = Some(worker.id);
                closest = closest.min(
                    (worker.x - route_waypoint.0).hypot(worker.y - route_waypoint.1),
                );
            }
        }

        let worker_id = first_worker_id.expect("the depot should produce its first Worker");
        assert!(
            closest <= config::ARRIVE_RADIUS_INTERMEDIATE_PX,
            "worker {worker_id} missed the shared route waypoint by {closest:.2}px"
        );
    }

    #[test]
    fn produced_rally_workers_reach_the_far_side() {
        let mut setup = Game::new_replay_256_worker_expansion_rally_scenario(
            EntityKind::Worker,
            4,
            0x2560_0220,
        )
        .expect("scenario");
        assert!(
            setup
                .game
                .snapshot_full_for(setup.player_id)
                .entities
                .iter()
                .all(|entity| entity.kind != "worker"),
            "the scenario must begin before any Worker is produced"
        );

        for _ in 0..4_000 {
            setup.game.tick();
        }

        let snapshot = setup.game.snapshot_full_for(setup.player_id);
        let workers: Vec<_> = snapshot
            .entities
            .iter()
            .filter(|entity| entity.kind == "worker")
            .collect();
        assert_eq!(workers.len(), 4, "the depot should produce all four workers");
        for worker in workers {
            let distance = (worker.x - RALLY.0).hypot(worker.y - RALLY.1);
            assert!(
                distance <= 64.0,
                "worker {} remained {distance:.1}px from the rally cluster",
                worker.id
            );
        }
    }
}
