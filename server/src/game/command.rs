//! Internal gameplay commands.
//!
//! Wire commands are decoded in `protocol.rs` and translated into this domain shape at the
//! networking/replay boundary. Simulation services consume `SimCommand` so they can work with
//! typed entity kinds instead of JSON-facing strings.

use crate::game::entity::EntityKind;
use crate::protocol;

#[derive(Debug, Clone, PartialEq)]
pub enum SimCommand {
    Move {
        units: Vec<u32>,
        x: f32,
        y: f32,
        queued: bool,
    },
    AttackMove {
        units: Vec<u32>,
        x: f32,
        y: f32,
        queued: bool,
    },
    Attack {
        units: Vec<u32>,
        target: u32,
        queued: bool,
    },
    SetupAtGuns {
        units: Vec<u32>,
        x: f32,
        y: f32,
    },
    TearDownAtGuns {
        units: Vec<u32>,
    },
    Charge {
        units: Vec<u32>,
    },
    Gather {
        units: Vec<u32>,
        node: u32,
        queued: bool,
    },
    Build {
        worker: u32,
        building: EntityKind,
        tile_x: u32,
        tile_y: u32,
        queued: bool,
    },
    Train {
        building: u32,
        unit: EntityKind,
    },
    Cancel {
        building: u32,
    },
    Stop {
        units: Vec<u32>,
    },
    SetRally {
        building: u32,
        x: f32,
        y: f32,
        queued: bool,
    },
    Rejected {
        reason: CommandRejection,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRejection {
    UnknownBuilding,
    UnknownUnit,
}

impl CommandRejection {
    pub(crate) fn notice_message(self) -> &'static str {
        match self {
            CommandRejection::UnknownBuilding => "Unknown building",
            CommandRejection::UnknownUnit => "Unknown unit",
        }
    }
}

impl SimCommand {
    pub fn from_protocol(command: protocol::Command) -> Self {
        match command {
            protocol::Command::Move {
                units,
                x,
                y,
                queued,
            } => SimCommand::Move {
                units,
                x,
                y,
                queued,
            },
            protocol::Command::AttackMove {
                units,
                x,
                y,
                queued,
            } => SimCommand::AttackMove {
                units,
                x,
                y,
                queued,
            },
            protocol::Command::Attack {
                units,
                target,
                queued,
            } => SimCommand::Attack {
                units,
                target,
                queued,
            },
            protocol::Command::SetupAtGuns { units, x, y } => {
                SimCommand::SetupAtGuns { units, x, y }
            }
            protocol::Command::TearDownAtGuns { units } => SimCommand::TearDownAtGuns { units },
            protocol::Command::Charge { units } => SimCommand::Charge { units },
            protocol::Command::Gather {
                units,
                node,
                queued,
            } => SimCommand::Gather {
                units,
                node,
                queued,
            },
            protocol::Command::Build {
                worker,
                building,
                tile_x,
                tile_y,
                queued,
            } => match building.parse::<EntityKind>() {
                Ok(building) if building.is_building() => SimCommand::Build {
                    worker,
                    building,
                    tile_x,
                    tile_y,
                    queued,
                },
                _ => SimCommand::Rejected {
                    reason: CommandRejection::UnknownBuilding,
                },
            },
            protocol::Command::Train { building, unit } => match unit.parse::<EntityKind>() {
                Ok(unit) if unit.is_unit() => SimCommand::Train { building, unit },
                _ => SimCommand::Rejected {
                    reason: CommandRejection::UnknownUnit,
                },
            },
            protocol::Command::Cancel { building } => SimCommand::Cancel { building },
            protocol::Command::Stop { units } => SimCommand::Stop { units },
            protocol::Command::SetRally {
                building,
                x,
                y,
                queued,
            } => SimCommand::SetRally {
                building,
                x,
                y,
                queued,
            },
        }
    }

    pub fn to_protocol(&self) -> Option<protocol::Command> {
        Some(match self {
            SimCommand::Move {
                units,
                x,
                y,
                queued,
            } => protocol::Command::Move {
                units: units.clone(),
                x: *x,
                y: *y,
                queued: *queued,
            },
            SimCommand::AttackMove {
                units,
                x,
                y,
                queued,
            } => protocol::Command::AttackMove {
                units: units.clone(),
                x: *x,
                y: *y,
                queued: *queued,
            },
            SimCommand::Attack {
                units,
                target,
                queued,
            } => protocol::Command::Attack {
                units: units.clone(),
                target: *target,
                queued: *queued,
            },
            SimCommand::SetupAtGuns { units, x, y } => protocol::Command::SetupAtGuns {
                units: units.clone(),
                x: *x,
                y: *y,
            },
            SimCommand::TearDownAtGuns { units } => protocol::Command::TearDownAtGuns {
                units: units.clone(),
            },
            SimCommand::Charge { units } => protocol::Command::Charge {
                units: units.clone(),
            },
            SimCommand::Gather {
                units,
                node,
                queued,
            } => protocol::Command::Gather {
                units: units.clone(),
                node: *node,
                queued: *queued,
            },
            SimCommand::Build {
                worker,
                building,
                tile_x,
                tile_y,
                queued,
            } => protocol::Command::Build {
                worker: *worker,
                building: building.to_protocol_str().to_string(),
                tile_x: *tile_x,
                tile_y: *tile_y,
                queued: *queued,
            },
            SimCommand::Train { building, unit } => protocol::Command::Train {
                building: *building,
                unit: unit.to_protocol_str().to_string(),
            },
            SimCommand::Cancel { building } => protocol::Command::Cancel {
                building: *building,
            },
            SimCommand::Stop { units } => protocol::Command::Stop {
                units: units.clone(),
            },
            SimCommand::SetRally {
                building,
                x,
                y,
                queued,
            } => protocol::Command::SetRally {
                building: *building,
                x: *x,
                y: *y,
                queued: *queued,
            },
            SimCommand::Rejected { .. } => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::kinds;

    #[test]
    fn protocol_build_command_translates_kind() {
        let command = protocol::Command::Build {
            worker: 7,
            building: kinds::BARRACKS.to_string(),
            tile_x: 4,
            tile_y: 8,
            queued: false,
        };

        assert_eq!(
            SimCommand::from_protocol(command),
            SimCommand::Build {
                worker: 7,
                building: EntityKind::Barracks,
                tile_x: 4,
                tile_y: 8,
                queued: false,
            }
        );
    }

    #[test]
    fn protocol_queued_flag_defaults_false_and_round_trips_when_true() {
        let decoded: protocol::Command =
            serde_json::from_str(r#"{"c":"move","units":[1],"x":10.0,"y":20.0}"#)
                .expect("omitted queued flag should deserialize");
        assert_eq!(
            SimCommand::from_protocol(decoded),
            SimCommand::Move {
                units: vec![1],
                x: 10.0,
                y: 20.0,
                queued: false,
            }
        );

        let command = protocol::Command::Move {
            units: vec![1],
            x: 10.0,
            y: 20.0,
            queued: true,
        };
        let encoded = serde_json::to_string(&command).expect("queued command should serialize");
        assert!(
            encoded.contains(r#""queued":true"#),
            "serialized command log entry should preserve queued=true"
        );
        assert_eq!(
            SimCommand::from_protocol(command),
            SimCommand::Move {
                units: vec![1],
                x: 10.0,
                y: 20.0,
                queued: true,
            }
        );
    }

    #[test]
    fn protocol_unknown_train_unit_becomes_rejected_command() {
        let command = protocol::Command::Train {
            building: 3,
            unit: "made_up".to_string(),
        };

        assert_eq!(
            SimCommand::from_protocol(command),
            SimCommand::Rejected {
                reason: CommandRejection::UnknownUnit,
            }
        );
    }

    #[test]
    fn protocol_at_gun_setup_commands_round_trip() {
        let setup = protocol::Command::SetupAtGuns {
            units: vec![3, 5],
            x: 100.0,
            y: 200.0,
        };
        assert_eq!(
            SimCommand::from_protocol(setup.clone()),
            SimCommand::SetupAtGuns {
                units: vec![3, 5],
                x: 100.0,
                y: 200.0,
            }
        );
        assert_eq!(
            SimCommand::from_protocol(setup.clone()).to_protocol(),
            Some(setup)
        );

        let teardown = protocol::Command::TearDownAtGuns { units: vec![7] };
        assert_eq!(
            SimCommand::from_protocol(teardown.clone()),
            SimCommand::TearDownAtGuns { units: vec![7] }
        );
        assert_eq!(
            SimCommand::from_protocol(teardown.clone()).to_protocol(),
            Some(teardown)
        );
    }
}
