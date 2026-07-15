use crate::protocol::{Command, LabReplayOperation, LabReplayOperationEntry};
use rts_sim::game::lab::LabCommandOptions;
use rts_sim::game::Game;

const SUPPLY_300_HELLHOLE_ID: &str = "supply-300-hellhole";
const LEG_TICKS: u32 = 900;
const TILE: f32 = 32.0;
const CENTER_TILE_X: f32 = 96.0;
const CENTER_TILE_Y: f32 = 63.0;
const COMBAT_OFFSET_TILES: f32 = 4.0;

pub(super) fn lab_scenario_driver_for(scenario_id: &str) -> Option<LabScenarioDriver> {
    (scenario_id == SUPPLY_300_HELLHOLE_ID).then(LabScenarioDriver::supply_300_hellhole)
}

pub(super) struct LabScenarioDriver {
    orders: [ScriptedCombatOrder; 2],
    last_issued_tick: Option<u32>,
}

impl LabScenarioDriver {
    fn supply_300_hellhole() -> Self {
        Self {
            orders: [
                ScriptedCombatOrder {
                    player_id: 1,
                    endpoint_a: combat_endpoint(-1.0),
                    endpoint_b: combat_endpoint(1.0),
                },
                ScriptedCombatOrder {
                    player_id: 2,
                    endpoint_a: combat_endpoint(1.0),
                    endpoint_b: combat_endpoint(-1.0),
                },
            ],
            last_issued_tick: None,
        }
    }

    pub(super) fn commands_for_tick(&mut self, game: &Game) -> Vec<LabScenarioCommand> {
        let tick = game.tick_count();
        if !self.commands_due_at(tick) {
            return Vec::new();
        }
        self.last_issued_tick = Some(tick);
        let phase = tick.saturating_div(LEG_TICKS);
        self.orders
            .iter()
            .filter_map(|order| order.command_for_phase(game, tick, phase))
            .collect()
    }

    pub(super) fn sync_to_tick(&mut self, tick: u32, entries: &[LabReplayOperationEntry]) {
        self.last_issued_tick = entries
            .iter()
            .filter(|entry| entry.tick <= tick)
            .filter(|entry| self.orders.iter().any(|order| order.matches(entry)))
            .map(|entry| entry.tick)
            .max();
    }

    fn commands_due_at(&self, tick: u32) -> bool {
        !self
            .last_issued_tick
            .is_some_and(|last_tick| tick.saturating_sub(last_tick) < LEG_TICKS)
    }
}

struct ScriptedCombatOrder {
    player_id: u32,
    endpoint_a: (f32, f32),
    endpoint_b: (f32, f32),
}

impl ScriptedCombatOrder {
    fn command_for_phase(&self, game: &Game, tick: u32, phase: u32) -> Option<LabScenarioCommand> {
        let units = game.lab_owned_unit_ids(self.player_id).ok()?;
        if units.is_empty() {
            return None;
        }
        let (x, y) = self.destination_for_phase(phase);
        Some(LabScenarioCommand {
            request_id: tick.saturating_add(1),
            player_id: self.player_id,
            command: Command::AttackMove {
                units,
                x,
                y,
                queued: false,
            },
            options: LabCommandOptions {
                ignore_command_limits: true,
            },
        })
    }

    fn matches(&self, entry: &LabReplayOperationEntry) -> bool {
        let phase = entry.tick.saturating_div(LEG_TICKS);
        let expected_destination = self.destination_for_phase(phase);
        entry.request_id == entry.tick.saturating_add(1)
            && matches!(
                &entry.op,
                LabReplayOperation::IssueCommandAs {
                    player_id,
                    cmd: Command::AttackMove { x, y, queued, .. },
                    ignore_command_limits,
                } if *player_id == self.player_id
                    && *ignore_command_limits
                    && !*queued
                    && (*x, *y) == expected_destination
            )
    }

    fn destination_for_phase(&self, phase: u32) -> (f32, f32) {
        if phase.is_multiple_of(2) {
            self.endpoint_b
        } else {
            self.endpoint_a
        }
    }
}

pub(super) struct LabScenarioCommand {
    pub(super) request_id: u32,
    pub(super) player_id: u32,
    pub(super) command: Command,
    pub(super) options: LabCommandOptions,
}

fn combat_endpoint(x_dir: f32) -> (f32, f32) {
    (
        (CENTER_TILE_X + x_dir * COMBAT_OFFSET_TILES) * TILE,
        CENTER_TILE_Y * TILE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seek_sync_preserves_scripted_order_cadence() {
        let mut driver = LabScenarioDriver::supply_300_hellhole();
        let mut entries = Vec::new();
        for tick in [1, 901] {
            for (sequence, order) in driver.orders.iter().enumerate() {
                let (x, y) = order.destination_for_phase(tick / LEG_TICKS);
                entries.push(LabReplayOperationEntry {
                    sequence: entries.len() as u64,
                    tick,
                    request_id: tick + 1,
                    operator_id: 99,
                    op: LabReplayOperation::IssueCommandAs {
                        player_id: order.player_id,
                        cmd: Command::AttackMove {
                            units: vec![sequence as u32 + 1],
                            x,
                            y,
                            queued: false,
                        },
                        ignore_command_limits: true,
                    },
                });
            }
        }
        entries.push(LabReplayOperationEntry {
            sequence: entries.len() as u64,
            tick: 400,
            request_id: 401,
            operator_id: 99,
            op: LabReplayOperation::IssueCommandAs {
                player_id: 1,
                cmd: Command::AttackMove {
                    units: vec![1],
                    x: 0.0,
                    y: 0.0,
                    queued: false,
                },
                ignore_command_limits: true,
            },
        });

        driver.sync_to_tick(500, &entries);

        assert_eq!(driver.last_issued_tick, Some(1));
        assert!(!driver.commands_due_at(900));
        assert!(driver.commands_due_at(901));
    }
}
