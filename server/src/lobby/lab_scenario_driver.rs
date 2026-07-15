use super::lab_replay_operations::lab_op_to_replay_operation;
use crate::protocol::{Command, LabReplayOperation, LabReplayOperationEntry};
use rts_sim::game::lab::{LabCommandOptions, LabOp};
use rts_sim::game::Game;

const SUPPLY_300_HELLHOLE_ID: &str = "supply-300-hellhole";
const LEG_TICKS: u32 = 900;
const TILE: f32 = 32.0;
const CENTER_TILE: f32 = 63.0;
const SHUTTLE_OFFSET_TILES: f32 = 18.0;
const MAX_ACTIONS_PER_TICK: usize = 16;

pub(crate) fn lab_scenario_driver_for(scenario_id: &str) -> Option<LabScenarioDriver> {
    (scenario_id == SUPPLY_300_HELLHOLE_ID).then(LabScenarioDriver::supply_300_hellhole)
}

pub(crate) struct LabScenarioDriver {
    shuttles: Vec<DiagonalShuttle>,
    scheduled_actions: Vec<ScheduledAction>,
    last_processed_tick: Option<u32>,
    retained_entries_at_tick: Vec<LabReplayOperationEntry>,
}

impl LabScenarioDriver {
    fn supply_300_hellhole() -> Self {
        Self {
            shuttles: vec![
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
            scheduled_actions: Vec::new(),
            last_processed_tick: None,
            retained_entries_at_tick: Vec::new(),
        }
    }

    pub(crate) fn actions_for_tick(&mut self, game: &Game) -> Vec<LabScenarioAction> {
        let tick = game.tick_count();
        if self.last_processed_tick == Some(tick) {
            return Vec::new();
        }
        self.last_processed_tick = Some(tick);

        let mut actions: Vec<_> = self
            .scheduled_actions
            .iter()
            .filter(|scheduled| scheduled.tick == tick)
            .map(|scheduled| scheduled.action.clone())
            .collect();
        if tick.is_multiple_of(LEG_TICKS) {
            let phase = tick / LEG_TICKS;
            actions.extend(
                self.shuttles
                    .iter()
                    .filter_map(|shuttle| shuttle.command_for_phase(game, tick, phase))
                    .map(LabScenarioAction::Command),
            );
        }
        actions.retain(|action| {
            !self
                .retained_entries_at_tick
                .iter()
                .any(|entry| action.matches_replay_entry(entry))
        });
        self.retained_entries_at_tick.clear();
        actions.truncate(MAX_ACTIONS_PER_TICK);
        actions
    }

    pub(super) fn sync_to_tick(&mut self, tick: u32, entries: &[LabReplayOperationEntry]) {
        self.last_processed_tick = None;
        self.retained_entries_at_tick = entries
            .iter()
            .filter(|entry| entry.tick == tick)
            .cloned()
            .collect();
    }

    #[cfg(test)]
    pub(crate) fn scripted_for_test(tick: u32, action: LabScenarioAction) -> Self {
        Self {
            shuttles: Vec::new(),
            scheduled_actions: vec![ScheduledAction { tick, action }],
            last_processed_tick: None,
            retained_entries_at_tick: Vec::new(),
        }
    }
}

struct ScheduledAction {
    tick: u32,
    action: LabScenarioAction,
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

    fn destination_for_phase(&self, phase: u32) -> (f32, f32) {
        if phase.is_multiple_of(2) {
            self.endpoint_b
        } else {
            self.endpoint_a
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) enum LabScenarioAction {
    Command(LabScenarioCommand),
    LabOperation { request_id: u32, op: LabOp },
}

impl LabScenarioAction {
    fn matches_replay_entry(&self, entry: &LabReplayOperationEntry) -> bool {
        match self {
            Self::Command(command) => {
                entry.request_id == command.request_id
                    && entry.op
                        == (LabReplayOperation::IssueCommandAs {
                            player_id: command.player_id,
                            cmd: command.command.clone(),
                            ignore_command_limits: command.options.ignore_command_limits,
                        })
            }
            Self::LabOperation { request_id, op } => {
                entry.request_id == *request_id
                    && lab_op_to_replay_operation(op).is_some_and(|replay_op| entry.op == replay_op)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
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
    use rts_sim::game::entity::EntityKind;
    use rts_sim::game::lab::LabSpawnEntity;

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
    fn replay_matching_only_recognizes_the_exact_scripted_shuttle_command() {
        let driver = LabScenarioDriver::supply_300_hellhole();
        let scripted_destination = driver.shuttles[0].destination_for_phase(1);
        let mut user_entry = move_entry(3, LEG_TICKS, (0.0, 0.0));
        let action = LabScenarioAction::Command(LabScenarioCommand {
            request_id: LEG_TICKS + 1,
            player_id: 3,
            command: Command::Move {
                units: vec![1],
                x: scripted_destination.0,
                y: scripted_destination.1,
                queued: false,
            },
            options: LabCommandOptions {
                ignore_command_limits: true,
            },
        });

        assert!(!action.matches_replay_entry(&user_entry));

        user_entry = move_entry(3, LEG_TICKS, scripted_destination);
        assert!(action.matches_replay_entry(&user_entry));

        user_entry.request_id += 1;
        assert!(!action.matches_replay_entry(&user_entry));
    }

    #[test]
    fn retained_spawn_action_is_filtered_independently() {
        let action = LabScenarioAction::LabOperation {
            request_id: 7,
            op: LabOp::SpawnEntities(vec![LabSpawnEntity {
                owner: 1,
                kind: EntityKind::Rifleman,
                x: 320.0,
                y: 320.0,
                completed: true,
            }]),
        };
        let replay_op = lab_op_to_replay_operation(match &action {
            LabScenarioAction::LabOperation { op, .. } => op,
            _ => unreachable!(),
        })
        .unwrap();
        let entry = LabReplayOperationEntry {
            sequence: 0,
            tick: 0,
            request_id: 7,
            operator_id: 99,
            op: replay_op,
        };
        let mut driver = LabScenarioDriver::scripted_for_test(0, action);
        driver.sync_to_tick(0, &[entry]);
        let scenario = crate::lab_scenarios::load_lab_scenario_by_id(SUPPLY_300_HELLHOLE_ID)
            .expect("hellhole scenario");
        let game = scenario.build_game().expect("hellhole game");
        assert!(driver.actions_for_tick(&game).is_empty());
    }
}
