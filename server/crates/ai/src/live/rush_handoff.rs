//! One-time Rush opening handoff to the ordinary Pressure controller.
use super::*;

const RUSH_STEEL_LOSS_LIMIT: u32 = 1_000;

impl AiController {
    pub(super) fn handoff_rush(&mut self, context: &AiThinkContext<'_>) -> Vec<SimCommand> {
        if self.behavior_profile_id != RUSH_AI_ID {
            return Vec::new();
        }
        let engineer_alive = context
            .snapshot
            .entities
            .iter()
            .any(|e| e.owner == self.player && e.kind == "worker" && e.hp > 0);
        self.rush_strategy
            .observe_engineer(context.snapshot.tick, engineer_alive);
        let steel_lost = context
            .known_losses
            .get(&self.player)
            .map(crate::sdk::AiLosses::from_counts)
            .map_or(0, |losses| losses.steel_value);
        if steel_lost < RUSH_STEEL_LOSS_LIMIT
            && !self.rush_strategy.handoff_due(context.snapshot.tick)
        {
            return Vec::new();
        }
        let team = context
            .start
            .players
            .iter()
            .find(|p| p.id == self.player)
            .map(|p| p.team_id);
        let enemy = context
            .start
            .players
            .iter()
            .find(|p| Some(p.team_id) != team && context.alive_player_ids.contains(&p.id));
        let mut commands = Vec::new();
        if let Some(enemy) = enemy {
            let size = context.start.map.tile_size as f32;
            let target = (
                (enemy.start_tile_x as f32 + 0.5) * size,
                (enemy.start_tile_y as f32 + 0.5) * size,
            );
            let mut barracks: Vec<_> = context
                .snapshot
                .entities
                .iter()
                .filter(|e| e.owner == self.player && e.kind == "barracks" && e.hp > 0)
                .collect();
            barracks.sort_by(|a, b| {
                let distance =
                    |e: &rts_sim::protocol::EntityView| (e.x - target.0).hypot(e.y - target.1);
                distance(a)
                    .total_cmp(&distance(b))
                    .then_with(|| a.id.cmp(&b.id))
            });
            // Preserve one rear producer even when the opening lost a Barracks.
            let retire = barracks.len().saturating_sub(1).min(2);
            for barracks in barracks.into_iter().take(retire) {
                self.retired_barracks.insert(barracks.id);
                // Cancel clears all standing allocations and pops one queued item. Empty
                // producers still need one command to clear their repeat allocations.
                for _ in 0..barracks.prod_queue.unwrap_or(0).max(1) {
                    commands.push(SimCommand::Cancel {
                        building: barracks.id,
                        construction: false,
                    });
                }
            }
        }
        self.behavior_profile_id = PRESSURE_AI_ID;
        self.memory = AiDecisionMemory::for_profile(self.profile());
        self.rush_strategy = RushStrategy::default();
        self.custom_strategy = None;
        self.strategy_initialized = false;
        self.staged_units.clear();
        self.held_stage_units.clear();
        self.active_attack_units.clear();
        // Requested Rush/composition identity and pending construction survive the handoff.
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rts_sim::protocol::EntityView;
    const HANDOFF_TICK: u32 = 7_200;

    #[test]
    fn sparse_rush_opening_preserves_one_producer_and_composition_identity() {
        let players = super::super::tests::driver_test_players();
        let game = Game::new_without_ai_controllers(&players, 13);
        let start = game.start_payload();
        for count in 0..=2 {
            let mut snapshot = game.snapshot_for(1);
            snapshot.tick = HANDOFF_TICK;
            for id in 0..count {
                snapshot.entities.push(EntityView::new(
                    900 + id,
                    1,
                    "barracks",
                    500.0 + id as f32,
                    500.0,
                    500,
                    500,
                    "idle",
                ));
            }
            let composition = composition_by_id(AI_2_3_ID).unwrap();
            let seed = (0..128)
                .find(|seed| composition.select(*seed, 1) == Some(RUSH_AI_ID))
                .unwrap();
            let mut controller = AiController::new(1);
            controller.select_for_match(seed);
            controller
                .rush_strategy
                .set_handoff_tick_for_test(HANDOFF_TICK);
            controller.think(AiThinkContext {
                known_losses: Default::default(),
                match_seed: seed,
                start: &start,
                snapshot: &snapshot,
                alive_player_ids: &[1, 2],
                retreat_commands: vec![],
            });
            assert_eq!(controller.profile_id(), AI_2_3_ID);
            assert_eq!(controller.selected_profile_id(), Some(PRESSURE_AI_ID));
            assert_eq!(
                controller.retired_barracks.len(),
                count.saturating_sub(1) as usize
            );
        }
    }

    #[test]
    fn rush_hands_off_once_when_wave_delay_expires_and_retires_nearest_two() {
        let players = super::super::tests::driver_test_players();
        let game = Game::new_without_ai_controllers(&players, 13);
        let start = game.start_payload();
        let mut snapshot = game.snapshot_for(1);
        let enemy = start.players.iter().find(|p| p.id == 2).unwrap();
        let size = start.map.tile_size as f32;
        for (id, offset) in [(900, 500.0), (901, 100.0), (902, 200.0)] {
            let mut entity = EntityView::new(
                id,
                1,
                "barracks",
                enemy.start_tile_x as f32 * size + offset,
                enemy.start_tile_y as f32 * size,
                500,
                500,
                "idle",
            );
            entity.prod_queue = Some(3);
            snapshot.entities.push(entity);
        }
        let mut controller = AiController::with_profile_id(1, RUSH_AI_ID);
        let run = |controller: &mut AiController, snapshot: &Snapshot| {
            controller.think(AiThinkContext {
                known_losses: Default::default(),
                match_seed: 0,
                start: &start,
                snapshot,
                alive_player_ids: &[1, 2],
                retreat_commands: vec![],
            })
        };
        controller
            .rush_strategy
            .set_handoff_tick_for_test(HANDOFF_TICK);
        snapshot.tick = HANDOFF_TICK - 1;
        run(&mut controller, &snapshot);
        assert_eq!(controller.selected_profile_id(), Some(RUSH_AI_ID));
        snapshot.tick = HANDOFF_TICK;
        let commands = run(&mut controller, &snapshot);
        assert_eq!(controller.selected_profile_id(), Some(PRESSURE_AI_ID));
        assert_eq!(controller.profile_id(), RUSH_AI_ID);
        assert_eq!(controller.retired_barracks, BTreeSet::from([901, 902]));
        assert_eq!(
            commands
                .iter()
                .filter(|c| matches!(c, SimCommand::Cancel { .. }))
                .count(),
            6
        );
        snapshot.tick = HANDOFF_TICK + 8; // Pressure's next decision for player 1.
        let commands = run(&mut controller, &snapshot);
        assert!(!commands
            .iter()
            .any(|c| matches!(c, SimCommand::Cancel { .. })));
        assert!(!commands.iter().any(|c| matches!(c,
            SimCommand::AdjustProductionRepeat { buildings, delta, .. }
            if *delta > 0 && buildings.iter().any(|id| [901, 902].contains(id)))));
        assert!(controller.custom_strategy.is_none());
        assert!(!controller.rush_strategy.handoff_due(u32::MAX));
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;

    #[test]
    fn steel_losses_switch_without_waves_and_bypass_engineer_recovery_delay() {
        use rts_rules::EntityKind::{Barracks, Tank};

        let players = super::super::tests::driver_test_players();
        let game = Game::new_without_ai_controllers(&players, 13);
        let start = game.start_payload();
        for engineer_alive in [true, false] {
            let mut snapshot = game.snapshot_for(1);
            snapshot.tick = 100;
            if !engineer_alive {
                snapshot
                    .entities
                    .retain(|e| !(e.owner == 1 && e.kind == "worker"));
            }
            let mut controller = AiController::with_profile_id(1, RUSH_AI_ID);
            let run = |controller: &mut AiController, snapshot: &Snapshot, own_losses| {
                controller.think(AiThinkContext {
                    // Enemy losses cannot trigger our handoff.
                    known_losses: BTreeMap::from([
                        (1, own_losses),
                        (2, BTreeMap::from([(Tank, 100)])),
                    ]),
                    match_seed: 0,
                    start: &start,
                    snapshot,
                    alive_player_ids: &[1, 2],
                    retreat_commands: vec![],
                })
            };
            run(&mut controller, &snapshot, BTreeMap::new());
            assert_eq!(controller.selected_profile_id(), Some(RUSH_AI_ID));
            // 700 Steel plus 290 Oil is still below the Steel-only threshold.
            snapshot.tick += 1;
            run(&mut controller, &snapshot, BTreeMap::from([(Tank, 2)]));
            assert_eq!(controller.selected_profile_id(), Some(RUSH_AI_ID));
            // Unit and building losses together reach exactly 1,000 Steel.
            snapshot.tick += 1;
            run(
                &mut controller,
                &snapshot,
                BTreeMap::from([(Tank, 2), (Barracks, 2)]),
            );
            assert_eq!(controller.selected_profile_id(), Some(PRESSURE_AI_ID));
            assert_eq!(controller.profile_id(), RUSH_AI_ID);
            snapshot.tick += 1;
            run(
                &mut controller,
                &snapshot,
                BTreeMap::from([(Tank, 3), (Barracks, 1)]),
            );
            assert_eq!(controller.selected_profile_id(), Some(PRESSURE_AI_ID));
        }
    }

    #[test]
    fn clock_alone_does_not_switch_but_engineer_loss_recovers_without_five_waves() {
        let players = super::super::tests::driver_test_players();
        let game = Game::new_without_ai_controllers(&players, 13);
        let start = game.start_payload();
        let mut snapshot = game.snapshot_for(1);
        let mut controller = AiController::with_profile_id(1, RUSH_AI_ID);
        let run = |controller: &mut AiController, snapshot: &Snapshot| {
            controller.think(AiThinkContext {
                known_losses: Default::default(),
                match_seed: 0,
                start: &start,
                snapshot,
                alive_player_ids: &[1, 2],
                retreat_commands: vec![],
            })
        };
        snapshot.tick = 20_000;
        run(&mut controller, &snapshot);
        assert_eq!(controller.selected_profile_id(), Some(RUSH_AI_ID));
        assert_eq!(controller.opening.trained_riflemen(), 0);
        // Units trained during Rush count toward Pressure's research milestone after handoff.
        for id in 1000..1009 {
            snapshot.entities.push(rts_sim::protocol::EntityView::new(
                id, 1, "rifleman", 500.0, 500.0, 45, 45, "idle",
            ));
        }
        snapshot.tick += 1;
        run(&mut controller, &snapshot);
        assert_eq!(controller.opening.trained_riflemen(), 9);
        snapshot
            .entities
            .retain(|e| !(e.owner == 1 && e.kind == "worker"));
        snapshot.tick = 20_001;
        run(&mut controller, &snapshot);
        snapshot.tick += crate::config::TICK_HZ * 30 - 1;
        run(&mut controller, &snapshot);
        assert_eq!(controller.selected_profile_id(), Some(RUSH_AI_ID));
        snapshot.tick += 1;
        run(&mut controller, &snapshot);
        assert_eq!(controller.selected_profile_id(), Some(PRESSURE_AI_ID));
        assert_eq!(controller.opening.trained_riflemen(), 9);
    }
}
