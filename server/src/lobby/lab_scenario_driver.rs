use crate::protocol::{Command, LabReplayOperation, LabReplayOperationEntry};
use rts_sim::game::lab::LabCommandOptions;
use rts_sim::game::Game;

const SUPPLY_300_HELLHOLE_ID: &str = "supply-300-hellhole";
const LEG_TICKS: u32 = 900;
const TILE: f32 = 32.0;
const CENTER_TILE: f32 = 63.0;
const SHUTTLE_OFFSET_TILES: f32 = 18.0;

pub(super) fn lab_scenario_driver_for(scenario_id: &str) -> Option<LabScenarioDriver> {
    (scenario_id == SUPPLY_300_HELLHOLE_ID).then(LabScenarioDriver::supply_300_hellhole)
}

pub(super) struct LabScenarioDriver {
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

    pub(super) fn commands_for_tick(&mut self, game: &Game) -> Vec<LabScenarioCommand> {
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
            entry.tick == tick
                && entry.request_id == tick.saturating_add(1)
                && matches!(
                    &entry.op,
                    LabReplayOperation::IssueCommandAs {
                        player_id: 3 | 4,
                        ..
                    }
                )
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
        let (x, y) = if phase.is_multiple_of(2) {
            self.endpoint_b
        } else {
            self.endpoint_a
        };
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
}

pub(super) struct LabScenarioCommand {
    pub(super) request_id: u32,
    pub(super) player_id: u32,
    pub(super) command: Command,
    pub(super) options: LabCommandOptions,
}

fn shuttle_endpoint(x_dir: f32, y_dir: f32) -> (f32, f32) {
    (
        (CENTER_TILE + x_dir * SHUTTLE_OFFSET_TILES) * TILE,
        (CENTER_TILE + y_dir * SHUTTLE_OFFSET_TILES) * TILE,
    )
}
