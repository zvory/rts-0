use crate::protocol::{Command, LabReplayOperation, LabReplayOperationEntry};
use rts_sim::game::lab::LabCommandOptions;
use rts_sim::game::Game;

const SUPPLY_300_HELLHOLE_ID: &str = "supply-300-hellhole";
const LEG_TICKS: u32 = 900;
const TILE: f32 = 32.0;
const CENTER_TILE: f32 = 63.0;
const SHUTTLE_OFFSET_TILES: f32 = 18.0;

pub(crate) fn lab_scenario_driver_for(scenario_id: &str) -> Option<LabScenarioDriver> {
    (scenario_id == SUPPLY_300_HELLHOLE_ID).then(LabScenarioDriver::supply_300_hellhole)
}

pub(crate) struct LabScenarioDriver {
    shuttles: [DiagonalShuttle; 2],
    last_issued_tick: Option<u32>,
}

impl LabScenarioDriver {
    fn supply_300_hellhole() -> Self {
        Self {
            shuttles: [
                DiagonalShuttle {
                    player_id: 3,
                    endpoint_a: shuttle_endpoint(1.0, -1.0),
                    endpoint_b: shuttle_endpoint(-1.0, 1.0),
                },
                DiagonalShuttle {
                    player_id: 4,
                    endpoint_a: shuttle_endpoint(-1.0, -1.0),
                    endpoint_b: shuttle_endpoint(1.0, 1.0),
                },
            ],
            last_issued_tick: None,
        }
    }

    pub(crate) fn commands_for_tick(&mut self, game: &Game) -> Vec<LabScenarioCommand> {
        let tick = game.tick_count();
        if !tick.is_multiple_of(LEG_TICKS) || self.last_issued_tick == Some(tick) {
            return Vec::new();
        }
        self.last_issued_tick = Some(tick);
        let phase = tick / LEG_TICKS;
        self.shuttles
            .iter()
            .filter_map(|shuttle| shuttle.command_for_phase(game, tick, phase))
            .collect()
    }

    pub(super) fn sync_to_tick(&mut self, tick: u32, entries: &[LabReplayOperationEntry]) {
        let recorded_at_tick = entries.iter().any(|entry| {
            entry.tick == tick && self.shuttles.iter().any(|shuttle| shuttle.matches(entry))
        });
        self.last_issued_tick = (!tick.is_multiple_of(LEG_TICKS) || recorded_at_tick)
            .then_some(tick - (tick % LEG_TICKS));
    }
}

struct DiagonalShuttle {
    player_id: u32,
    endpoint_a: (f32, f32),
    endpoint_b: (f32, f32),
}

impl DiagonalShuttle {
    fn command_for_phase(&self, game: &Game, tick: u32, phase: u32) -> Option<LabScenarioCommand> {
        let units = game.lab_owned_unit_ids(self.player_id).ok()?;
        if units.is_empty() {
            return None;
        }
        let (x, y) = self.destination_for_phase(phase);
        Some(LabScenarioCommand {
            request_id: tick.saturating_add(1),
            player_id: self.player_id,
            command: Command::Move {
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
        let expected_destination = self.destination_for_phase(entry.tick / LEG_TICKS);
        entry.request_id == entry.tick.saturating_add(1)
            && matches!(
                &entry.op,
                LabReplayOperation::IssueCommandAs {
                    player_id,
                    cmd: Command::Move { x, y, queued, .. },
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

pub(crate) struct LabScenarioCommand {
    pub(crate) request_id: u32,
    pub(crate) player_id: u32,
    pub(crate) command: Command,
    pub(crate) options: LabCommandOptions,
}

fn shuttle_endpoint(x_dir: f32, y_dir: f32) -> (f32, f32) {
    (
        (CENTER_TILE + x_dir * SHUTTLE_OFFSET_TILES) * TILE,
        (CENTER_TILE + y_dir * SHUTTLE_OFFSET_TILES) * TILE,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn move_entry(player_id: u32, tick: u32, destination: (f32, f32)) -> LabReplayOperationEntry {
        LabReplayOperationEntry {
            sequence: 0,
            tick,
            request_id: tick + 1,
            operator_id: 99,
            op: LabReplayOperation::IssueCommandAs {
                player_id,
                cmd: Command::Move {
                    units: vec![1],
                    x: destination.0,
                    y: destination.1,
                    queued: false,
                },
                ignore_command_limits: true,
            },
        }
    }

    #[test]
    fn seek_sync_only_recognizes_the_scripted_shuttle_command() {
        let mut driver = LabScenarioDriver::supply_300_hellhole();
        let scripted_destination = driver.shuttles[0].destination_for_phase(1);
        let mut user_entry = move_entry(3, LEG_TICKS, (0.0, 0.0));

        driver.sync_to_tick(LEG_TICKS, std::slice::from_ref(&user_entry));
        assert_eq!(driver.last_issued_tick, None);

        user_entry = move_entry(3, LEG_TICKS, scripted_destination);
        driver.sync_to_tick(LEG_TICKS, std::slice::from_ref(&user_entry));
        assert_eq!(driver.last_issued_tick, Some(LEG_TICKS));

        user_entry.request_id += 1;
        driver.sync_to_tick(LEG_TICKS, &[user_entry]);
        assert_eq!(driver.last_issued_tick, None);
    }
}
