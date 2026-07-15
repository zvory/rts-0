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
    shuttles: [DiagonalShuttle; 2],
    initial_issued: bool,
    last_issued_tick: Option<u32>,
}

impl LabScenarioDriver {
    fn supply_300_hellhole() -> Self {
        Self {
            shuttles: [
                DiagonalShuttle {
                    player_id: 1,
                    endpoint_a: combat_endpoint(-1.0),
                    endpoint_b: combat_endpoint(1.0),
                },
                DiagonalShuttle {
                    player_id: 2,
                    endpoint_a: combat_endpoint(1.0),
                    endpoint_b: combat_endpoint(-1.0),
                },
            ],
            initial_issued: false,
            last_issued_tick: None,
        }
    }

    pub(super) fn commands_for_tick(&mut self, game: &Game) -> Vec<LabScenarioCommand> {
        let tick = game.tick_count();
        if self.initial_issued {
            let Some(last_tick) = self.last_issued_tick else {
                self.last_issued_tick = Some(tick);
                return Vec::new();
            };
            if tick.saturating_sub(last_tick) < LEG_TICKS {
                return Vec::new();
            }
        }
        self.initial_issued = true;
        self.last_issued_tick = Some(tick);
        let phase = tick.saturating_div(LEG_TICKS);
        self.shuttles
            .iter()
            .filter_map(|shuttle| shuttle.command_for_phase(game, tick, phase))
            .collect()
    }

    pub(super) fn sync_to_tick(&mut self, _tick: u32, entries: &[LabReplayOperationEntry]) {
        let recorded_at_tick = entries.iter().any(|entry| {
            matches!(
                &entry.op,
                LabReplayOperation::IssueCommandAs {
                    player_id: 1 | 2,
                    ..
                }
            )
        });
        self.initial_issued = recorded_at_tick;
        self.last_issued_tick = None;
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
