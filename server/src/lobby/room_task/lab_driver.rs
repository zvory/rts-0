use crate::protocol::{kinds, Command};
use rts_sim::game::lab::LabCommandOptions;
use rts_sim::game::Game;

const SUPPLY_300_HELLHOLE_ID: &str = "supply-300-hellhole";
const LEG_TICKS: u32 = 900;
const TILE: f32 = 32.0;
const CENTER_TILE: f32 = 63.0;
const SHUTTLE_OFFSET_TILES: f32 = 18.0;

pub(super) fn lab_scenario_driver_for(scenario_id: &str) -> Option<LabScenarioDriver> {
    match scenario_id {
        SUPPLY_300_HELLHOLE_ID => Some(LabScenarioDriver::supply_300_hellhole()),
        _ => None,
    }
}

pub(super) struct LabScenarioDriver {
    shuttles: Vec<DiagonalShuttle>,
}

impl LabScenarioDriver {
    fn supply_300_hellhole() -> Self {
        Self {
            shuttles: vec![
                DiagonalShuttle {
                    player_id: 3,
                    endpoint_a: shuttle_endpoint(1.0, -1.0),
                    endpoint_b: shuttle_endpoint(-1.0, 1.0),
                    phase_offset_ticks: 0,
                    last_issued_phase: None,
                },
                DiagonalShuttle {
                    player_id: 4,
                    endpoint_a: shuttle_endpoint(-1.0, -1.0),
                    endpoint_b: shuttle_endpoint(1.0, 1.0),
                    phase_offset_ticks: 0,
                    last_issued_phase: None,
                },
            ],
        }
    }

    pub(super) fn enqueue_for_tick(&mut self, room: &str, game: &mut Game) {
        let tick = game.tick_count();
        for shuttle in &mut self.shuttles {
            shuttle.enqueue_for_tick(room, game, tick);
        }
    }
}

struct DiagonalShuttle {
    player_id: u32,
    endpoint_a: (f32, f32),
    endpoint_b: (f32, f32),
    phase_offset_ticks: u32,
    last_issued_phase: Option<u32>,
}

impl DiagonalShuttle {
    fn enqueue_for_tick(&mut self, room: &str, game: &mut Game, tick: u32) {
        let phase = self.phase_for_tick(tick);
        if self.last_issued_phase == Some(phase) {
            return;
        }
        self.last_issued_phase = Some(phase);
        let target = self.target_for_phase(phase);
        let snapshot = game.snapshot_full_for(self.player_id);
        let units: Vec<u32> = snapshot
            .entities
            .iter()
            .filter(|entity| entity.owner == self.player_id && unit_can_shuttle(&entity.kind))
            .map(|entity| entity.id)
            .collect();
        if units.is_empty() {
            return;
        }
        if let Err(err) = game.issue_lab_command_as(
            self.player_id,
            Command::Move {
                units,
                x: target.0,
                y: target.1,
                queued: false,
            },
            LabCommandOptions {
                ignore_command_limits: true,
            },
        ) {
            crate::log_warn!(
                room = %room,
                player_id = self.player_id,
                error = ?err,
                "lab scenario shuttle command rejected"
            );
        }
    }

    fn phase_for_tick(&self, tick: u32) -> u32 {
        tick.saturating_add(self.phase_offset_ticks) / LEG_TICKS
    }

    fn target_for_phase(&self, phase: u32) -> (f32, f32) {
        if phase % 2 == 0 {
            self.endpoint_b
        } else {
            self.endpoint_a
        }
    }
}

fn unit_can_shuttle(kind: &str) -> bool {
    matches!(
        kind,
        kinds::WORKER
            | kinds::GOLEM
            | kinds::RIFLEMAN
            | kinds::MACHINE_GUNNER
            | kinds::PANZERFAUST
            | kinds::ANTI_TANK_GUN
            | kinds::MORTAR_TEAM
            | kinds::ARTILLERY
            | kinds::SCOUT_CAR
            | kinds::TANK
            | kinds::COMMAND_CAR
            | kinds::EKAT
    )
}

fn shuttle_endpoint(x_dir: f32, y_dir: f32) -> (f32, f32) {
    (
        (CENTER_TILE + x_dir * SHUTTLE_OFFSET_TILES) * TILE,
        (CENTER_TILE + y_dir * SHUTTLE_OFFSET_TILES) * TILE,
    )
}
