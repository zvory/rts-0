use super::*;

type Replay238Layout = (Map, (u32, u32), (f32, f32), (f32, f32));

impl Game {
    pub fn new_replay_238_rifleman_corner_lock_scenario(
        unit: EntityKind,
        unit_count: usize,
        seed: u32,
    ) -> Result<DevScenarioSetup, String> {
        if unit != EntityKind::Rifleman || unit_count != 1 {
            return Err(format!(
                "replay-238 corner lock requires one rifleman, got {unit_count} {unit}"
            ));
        }

        let (map, start_tile, start, goal) = replay_238_rifleman_corner_map();
        let mut entities = EntityStore::new();
        let rifleman = entities
            .spawn_unit(1, unit, start.0, start.1)
            .ok_or_else(|| "failed to spawn replay-238 Rifleman".to_string())?;
        if let Some(entity) = entities.get_mut(rifleman) {
            entity.set_facing(2.931_044_8);
        }
        let player_id = 1;
        let game = build_dev_scenario_game(
            map,
            entities,
            player_id,
            start_tile,
            seed,
            "dev:replay_238_rifleman_corner_lock",
        );

        DevScenarioSetup {
            game,
            player_id,
            units: vec![rifleman],
            goal,
            issue_after_ticks: 0,
            order: DevScenarioOrder::Move,
        }
        .checkpoint_backed("dev:replay_238_rifleman_corner_lock")
    }
}

/// Recreates the northeast rock corner around world tile (85, 10) from beta match 238 on
/// Schone Tage. The translated shape is intentionally exact around the Rifleman's contact point.
fn replay_238_rifleman_corner_map() -> Replay238Layout {
    let mut map = flat_dev_map(1);
    block_rect_tiles(&mut map, 83, 10, 85, 10);
    block_rect_tiles(&mut map, 81, 11, 87, 12);
    block_rect_tiles(&mut map, 80, 13, 88, 14);
    block_rect_tiles(&mut map, 81, 15, 87, 17);

    // Tick 2,915 replay evidence: Rifleman 165 was fixed at this point while repeatedly
    // repathing toward the adjacent grass tile at (82.5, 10.5).
    let start = (2755.293, 311.360);
    let goal = (2640.0, 336.0);
    let start_tile = (86, 9);
    if let Some(slot) = map.starts.get_mut(0) {
        *slot = start_tile;
    }
    (map, start_tile, start, goal)
}
